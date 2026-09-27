// CLI arguments: clap-derived flags merged over environment configuration.
// Precedence: CLI > environment > defaults.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

use crate::config::Config;

#[derive(Debug, Parser)]
#[command(
    name = "vibe-audio-bridge",
    version,
    about = "Vibe voice-mode bridge: Mistral realtime transcription WebSocket protocol -> OpenAI-compatible /v1/audio/transcriptions endpoint"
)]
pub struct Cli {
    /// Address to listen on [env: VIBE_BRIDGE_LISTEN=127.0.0.1:8081]
    #[arg(long)]
    pub listen: Option<SocketAddr>,

    /// Upstream transcription server base URL [env: VIBE_BRIDGE_UPSTREAM=http://127.0.0.1:8080]
    #[arg(long)]
    pub upstream: Option<String>,

    /// Log level (error|warn|info|debug|trace) [env: VIBE_BRIDGE_LOG_LEVEL=info]
    #[arg(long)]
    pub log_level: Option<String>,

    /// Dump the raw PCM of each recording for debugging
    #[arg(long)]
    pub debug_dump: bool,

    /// Directory for debug dumps (implies --debug-dump) [default: /tmp/voxtral-debug]
    #[arg(long)]
    pub dump_dir: Option<PathBuf>,
}

/// Merge CLI flags over the environment lookup into a Config.
pub fn config_from(cli: &Cli, env: impl Fn(&str) -> Option<String>) -> Config {
    Config::from_lookup(|key| {
        let from_cli = match key {
            "VIBE_BRIDGE_LISTEN" => cli.listen.map(|a| a.to_string()),
            "VIBE_BRIDGE_UPSTREAM" => cli.upstream.clone(),
            "VIBE_BRIDGE_LOG_LEVEL" => cli.log_level.clone(),
            "VOXTRAL_DEBUG_DUMP" => {
                if cli.debug_dump || cli.dump_dir.is_some() {
                    Some("1".to_string())
                } else {
                    None
                }
            }
            "VIBE_BRIDGE_DUMP_DIR" => cli
                .dump_dir
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            _ => None,
        };
        from_cli.or_else(|| env(key))
    })
}
