// Story: 005/009 — CLI parsing unit tests (no process spawning).
// Environment lookup is stripped so tests do not read the host environment;
// env precedence is covered against the real binary in tests/system/cli.rs.

use clap::{CommandFactory, FromArgMatches};
use vibe_audio_bridge::cli::Cli;

/// The clap command with env lookup disabled, for hermetic parsing.
fn cmd() -> clap::Command {
    let mut cmd = Cli::command();
    for id in ["listen", "upstream", "log_level", "debug_dump", "dump_dir", "flush_interval_ms"] {
        cmd = cmd.mut_arg(id, |a| a.env(None));
    }
    cmd
}

fn parse(args: &[&str]) -> Cli {
    Cli::from_arg_matches(&cmd().try_get_matches_from(args.iter().copied()).unwrap()).unwrap()
}

#[test]
fn test_flags_parse() {
    let cli = parse(&[
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
    ]);
    assert_eq!(cli.listen.to_string(), "127.0.0.1:9955");
    assert_eq!(cli.upstream, "http://127.0.0.1:9999");
    assert_eq!(cli.log_level, "debug");
    assert!(cli.debug_dump);
    assert_eq!(cli.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/bridge-cli-dumps");
}

#[test]
fn test_defaults_without_env() {
    let cli = parse(&["vibe-audio-bridge"]);
    assert_eq!(cli.listen.to_string(), "127.0.0.1:8081");
    assert_eq!(cli.upstream, "http://127.0.0.1:8080");
    assert_eq!(cli.log_level, "info");
    assert!(!cli.debug_dump);
    assert!(cli.dump_dir.is_none());
}

#[test]
fn test_shorthands_match_long_forms() {
    // Story: 009 — shorthands parse to the same Cli as their long forms.
    let short = parse(&[
        "vibe-audio-bridge",
        "-l",
        "127.0.0.1:9955",
        "-u",
        "http://127.0.0.1:9999",
        "-d",
        "-D",
        "/tmp/bridge-cli-dumps",
    ]);
    let long = parse(&[
        "vibe-audio-bridge",
        "--listen",
        "127.0.0.1:9955",
        "--upstream",
        "http://127.0.0.1:9999",
        "--debug-dump",
        "--dump-dir",
        "/tmp/bridge-cli-dumps",
    ]);
    assert_eq!(format!("{short:?}"), format!("{long:?}"));
}

#[test]
fn test_flush_interval_default_and_flag() {
    // Story: 011 — streaming flush interval: default 1000 ms, 0 disables streaming.
    let cli = parse(&["vibe-audio-bridge"]);
    assert_eq!(cli.flush_interval_ms, 1000);
    let cli = parse(&["vibe-audio-bridge", "--flush-interval-ms", "250"]);
    assert_eq!(cli.flush_interval_ms, 250);
    let cli = parse(&["vibe-audio-bridge", "--flush-interval-ms", "0"]);
    assert_eq!(cli.flush_interval_ms, 0);
}

#[test]
fn test_invalid_listen_address_rejected() {
    let err = cmd()
        .try_get_matches_from(["vibe-audio-bridge", "--listen", "not-an-address"])
        .unwrap_err();
    assert!(err.to_string().contains("invalid value"), "{err}");
}

#[test]
fn test_invalid_upstream_rejected() {
    // Regression: a scheme-less or empty upstream used to surface per-request
    // as "bridge error: builder error"; it must fail at startup instead.
    for bad in ["127.0.0.1:9931", ""] {
        let err = cmd()
            .try_get_matches_from(vec!["vibe-audio-bridge", "--upstream", bad])
            .unwrap_err();
        assert!(err.to_string().contains("http(s)"), "upstream {bad:?} accepted: {err}");
    }
}
