// Configuration from environment variables, with Python-bridge defaults.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub upstream: String,
    pub log_level: String,
    pub dump_dir: Option<PathBuf>,
}

impl Config {
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let listen = match lookup("VIBE_BRIDGE_LISTEN") {
            Some(v) => v.parse().unwrap_or_else(|_| panic!("invalid VIBE_BRIDGE_LISTEN: {v}")),
            None => SocketAddr::from(([127, 0, 0, 1], 8081)),
        };
        let upstream =
            lookup("VIBE_BRIDGE_UPSTREAM").unwrap_or_else(|| "http://127.0.0.1:8080".into());
        let log_level = lookup("VIBE_BRIDGE_LOG_LEVEL").unwrap_or_else(|| "info".into());
        let dump_dir = if lookup("VOXTRAL_DEBUG_DUMP").as_deref() == Some("1") {
            Some(PathBuf::from(
                lookup("VIBE_BRIDGE_DUMP_DIR").unwrap_or_else(|| "/tmp/voxtral-debug".into()),
            ))
        } else {
            None
        };
        Config { listen, upstream, log_level, dump_dir }
    }

    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }
}
