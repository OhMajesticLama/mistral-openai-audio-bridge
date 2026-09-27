use vibe_audio_bridge::config::Config;
use vibe_audio_bridge::router;

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(&cfg.log_level))
        .init();

    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {}: {e}", cfg.listen));
    tracing::info!(
        "listening on ws://{}/v1/audio/transcriptions/realtime -> {}",
        cfg.listen,
        cfg.upstream
    );
    axum::serve(listener, router(cfg)).await.unwrap();
}
