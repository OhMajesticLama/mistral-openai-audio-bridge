// Story: 001 — system test: bridge against the real transcription server.
// The upstream under test is VIBE_BRIDGE_UPSTREAM_TEST, else the test
// default (http://127.0.0.1:9931, the tunneled transcription server). These
// tests run when something is listening there and skip (with a note) when
// not — no --ignored needed, plain `cargo test` creates remote load:
//   VIBE_BRIDGE_UPSTREAM_TEST=http://127.0.0.1:8080 cargo test --test 'system*'

use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use vibe_audio_bridge::config::Config;
use vibe_audio_bridge::router;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const FIXTURE: &str = "tests/fixtures/bonjour.pcm"; // 16 kHz s16le mono, "Bonjour, comment ça va?"

async fn recv_json(ws: &mut Ws) -> Value {
    let msg = tokio::time::timeout(Duration::from_secs(30), ws.next())
        .await
        .expect("timed out waiting for message")
        .expect("ws error")
        .expect("stream closed");
    match msg {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected text message, got {other:?}"),
    }
}

/// Upstream under test: VIBE_BRIDGE_UPSTREAM_TEST, else the test default
/// (the tunneled transcription server on 9931). None when nothing is
/// listening there — live tests skip then, so the suite stays green
/// without a server and creates load when one is reachable.
async fn live_upstream() -> Option<String> {
    let url = std::env::var("VIBE_BRIDGE_UPSTREAM_TEST")
        .ok()
        .unwrap_or_else(|| "http://127.0.0.1:9931".to_string());
    let host_port = url.strip_prefix("http://").unwrap_or(&url);
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
        None => (host_port.to_string(), 80),
    };
    let probe = tokio::time::timeout(
        Duration::from_secs(1),
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await;
    probe.ok()?.ok().map(|_| url)
}

/// Skip guard for live-server tests.
macro_rules! require_live_upstream {
    ($upstream:ident) => {
        let $upstream = match live_upstream().await {
            Some(u) => u,
            None => {
                eprintln!("skipped: no live transcription server reachable");
                return;
            }
        };
    };
}

#[tokio::test]
async fn test_live_transcription_end_to_end() {
    require_live_upstream!(upstream);
    let cfg = Config {
        listen: "127.0.0.1:8081".parse().unwrap(),
        upstream,
        log_level: "info".into(),
        dump_dir: None,
        flush_interval_ms: 0,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(cfg);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{addr}/v1/audio/transcriptions/realtime?model=voxtral-realtime");
    let (mut ws, _) = connect_async(url.into_client_request().unwrap()).await.unwrap();

    let ev = recv_json(&mut ws).await;
    assert_eq!(ev["type"], "session.created");

    let pcm = std::fs::read(FIXTURE).expect("fixture bonjour.pcm missing");
    ws.send(Message::Text(
        serde_json::json!({"type": "input_audio.append", "audio": STANDARD.encode(&pcm)}).to_string().into(),
    ))
    .await
    .unwrap();
    ws.send(Message::Text(serde_json::json!({"type": "input_audio.end"}).to_string().into()))
        .await
        .unwrap();

    let mut text = String::new();
    loop {
        let ev = recv_json(&mut ws).await;
        match ev["type"].as_str().unwrap_or_default() {
            "transcription.text.delta" => text.push_str(ev["text"].as_str().unwrap_or_default()),
            "transcription.done" => {
                let done = ev["text"].as_str().unwrap_or_default();
                assert!(done.contains("Bonjour"), "unexpected transcript: {done:?}");
                break;
            }
            "error" => panic!("bridge error event: {ev}"),
            _ => {}
        }
    }
    assert!(text.contains("Bonjour"), "deltas missing transcript: {text:?}");
}

// Story: 011 — real-time transcription for audio up to at least 2 minutes:
// stream 120 s of PCM at real-time pace through the live endpoint; deltas
// must keep arriving while the speaker is still talking, and the final done
// must arrive within a bounded time after input_audio.end.
// Feature-gated (test-long): ~2.5 minutes of wall time. Run explicitly:
//   cargo test --features test-long --test system_transcription
#[cfg(feature = "test-long")]
#[tokio::test]
async fn test_two_minute_realtime_recording() {
    use futures_util::StreamExt;
    use std::time::Instant;

    require_live_upstream!(upstream);

    let fixture = std::fs::read(FIXTURE).expect("fixture bonjour.pcm missing");
    let mut pcm = fixture.clone();
    for _ in 0..19 {
        pcm.extend_from_slice(&fixture);
    }
    assert!(pcm.len() >= 120 * 32_000, "fixture too short for 120 s");

    let cfg = Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream,
        log_level: "info".into(),
        dump_dir: None,
        flush_interval_ms: 1000,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(cfg);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let url = format!("ws://{addr}/v1/audio/transcriptions/realtime?model=voxtral-realtime");
    let (ws, _) = connect_async(url.into_client_request().unwrap()).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    // Sender: 100 ms chunks at real-time pace, then end.
    let (end_tx, mut end_rx) = tokio::sync::mpsc::channel::<Instant>(1);
    tokio::spawn(async move {
        for chunk in pcm.chunks(3200) {
            let msg = serde_json::json!({
                "type": "input_audio.append",
                "audio": STANDARD.encode(chunk),
            });
            use futures_util::SinkExt;
            sink.send(Message::Text(msg.to_string().into())).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        use futures_util::SinkExt;
        sink.send(Message::Text(
            serde_json::json!({"type": "input_audio.end"}).to_string().into(),
        ))
        .await
        .unwrap();
        let _ = end_tx.send(Instant::now()).await;
    });

    let mut deltas_during = 0u32;
    let mut end_instant: Option<Instant> = None;
    let mut done_text = String::new();
    loop {
        tokio::select! {
            t = end_rx.recv() => {
                if let Some(t) = t {
                    end_instant = Some(t);
                }
            }
            msg = stream.next() => {
                let msg = msg.expect("ws error").expect("stream closed");
                let Message::Text(t) = msg else { continue };
                let ev: Value = serde_json::from_str(&t).unwrap();
                match ev["type"].as_str().unwrap_or_default() {
                    "transcription.text.delta" => {
                        if end_instant.is_none() {
                            deltas_during += 1;
                        }
                    }
                    "transcription.done" => {
                        done_text = ev["text"].as_str().unwrap_or_default().to_string();
                        break;
                    }
                    "error" => panic!("bridge error event: {ev}"),
                    _ => {}
                }
            }
        }
    }

    assert!(deltas_during > 0, "no deltas arrived while still talking");
    assert!(!done_text.trim().is_empty(), "empty transcript for 120 s of speech");
    let tail = end_instant.expect("end was never sent").elapsed();
    assert!(tail.as_secs() < 30, "done took {tail:?} after input_audio.end");
}

// Story: 011 — fast linearity check (~2 s): on the live path, per-chunk
// processing must not grow with the recording length. Send 500 ms of audio,
// measure the time to its delta, send the next 500 ms, and require the
// second chunk's latency < 1.5x the first's. A full-buffer re-processing
// regression (flush-style) would grow this ratio with chunk size; the
// feature-gated 2-minute test is the strong proof, this is the cheap guard.
#[tokio::test]
async fn test_live_processing_is_incremental() {
    require_live_upstream!(upstream);

    let fixture = std::fs::read(FIXTURE).expect("fixture bonjour.pcm missing");
    assert!(fixture.len() >= 32_000, "fixture too short for 2 x 500 ms");

    let cfg = Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream,
        log_level: "info".into(),
        dump_dir: None,
        flush_interval_ms: 1000,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(cfg);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let url = format!("ws://{addr}/v1/audio/transcriptions/realtime?model=voxtral-realtime");
    let (mut ws, _) = connect_async(url.into_client_request().unwrap()).await.unwrap();
    let ev: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(ev["type"], "session.created");

    async fn send_chunk(ws: &mut Ws, pcm: &[u8]) {
        use futures_util::SinkExt;
        ws.send(Message::Text(
            serde_json::json!({
                "type": "input_audio.append",
                "audio": STANDARD.encode(pcm),
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    }

    /// Wait for the next delta; None on timeout (used while priming).
    async fn try_delta(ws: &mut Ws, limit: std::time::Duration) -> Option<std::time::Duration> {
        let start = std::time::Instant::now();
        let got = tokio::time::timeout(limit, async {
            loop {
                let msg = ws.next().await.unwrap().unwrap();
                let Message::Text(t) = msg else { continue };
                let ev: Value = serde_json::from_str(&t).unwrap();
                match ev["type"].as_str().unwrap_or_default() {
                    "transcription.text.delta" => return,
                    "error" => panic!("bridge error event: {ev}"),
                    _ => {}
                }
            }
        })
        .await;
        got.ok().map(|_| start.elapsed())
    }

    async fn wait_delta(ws: &mut Ws) -> std::time::Duration {
        let start = std::time::Instant::now();
        let got = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let msg = ws.next().await.unwrap().unwrap();
                let Message::Text(t) = msg else { continue };
                let ev: Value = serde_json::from_str(&t).unwrap();
                match ev["type"].as_str().unwrap_or_default() {
                    "transcription.text.delta" => return,
                    "error" => panic!("bridge error event: {ev}"),
                    _ => {}
                }
            }
        })
        .await;
        got.expect("no delta within 10 s of sending a chunk");
        start.elapsed()
    }

    // The model emits its first delta only after enough context, so prime
    // with 500 ms speech chunks until deltas flow, then measure two
    // consecutive chunk latencies.
    let speech = &fixture[80_000..]; // 2.5 s onward: the speech-active region
    let mut sent = 0usize;
    let mut primed = false;
    while sent + 16_000 <= speech.len() {
        send_chunk(&mut ws, &speech[sent..sent + 16_000]).await;
        sent += 16_000;
        if let Some(_) = try_delta(&mut ws, std::time::Duration::from_secs(2)).await {
            primed = true;
            break;
        }
    }
    assert!(primed, "no delta arrived while priming with speech audio");

    // Drain: wait for a quiet window so the measured latencies are genuine
    // per-chunk round trips, not stale deltas still in flight.
    while try_delta(&mut ws, std::time::Duration::from_millis(300)).await.is_some() {}

    // Two consecutive 500 ms chunks: the second must not process slower
    // than 1.5x the first.
    assert!(sent + 32_000 <= speech.len(), "fixture exhausted during priming");
    send_chunk(&mut ws, &speech[sent..sent + 16_000]).await;
    let t1 = wait_delta(&mut ws).await;
    sent += 16_000;
    send_chunk(&mut ws, &speech[sent..sent + 16_000]).await;
    let t2 = wait_delta(&mut ws).await;

    assert!(
        t2 < t1 * 3 / 2 + std::time::Duration::from_millis(300),
        "second chunk's latency {t2:?} grew past 1.5x the first's {t1:?} \
         (+300 ms jitter slack): processing is not incremental"
    );
}
