// Story: 001 — realtime transcription protocol over WebSocket, against a mock upstream.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use vibe_audio_bridge::config::Config;
use vibe_audio_bridge::protocol::{parse_client_message, ClientMessage};
use vibe_audio_bridge::router;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const SSE_OK: &str = concat!(
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\" Bon\"}\n\n",
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\"jour\"}\n\n",
    "data: {\"type\":\"transcript.text.done\",\"text\":\" Bonjour\"}\n\n",
    "data: [DONE]\n\n",
);

const SSE_BON: &str = concat!(
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\" Bon\"}\n\n",
    "data: {\"type\":\"transcript.text.done\",\"text\":\" Bon\"}\n\n",
);

const SSE_BONJOUR: &str = concat!(
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\" Bon\"}\n\n",
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\"jour\"}\n\n",
    "data: {\"type\":\"transcript.text.done\",\"text\":\" Bonjour\"}\n\n",
);

/// Mock upstream serving a different SSE body per hit (the last one repeats),
/// so a growing buffer "transcribes" to a growing text.
async fn spawn_upstream_seq(bodies: Vec<&'static str>) -> MockUpstream {
    assert!(!bodies.is_empty());
    let hits = Arc::new(Mutex::new(0u32));
    let last_body = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits_clone = hits.clone();
    let body_clone = last_body.clone();
    let bodies = Arc::new(bodies);
    let app = axum::Router::new().route(
        "/v1/audio/transcriptions",
        axum::routing::post(move |bytes: Bytes| {
            let hits = hits_clone.clone();
            let last_body = body_clone.clone();
            let bodies = bodies.clone();
            async move {
                let n = {
                    let mut h = hits.lock().await;
                    *h += 1;
                    *h - 1
                };
                *last_body.lock().await = bytes.to_vec();
                let body = bodies[(n as usize).min(bodies.len() - 1)];
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    body.to_string(),
                )
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    MockUpstream { addr, hits, last_body }
}

struct MockUpstream {
    addr: SocketAddr,
    hits: Arc<Mutex<u32>>,
    last_body: Arc<Mutex<Vec<u8>>>,
}

async fn spawn_upstream(status: u16, body: &'static str) -> MockUpstream {
    let hits = Arc::new(Mutex::new(0u32));
    let last_body = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits_clone = hits.clone();
    let body_clone = last_body.clone();
    let app = axum::Router::new().route(
        "/v1/audio/transcriptions",
        axum::routing::post(move |bytes: Bytes| {
            let hits = hits_clone.clone();
            let last_body = body_clone.clone();
            async move {
                *hits.lock().await += 1;
                *last_body.lock().await = bytes.to_vec();
                (
                    axum::http::StatusCode::from_u16(status).unwrap(),
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    body.to_string(),
                )
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    MockUpstream { addr, hits, last_body }
}

async fn spawn_bridge(cfg: Config) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(cfg);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

fn bridge_cfg(upstream: SocketAddr) -> Config {
    Config {
        listen: "127.0.0.1:8081".parse().unwrap(),
        upstream: format!("http://{upstream}"),
        log_level: "info".into(),
        dump_dir: None,
        flush_interval_ms: 1000,
    }
}

async fn ws_connect(addr: SocketAddr, query: &str) -> Ws {
    let url = format!("ws://{}/v1/audio/transcriptions/realtime{}", addr, query);
    let (ws, _) = connect_async(url.into_client_request().unwrap()).await.unwrap();
    ws
}

async fn recv_json(ws: &mut Ws) -> Value {
    let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("timed out waiting for message")
        .expect("ws error")
        .expect("stream closed");
    match msg {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected text message, got {other:?}"),
    }
}

async fn send_json(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

fn append_msg(pcm: &[u8]) -> Value {
    serde_json::json!({"type": "input_audio.append", "audio": STANDARD.encode(pcm)})
}

fn end_msg() -> Value {
    serde_json::json!({"type": "input_audio.end"})
}

async fn collect_until_done(ws: &mut Ws) -> (Vec<String>, String) {
    let mut deltas = Vec::new();
    loop {
        let ev = recv_json(ws).await;
        match ev["type"].as_str().unwrap_or_default() {
            "transcription.text.delta" => deltas.push(ev["text"].as_str().unwrap_or_default().to_string()),
            "transcription.done" => return (deltas, ev["text"].as_str().unwrap_or_default().to_string()),
            "error" => panic!("unexpected error event: {ev}"),
            _ => {}
        }
    }
}

// Bug repro: Vibe voice mode shows no text until the recording ends.
// The bridge must emit transcription.text.delta while audio is still being
// appended, without waiting for input_audio.end.
#[tokio::test]
async fn test_deltas_stream_before_input_audio_end() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await; // session.created

    // Recording in progress: audio appended, no input_audio.end sent.
    send_json(&mut ws, append_msg(&[0u8; 32])).await;

    // A delta must arrive on its own; timing out means the bridge is
    // still batch-only (buffers everything until input_audio.end).
    let ev = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("no event before input_audio.end: streaming is batch-only")
        .expect("ws error")
        .expect("stream closed");
    let ev: Value = match ev {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected text message, got {other:?}"),
    };
    assert_eq!(ev["type"], "transcription.text.delta");
}

// Story: 011 — deltas emitted during recording concatenate to the final text.
#[tokio::test]
async fn test_streaming_deltas_concatenate_to_done() {
    // The mock transcribes a growing buffer: hit 1 -> " Bon", later hits -> " Bonjour".
    let upstream = spawn_upstream_seq(vec![SSE_BON, SSE_BONJOUR, SSE_BONJOUR]).await;
    let cfg = Config { flush_interval_ms: 50, ..bridge_cfg(upstream.addr) };
    let addr = spawn_bridge(cfg).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await; // session.created

    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    // First flush transcribes the buffer so far; all of its text is new.
    let d1 = recv_json(&mut ws).await;
    assert_eq!(d1["type"], "transcription.text.delta");
    assert_eq!(d1["text"], " Bon");

    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    // Second flush re-transcribes from scratch; only the unsent suffix may be emitted.
    let d2 = recv_json(&mut ws).await;
    assert_eq!(d2["type"], "transcription.text.delta");
    assert_eq!(d2["text"], "jour");

    // Ending the recording produces the authoritative done event without
    // re-emitting already-sent text.
    send_json(&mut ws, end_msg()).await;
    let (deltas, done) = collect_until_done(&mut ws).await;
    assert!(deltas.is_empty(), "final transcription re-emitted text: {deltas:?}");
    assert_eq!(done, " Bonjour");
}

#[tokio::test]
async fn test_session_created_on_connect() {
    let addr = spawn_bridge(bridge_cfg(spawn_upstream(200, SSE_OK).await.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let ev = recv_json(&mut ws).await;
    assert_eq!(ev["type"], "session.created");
    assert_eq!(ev["session"]["model"], "test-model");
    assert_eq!(ev["session"]["audio_format"]["encoding"], "pcm_s16le");
    assert_eq!(ev["session"]["audio_format"]["sample_rate"], 16000);
    assert!(ev["session"]["request_id"].as_str().is_some());

    // Without a model query parameter the default model is used.
    let mut ws = ws_connect(addr, "").await;
    let ev = recv_json(&mut ws).await;
    assert_eq!(ev["session"]["model"], "voxtral-realtime");
}

#[tokio::test]
async fn test_append_then_end_posts_wav_upstream() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let pcm: Vec<u8> = (0..64u8).collect();
    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await; // session.created
    send_json(&mut ws, append_msg(&pcm)).await;
    send_json(&mut ws, end_msg()).await;
    let _ = collect_until_done(&mut ws).await;

    assert_eq!(*upstream.hits.lock().await, 1);
    let body = upstream.last_body.lock().await.clone();
    assert!(body.windows(4).any(|w| w == b"WAVE"), "upstream got no WAV");
    // The WAV data chunk must contain exactly the sent PCM (the multipart
    // body continues past it with the closing boundary).
    let wave_pos = body.windows(4).position(|w| w == b"WAVE").expect("no WAV");
    let pos = body[wave_pos..]
        .windows(4)
        .position(|w| w == b"data")
        .map(|p| p + wave_pos)
        .expect("no data chunk");
    let len = u32::from_le_bytes(body[pos + 4..pos + 8].try_into().unwrap()) as usize;
    assert_eq!(&body[pos + 8..pos + 8 + len], &pcm[..], "data chunk != sent PCM");
}

#[tokio::test]
async fn test_upstream_deltas_forwarded_as_transcription_events() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await;
    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    send_json(&mut ws, end_msg()).await;

    let (deltas, done) = collect_until_done(&mut ws).await;
    assert_eq!(deltas.join(""), " Bonjour");
    assert_eq!(done, " Bonjour");
}

#[tokio::test]
async fn test_empty_recording_short_circuits() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await;
    send_json(&mut ws, end_msg()).await;

    let (deltas, done) = collect_until_done(&mut ws).await;
    assert!(deltas.is_empty());
    assert_eq!(done, "");
    assert_eq!(*upstream.hits.lock().await, 0, "upstream must not be called");
}

#[tokio::test]
async fn test_two_recordings_in_one_session() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await; // session.created
    for _ in 0..2 {
        send_json(&mut ws, append_msg(&[0u8; 32])).await;
        send_json(&mut ws, end_msg()).await;
        let (_, done) = collect_until_done(&mut ws).await;
        assert_eq!(done, " Bonjour");
    }
    // Buffer must reset between recordings: both hit the upstream.
    assert_eq!(*upstream.hits.lock().await, 2);
}

#[tokio::test]
async fn test_upstream_trailing_slash_still_resolves() {
    let upstream = spawn_upstream(200, SSE_OK).await;
    let cfg = Config { upstream: format!("http://{}/", upstream.addr), ..bridge_cfg(upstream.addr) };
    let addr = spawn_bridge(cfg).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await;
    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    send_json(&mut ws, end_msg()).await;
    let _ = collect_until_done(&mut ws).await;
    assert_eq!(*upstream.hits.lock().await, 1, "trailing slash broke the upstream URL");
}

#[tokio::test]
async fn test_upstream_error_reported_to_client() {
    let upstream = spawn_upstream(500, "boom").await;
    let addr = spawn_bridge(bridge_cfg(upstream.addr)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await;
    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    send_json(&mut ws, end_msg()).await;

    let ev = recv_json(&mut ws).await;
    assert_eq!(ev["type"], "error");
    let msg = ev["error"]["message"].as_str().unwrap();
    assert!(msg.contains("500"), "message should mention status: {msg}");
    assert!(msg.contains("boom"), "message should include body: {msg}");
}

#[tokio::test]
async fn test_debug_dump_written() {
    let dump_dir = std::env::temp_dir().join(format!("bridge-dump-test-{}", std::process::id()));
    std::fs::remove_dir_all(&dump_dir).ok();
    std::fs::create_dir_all(&dump_dir).unwrap();

    let upstream = spawn_upstream(200, SSE_OK).await;
    let cfg = Config { dump_dir: Some(dump_dir.clone()), ..bridge_cfg(upstream.addr) };
    let addr = spawn_bridge(cfg).await;

    let pcm: Vec<u8> = (200..232u8).collect();
    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await;
    send_json(&mut ws, append_msg(&pcm)).await;
    send_json(&mut ws, end_msg()).await;
    let _ = collect_until_done(&mut ws).await;

    let mut files: Vec<_> = std::fs::read_dir(&dump_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "expected exactly one dump file");
    let contents = std::fs::read(files.remove(0).unwrap().path()).unwrap();
    assert_eq!(contents, pcm);
    std::fs::remove_dir_all(&dump_dir).ok();
}

#[test]
fn test_parse_client_message() {
    let pcm = vec![1u8, 2, 3, 4];
    let b64 = STANDARD.encode(&pcm);
    assert_eq!(
        parse_client_message(&format!("{{\"type\":\"input_audio.append\",\"audio\":\"{b64}\"}}")),
        Some(ClientMessage::Append(pcm))
    );
    // flush is ignored like unknown types (batch upstream: nothing to flush)
    assert_eq!(parse_client_message("{\"type\":\"input_audio.flush\"}"), None);
    assert_eq!(
        parse_client_message("{\"type\":\"input_audio.end\"}"),
        Some(ClientMessage::End)
    );
    assert_eq!(parse_client_message("{\"type\":\"session.update\"}"), None);
    assert_eq!(parse_client_message("not json"), None);
}


// ---- Story: 011 — live ingestion endpoint (chunked PCM in, deltas out). ----

/// mpsc receiver as a Stream, for axum's Body::from_stream.
struct Chan(tokio::sync::mpsc::Receiver<Result<axum::body::Bytes, std::io::Error>>);
impl futures_util::Stream for Chan {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

/// Live-ingest mock: one delta per received body chunk, a done event when
/// the upload ends, then the terminal marker.
async fn live_handler(req: axum::extract::Request) -> axum::response::Response {
    use futures_util::StreamExt;
    let mut stream = req.into_body().into_data_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::io::Error>>(16);
    tokio::spawn(async move {
        let mut n = 0u32;
        while let Some(chunk) = stream.next().await {
            if chunk.is_ok() {
                n += 1;
                let event = format!(
                    "data: {{\"type\":\"transcript.text.delta\",\"delta\":\" w{n}\"}}\n\n"
                );
                if tx.send(Ok(event.into())).await.is_err() {
                    return;
                }
            }
        }
        let done = "data: {\"type\":\"transcript.text.done\",\"text\":\" live done\"}\n\n";
        let _ = tx.send(Ok(done.into())).await;
        let marker = format!("data: [{}]\n\n", "DONE");
        let _ = tx.send(Ok(marker.into())).await;
    });
    axum::http::Response::builder()
        .status(200)
        .header(axum::http::header::CONTENT_TYPE, "text/event-stream")
        .body(axum::body::Body::from_stream(Chan(rx)))
        .unwrap()
}

/// Mock upstream serving both the batch route (SSE_OK) and the live route.
async fn spawn_upstream_live() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/v1/audio/transcriptions",
            axum::routing::post(|| async {
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    SSE_OK.to_string(),
                )
            }),
        )
        .route("/v1/audio/transcriptions/live", axum::routing::post(live_handler));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn test_live_endpoint_streams_deltas() {
    let upstream = spawn_upstream_live().await;
    let addr = spawn_bridge(bridge_cfg(upstream)).await;

    let mut ws = ws_connect(addr, "?model=test-model").await;
    let _ = recv_json(&mut ws).await; // session.created

    // One append, no end: the live route must produce a delta on its own.
    send_json(&mut ws, append_msg(&[0u8; 32])).await;
    let ev = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("no delta before input_audio.end: live endpoint not used")
        .expect("ws error")
        .expect("stream closed");
    let ev: Value = match ev {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected text message, got {other:?}"),
    };
    assert_eq!(ev["type"], "transcription.text.delta");
    assert_eq!(ev["text"], " w1");

    // Ending the recording closes the upload; the done event carries the
    // live transcript.
    send_json(&mut ws, end_msg()).await;
    let (deltas, done) = collect_until_done(&mut ws).await;
    assert!(
        deltas.iter().all(|d| d.starts_with(" w")),
        "unexpected deltas after end: {deltas:?}"
    );
    assert_eq!(done, " live done");
}
