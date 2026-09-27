// Story: 001 — system test: bridge against the real voxtral server.
// Requires a live transcription upstream; defaults to the package default
// (http://127.0.0.1:8080). Point at a tunnel with VIBE_BRIDGE_UPSTREAM_TEST,
// e.g. VIBE_BRIDGE_UPSTREAM_TEST=http://127.0.0.1:9931
// Run with: cargo test --test system_transcription -- --ignored

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

#[tokio::test]
#[ignore = "requires a live voxtral server on 127.0.0.1:9931"]
async fn test_live_transcription_end_to_end() {
    let cfg = Config::from_lookup(|k| match k {
        "VIBE_BRIDGE_UPSTREAM" => std::env::var("VIBE_BRIDGE_UPSTREAM_TEST").ok(),
        _ => None,
    });
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
