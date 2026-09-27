// Story: 005 — CLI parse/merge unit tests (no process spawning).

use clap::Parser;
use vibe_audio_bridge::cli::{self, Cli};
use vibe_audio_bridge::config::Config;

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn test_flags_override_env_and_defaults() {
    let cli = Cli::try_parse_from([
        "vibe-audio-bridge",
        "--listen",
        "127.0.0.1:9955",
        "--upstream",
        "http://127.0.0.1:9999",
        "--log-level",
        "debug",
        "--debug-dump",
        "--dump-dir",
        "/tmp/bridge-cli-dumps",
    ])
    .unwrap();

    // CLI wins over a conflicting environment.
    let env = |k: &str| {
        match k {
            "VIBE_BRIDGE_LISTEN" => Some("127.0.0.1:1".to_string()),
            "VIBE_BRIDGE_UPSTREAM" => Some("http://127.0.0.1:2".to_string()),
            "VIBE_BRIDGE_LOG_LEVEL" => Some("warn".to_string()),
            "VOXTRAL_DEBUG_DUMP" => Some("0".to_string()),
            _ => None,
        }
    };
    let cfg = cli::config_from(&cli, env);
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:9955");
    assert_eq!(cfg.upstream, "http://127.0.0.1:9999");
    assert_eq!(cfg.log_level, "debug");
    let dump_dir = cfg.dump_dir.expect("--debug-dump must enable dumps");
    assert_eq!(dump_dir.to_str().unwrap(), "/tmp/bridge-cli-dumps");

    // Same CLI, no environment: CLI still wins over defaults.
    let cfg = cli::config_from(&cli, no_env);
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:9955");
    assert!(cfg.dump_dir.is_some());
}

#[test]
fn test_dump_dir_alone_enables_dumps() {
    let cli = Cli::try_parse_from(["vibe-audio-bridge", "--dump-dir", "/tmp/d"])
        .unwrap();
    let cfg = cli::config_from(&cli, no_env);
    assert_eq!(cfg.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/d");
}

#[test]
fn test_no_args_falls_back_to_env() {
    let cli = Cli::try_parse_from(["vibe-audio-bridge"]).unwrap();

    // Environment provides the values.
    let env = |k: &str| {
        match k {
            "VIBE_BRIDGE_LISTEN" => Some("127.0.0.1:9966".to_string()),
            "VIBE_BRIDGE_UPSTREAM" => Some("http://127.0.0.1:9977".to_string()),
            "VIBE_BRIDGE_LOG_LEVEL" => Some("trace".to_string()),
            _ => None,
        }
    };
    let cfg = cli::config_from(&cli, env);
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:9966");
    assert_eq!(cfg.upstream, "http://127.0.0.1:9977");
    assert_eq!(cfg.log_level, "trace");

    // No CLI, no environment: US001 defaults, unchanged.
    let cfg = cli::config_from(&cli, no_env);
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:8081");
    assert_eq!(cfg.upstream, "http://127.0.0.1:8080");
    assert_eq!(cfg.log_level, "info");
    assert!(cfg.dump_dir.is_none());
    assert_eq!(Config::from_lookup(no_env).listen, cfg.listen);
}
