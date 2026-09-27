// Vibe audio bridge: Mistral realtime transcription WS protocol -> OpenAI-compatible transcriptions HTTP.

pub mod cli;
pub mod config;
pub mod protocol;
pub mod wav;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::Router;
use config::Config;
use std::collections::HashMap;

pub fn router(cfg: Config) -> Router {
    Router::new()
        .route("/v1/audio/transcriptions/realtime", get(ws_upgrade))
        .with_state(cfg)
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(cfg): State<Config>,
) -> axum::response::Response {
    let model = params
        .get("model")
        .cloned()
        .unwrap_or_else(|| "voxtral-realtime".into());
    ws.on_upgrade(move |socket| protocol::session(socket, cfg, model))
}
