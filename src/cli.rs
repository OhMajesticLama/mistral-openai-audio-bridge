// CLI arguments: clap-derived flags with environment-variable fallbacks.
// Precedence: CLI > environment > defaults, applied by clap itself.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

/// Upstream must be an http(s) URL; anything else fails at argument parsing.
fn http_url(s: &str) -> Result<String, String> {
    match reqwest::Url::parse(s) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(s.to_string()),
        _ => Err(format!("invalid upstream {s:?}: must be an http(s) URL")),
    }
}

/// Accepts the usual truthy/falsy spellings for VIBE_BRIDGE_DEBUG_DUMP ("1" is
/// the documented value; clap's default bool parser rejects it).
fn boolish(s: &str) -> Result<bool, String> {
    match s.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" | "" => Ok(false),
        _ => Err(format!("invalid boolean {s:?}")),
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "vibe-audio-bridge",
    version,
    about = "Vibe voice-mode bridge: Mistral realtime transcription WebSocket protocol -> OpenAI-compatible /v1/audio/transcriptions endpoint"
)]
pub struct Cli {
    /// Address to listen on
    #[arg(long, short = 'l', env = "VIBE_BRIDGE_LISTEN", default_value = "127.0.0.1:8081")]
    pub listen: SocketAddr,

    /// Upstream transcription server base URL
    #[arg(long, short = 'u', env = "VIBE_BRIDGE_UPSTREAM", default_value = "http://127.0.0.1:8080", value_parser = http_url)]
    pub upstream: String,

    /// Log level (error|warn|info|debug|trace)
    #[arg(long, env = "VIBE_BRIDGE_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Dump the raw PCM of each recording for debugging
    #[arg(long, short = 'd', env = "VIBE_BRIDGE_DEBUG_DUMP", value_parser = boolish)]
    pub debug_dump: bool,

    /// Directory for debug dumps (implies --debug-dump) [default: /tmp/vibe-audio-bridge-debug]
    #[arg(long, short = 'D', env = "VIBE_BRIDGE_DUMP_DIR")]
    pub dump_dir: Option<PathBuf>,
}
