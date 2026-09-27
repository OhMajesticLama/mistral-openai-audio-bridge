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
    Config::from_lookup(|k| match k {
        "VIBE_BRIDGE_UPSTREAM" => Some(format!("http://{}", upstream)),
        _ => None,
    })
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

#[tokio::test]
async fn test_session_created_on_connect() {
    let addr = spawn_bridge(bridge_cfg(spawn_upstream(200, SSE_OK).await.addr)).await;

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
    let ev = recv_json(&mut ws).await;
    assert_eq!(ev["type"], "session.created");
    assert_eq!(ev["session"]["model"], "voxtral-realtime");
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
    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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
    let cfg = Config::from_lookup(|k| match k {
        "VIBE_BRIDGE_UPSTREAM" => Some(format!("http://{}/", upstream.addr)),
        _ => None,
    });
    let addr = spawn_bridge(cfg).await;

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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

    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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
    let cfg = Config::from_lookup(|k| match k {
        "VIBE_BRIDGE_UPSTREAM" => Some(format!("http://{}", upstream.addr)),
        "VOXTRAL_DEBUG_DUMP" => Some("1".to_string()),
        "VIBE_BRIDGE_DUMP_DIR" => Some(dump_dir.to_string_lossy().into_owned()),
        _ => None,
    });
    let addr = spawn_bridge(cfg).await;

    let pcm: Vec<u8> = (200..232u8).collect();
    let mut ws = ws_connect(addr, "?model=voxtral-realtime").await;
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
    assert_eq!(
        parse_client_message("{\"type\":\"input_audio.flush\"}"),
        Some(ClientMessage::Flush)
    );
    assert_eq!(
        parse_client_message("{\"type\":\"input_audio.end\"}"),
        Some(ClientMessage::End)
    );
    assert_eq!(parse_client_message("{\"type\":\"session.update\"}"), None);
    assert_eq!(parse_client_message("not json"), None);
}
