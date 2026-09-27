// Mistral realtime transcription protocol: client message parsing, event
// construction, and the per-session loop that bridges to the HTTP upstream.

use axum::extract::ws::{Message, WebSocket};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use crate::config::Config;
use crate::wav;

#[derive(Debug, PartialEq)]
pub enum ClientMessage {
    Append(Vec<u8>),
    End,
}

/// Parse one client text frame; None for non-JSON, ignored or unknown types,
/// or bad base64.
pub fn parse_client_message(raw: &str) -> Option<ClientMessage> {
    let v: Value = serde_json::from_str(raw).ok()?;
    match v["type"].as_str()? {
        "input_audio.append" => Some(ClientMessage::Append(
            STANDARD.decode(v["audio"].as_str()?).ok()?,
        )),
        "input_audio.end" => Some(ClientMessage::End),
        _ => None, // flush, session.update and unknown types are ignored
    }
}

pub fn session_created(model: &str) -> String {
    json!({
        "type": "session.created",
        "session": {
            "request_id": uuid::Uuid::new_v4().to_string(),
            "model": model,
            "audio_format": {"encoding": "pcm_s16le", "sample_rate": 16_000},
            "target_streaming_delay_ms": 500,
        }
    })
    .to_string()
}

fn done_event(model: &str, text: &str) -> String {
    json!({
        "type": "transcription.done",
        "model": model,
        "text": text,
        "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
        "language": null,
    })
    .to_string()
}

async fn send_error(ws: &mut WebSocket, message: &str) {
    let _ = ws
        .send(Message::Text(
            json!({"type": "error", "error": {"message": message, "code": 0}}).to_string().into(),
        ))
        .await;
}

/// One WebSocket session: buffer appends, transcribe on end.
pub async fn session(mut ws: WebSocket, cfg: Config, model: String) {
    tracing::info!("session start model={model}");
    if ws.send(Message::Text(session_created(&model).into())).await.is_err() {
        return;
    }
    let mut buf: Vec<u8> = Vec::new();
    while let Some(Ok(msg)) = ws.recv().await {
        let Message::Text(text) = msg else { continue };
        match parse_client_message(&text) {
            Some(ClientMessage::Append(pcm)) => buf.extend_from_slice(&pcm),
            Some(ClientMessage::End) => {
                let pcm = std::mem::take(&mut buf);
                transcribe(&mut ws, &cfg, &model, pcm).await;
            }
            None => {} // flush, session.update and unknown types are ignored
        }
    }
    tracing::info!("session end model={model}");
}

async fn transcribe(ws: &mut WebSocket, cfg: &Config, model: &str, pcm: Vec<u8>) {
    if pcm.is_empty() {
        // Empty recording (user released without speaking): benign done.
        let _ = ws.send(Message::Text(done_event(model, "").into())).await;
        return;
    }

    let peak = wav::peak(&pcm);
    tracing::info!(
        "transcribing {} bytes (~{:.2}s) peak={}/32767",
        pcm.len(),
        pcm.len() as f64 / 32_000.0,
        peak
    );
    if let Some(dir) = &cfg.dump_dir {
        dump(dir, &pcm);
    }

    let part = reqwest::multipart::Part::bytes(wav::wrap(&pcm))
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .expect("static mime type");
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .text("stream", "true".to_string())
        .part("file", part);

    let resp = match reqwest::Client::new()
        .post(format!(
            "{}/v1/audio/transcriptions",
            cfg.upstream.trim_end_matches('/')
        ))
        .multipart(form)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(exc) => {
            send_error(ws, &format!("bridge error: {exc}")).await;
            return;
        }
    };

    if resp.status() != reqwest::StatusCode::OK {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let truncated: String = body.chars().take(500).collect();
        send_error(ws, &format!("upstream HTTP {status}: {truncated}")).await;
        return;
    }

    forward_sse(ws, model, resp).await;
}

/// Stream the upstream SSE body, forwarding deltas and the final done event.
async fn forward_sse(ws: &mut WebSocket, model: &str, resp: reqwest::Response) {
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut pending = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(exc) => {
                send_error(ws, &format!("bridge error: {exc}")).await;
                return;
            }
        };
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = pending.find('\n') {
            let line: String = pending.drain(..=pos).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            let Some(payload) = line.strip_prefix("data: ") else {
                continue;
            };
            if payload == "[DONE]" {
                return;
            }
            let Ok(ev) = serde_json::from_str::<Value>(payload) else {
                continue;
            };
            match ev["type"].as_str().unwrap_or_default() {
                "transcript.text.delta" => {
                    let out = json!({
                        "type": "transcription.text.delta",
                        "text": ev["delta"].as_str().unwrap_or_default(),
                    });
                    if ws.send(Message::Text(out.to_string().into())).await.is_err() {
                        return;
                    }
                }
                "transcript.text.done" => {
                    let text = ev["text"].as_str().unwrap_or_default();
                    tracing::info!("upstream done text={text:?}");
                    let _ = ws.send(Message::Text(done_event(model, text).into())).await;
                    return;
                }
                _ => {}
            }
        }
    }
    // ponytail: upstream ended without transcript.text.done — Python bridge
    // also returns silently here; revisit if Vibe ever hangs on this.
}

fn dump(dir: &std::path::Path, pcm: &[u8]) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("{}.pcm", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()));
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(pcm);
        tracing::info!("dumped pcm to {}", path.display());
    }
}
