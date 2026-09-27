// Story: 001/005 — Config construction from parsed CLI values.

use vibe_audio_bridge::cli::Cli;
use vibe_audio_bridge::config::Config;

fn cli() -> Cli {
    Cli {
        listen: "127.0.0.1:8081".parse().unwrap(),
        upstream: "http://127.0.0.1:8080".into(),
        log_level: "info".into(),
        debug_dump: false,
        dump_dir: None,
    }
}

#[test]
fn test_from_cli_defaults() {
    let cfg = Config::from_cli(&cli());
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:8081");
    assert_eq!(cfg.upstream, "http://127.0.0.1:8080");
    assert_eq!(cfg.log_level, "info");
    assert!(cfg.dump_dir.is_none());
}

#[test]
fn test_debug_dump_uses_default_dir() {
    let mut c = cli();
    c.debug_dump = true;
    let cfg = Config::from_cli(&c);
    assert_eq!(cfg.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/vibe-audio-bridge-debug");
}

#[test]
fn test_dump_dir_implies_debug_dump() {
    let mut c = cli();
    c.dump_dir = Some("/tmp/bridge-dumps".into());
    let cfg = Config::from_cli(&c);
    assert_eq!(cfg.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/bridge-dumps");
}
