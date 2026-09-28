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

/// mpsc receiver as a Stream, for reqwest::Body::wrap_stream.
struct BodyChannel(tokio::sync::mpsc::Receiver<Vec<u8>>);

impl futures_util::Stream for BodyChannel {
    type Item = Result<Vec<u8>, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_recv(cx).map(|r| r.map(|v| Ok(v)))
    }
}

/// Incremental SSE reader over a live response: yields "data: " payloads.
struct SseReader {
    stream:
        std::pin::Pin<Box<dyn futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    pending: String,
}

impl SseReader {
    fn new(resp: reqwest::Response) -> Self {
        SseReader {
            stream: Box::pin(resp.bytes_stream()),
            pending: String::new(),
        }
    }

    /// Next "data: " payload, or None when the stream ends or fails.
    async fn next_event(&mut self) -> Option<String> {
        use futures_util::StreamExt;
        loop {
            while let Some(pos) = self.pending.find('\n') {
                let line: String = self.pending.drain(..=pos).collect();
                let line = line.trim_end_matches(['\n', '\r']);
                if let Some(payload) = line.strip_prefix("data: ") {
                    return Some(payload.to_string());
                }
            }
            let chunk = self.stream.next().await?;
            self.pending.push_str(&String::from_utf8_lossy(&chunk.ok()?));
        }
    }
}

/// One open live-ingest request: PCM flows in through `body_tx` while
/// transcript events are read from `sse` on the same connection.
struct LiveSession {
    body_tx: Option<tokio::sync::mpsc::Sender<Vec<u8>>>,
    sse: SseReader,
    /// input_audio.end sent; waiting for the upstream's final done event.
    ended: bool,
    /// Upstream done arrived before end (early server finish).
    done_text: Option<String>,
}

/// Open a live-ingest request. The body stays open (fed via the returned
/// sender) while the SSE response can already be read.
async fn open_live(cfg: &Config, model: &str) -> Result<LiveSession, String> {
    let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
    let url = format!(
        "{}/v1/audio/transcriptions/live",
        cfg.upstream.trim_end_matches('/')
    );
    let resp = reqwest::Client::new()
        .post(url)
        .query(&[
            ("model", model),
            ("sample_rate", "16000"),
            ("channels", "1"),
            ("sample_format", "s16le"),
        ])
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(reqwest::Body::wrap_stream(BodyChannel(body_rx)))
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
        .map_err(|exc| format!("bridge error: {exc}"))?;
    if resp.status() != reqwest::StatusCode::OK {
        return Err(format!("live endpoint HTTP {}", resp.status().as_u16()));
    }
    Ok(LiveSession {
        body_tx: Some(body_tx),
        sse: SseReader::new(resp),
        ended: false,
        done_text: None,
    })
}

/// One WebSocket session. Preferred path: stream audio into the upstream's
/// live-ingest endpoint and forward its deltas as they are produced. If the
/// upstream has no live route, batch-transcribe on end only.
pub async fn session(mut ws: WebSocket, cfg: Config, model: String) {
    tracing::info!("session start model={model}");
    if ws
        .send(Message::Text(session_created(&model).into()))
        .await
        .is_err()
    {
        return;
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut live: Option<LiveSession> = None;
    let mut live_supported = true; // false after a failed open: batch on end
    loop {
        tokio::select! {
            ev = async {
                match live.as_mut() {
                    Some(l) => l.sse.next_event().await,
                    None => std::future::pending().await,
                }
            }, if live.is_some() => {
                let l = live.as_mut().unwrap();
                match ev {
                    Some(payload) => {
                        if payload == concat!("[", "DONE", "]") {
                            // Server closed the stream; treat as stream end.
                            let ended = l.ended;
                            live = None;
                            if ended {
                                // No done event arrived: fall back to a batch
                                // transcription of the buffered recording.
                                match transcribe(&cfg, &model, &buf).await {
                                    Ok(text) => {
                                        let _ = ws
                                            .send(Message::Text(done_event(&model, &text).into()))
                                            .await;
                                    }
                                    Err(msg) => send_error(&mut ws, &msg).await,
                                }
                            } else {
                                // Live died mid-recording: batch on end.
                                live_supported = false;
                            }
                        } else if let Ok(ev) = serde_json::from_str::<Value>(&payload) {
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
                                    let text = ev["text"].as_str().unwrap_or_default().to_string();
                                    tracing::info!("upstream done text={text:?}");
                                    if l.ended {
                                        let _ = ws
                                            .send(Message::Text(done_event(&model, &text).into()))
                                            .await;
                                        live = None;
                                    } else {
                                        l.done_text = Some(text);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    None => {
                        // SSE stream ended without the terminal marker.
                        let ended = l.ended;
                        live = None;
                        if ended {
                            match transcribe(&cfg, &model, &buf).await {
                                Ok(text) => {
                                    let _ = ws
                                        .send(Message::Text(done_event(&model, &text).into()))
                                        .await;
                                }
                                Err(msg) => send_error(&mut ws, &msg).await,
                            }
                        } else {
                            live_supported = false;
                        }
                    }
                }
            }
            msg = ws.recv() => {
                let Some(Ok(msg)) = msg else { break };
                let Message::Text(text) = msg else { continue };
                match parse_client_message(&text) {
                    Some(ClientMessage::Append(pcm)) => {
                        buf.extend_from_slice(&pcm);
                        match live.as_mut() {
                            Some(l) => {
                                if let Some(tx) = &l.body_tx {
                                    if tx.send(pcm).await.is_err() {
                                        // Upstream closed the request mid-recording.
                                        live = None;
                                        live_supported = false;
                                    }
                                }
                                // body_tx already closed (end sent): audio
                                // buffers until the pending done arrives.
                            }
                            None if live_supported => {
                                match open_live(&cfg, &model).await {
                                    Ok(l) => {
                                        if let Some(tx) = &l.body_tx {
                                            let _ = tx.send(buf.clone()).await;
                                        }
                                        live = Some(l);
                                    }
                                    Err(msg) => {
                                        tracing::warn!(
                                            "live endpoint unavailable ({msg}); \
                                             batch transcription on end"
                                        );
                                        live_supported = false;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(ClientMessage::End) => {
                        let pcm = std::mem::take(&mut buf);
                        if let Some(mut l) = live.take() {
                            log_recording(&cfg, &pcm);
                            l.body_tx = None; // close the upload; done follows
                            if let Some(text) = l.done_text.take() {
                                let _ = ws
                                    .send(Message::Text(done_event(&model, &text).into()))
                                    .await;
                            } else {
                                l.ended = true;
                                live = Some(l);
                            }
                        } else if pcm.is_empty() {
                            // Empty recording (user released without speaking): benign done.
                            let _ = ws
                                .send(Message::Text(done_event(&model, "").into()))
                                .await;
                        } else {
                            log_recording(&cfg, &pcm);
                            match transcribe(&cfg, &model, &pcm).await {
                                Ok(text) => {
                                    let _ = ws
                                        .send(Message::Text(done_event(&model, &text).into()))
                                        .await;
                                }
                                Err(msg) => send_error(&mut ws, &msg).await,
                            }
                        }
                    }
                    None => {} // flush, session.update and unknown types are ignored
                }
            }
        }
    }
    tracing::info!("session end model={model}");
}
fn log_recording(cfg: &Config, pcm: &[u8]) {
    let peak = wav::peak(pcm);
    tracing::info!(
        "transcribing {} bytes (~{:.2}s) peak={}/32767",
        pcm.len(),
        pcm.len() as f64 / 32_000.0,
        peak
    );
    if let Some(dir) = &cfg.dump_dir {
        dump(dir, pcm);
    }
}

/// POST the PCM to the upstream and return its final transcript.
async fn transcribe(cfg: &Config, model: &str, pcm: &[u8]) -> Result<String, String> {
    let part = reqwest::multipart::Part::bytes(wav::wrap(pcm))
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .expect("static mime type");
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .text("stream", "true".to_string())
        .part("file", part);

    let resp = reqwest::Client::new()
        .post(format!(
            "{}/v1/audio/transcriptions",
            cfg.upstream.trim_end_matches('/')
        ))
        .multipart(form)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|exc| format!("bridge error: {exc}"))?;

    if resp.status() != reqwest::StatusCode::OK {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let truncated: String = body.chars().take(500).collect();
        return Err(format!("upstream HTTP {status}: {truncated}"));
    }

    collect_done_text(resp).await
}

/// Stream the upstream SSE body and return the final transcript text.
/// Upstream deltas are ignored: the batch path emits only the final
/// transcription.done.
async fn collect_done_text(resp: reqwest::Response) -> Result<String, String> {
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut pending = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|exc| format!("bridge error: {exc}"))?;
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = pending.find('\n') {
            let line: String = pending.drain(..=pos).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            let Some(payload) = line.strip_prefix("data: ") else {
                continue;
            };
            if payload == "[D]" {
                break;
            }
            let Ok(ev) = serde_json::from_str::<Value>(payload) else {
                continue;
            };
            if ev["type"].as_str().unwrap_or_default() == "transcript.text.done" {
                let text = ev["text"].as_str().unwrap_or_default().to_string();
                tracing::info!("upstream done text={text:?}");
                return Ok(text);
            }
        }
    }
    Err("bridge error: upstream ended without transcript.text.done".to_string())
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
