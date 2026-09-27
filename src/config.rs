// Runtime configuration, built from the parsed CLI. clap has already merged
// environment variables and defaults into it (CLI > env > defaults).

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::Cli;

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub upstream: String,
    pub log_level: String,
    pub dump_dir: Option<PathBuf>,
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Self {
        let dump_dir = if cli.debug_dump || cli.dump_dir.is_some() {
            Some(cli.dump_dir.clone().unwrap_or_else(|| PathBuf::from("/tmp/vibe-audio-bridge-debug")))
        } else {
            None
        };
        Config { listen: cli.listen, upstream: cli.upstream.clone(), log_level: cli.log_level.clone(), dump_dir }
    }
}
