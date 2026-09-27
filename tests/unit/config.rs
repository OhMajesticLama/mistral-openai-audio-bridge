// Story: 001 — configuration from environment variables.

use std::collections::HashMap;

use vibe_audio_bridge::config::Config;

fn lookup_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k| map.get(k).cloned()
}

#[test]
fn test_env_overrides_defaults() {
    let cfg = Config::from_lookup(lookup_from(&[]));
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:9932");
    assert_eq!(cfg.upstream, "http://127.0.0.1:9931");
    assert!(cfg.dump_dir.is_none());

    let cfg = Config::from_lookup(lookup_from(&[
        ("VIBE_BRIDGE_LISTEN", "127.0.0.1:9944"),
        ("VIBE_BRIDGE_UPSTREAM", "http://127.0.0.1:9999"),
        ("VOXTRAL_DEBUG_DUMP", "1"),
    ]));
    assert_eq!(cfg.listen.to_string(), "127.0.0.1:9944");
    assert_eq!(cfg.upstream, "http://127.0.0.1:9999");
    assert_eq!(cfg.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/voxtral-debug");
}

#[test]
fn test_dump_dir_env_selects_directory() {
    let cfg = Config::from_lookup(lookup_from(&[
        ("VOXTRAL_DEBUG_DUMP", "1"),
        ("VIBE_BRIDGE_DUMP_DIR", "/tmp/bridge-dumps"),
    ]));
    assert_eq!(cfg.dump_dir.as_deref().unwrap().to_str().unwrap(), "/tmp/bridge-dumps");

    // Dump disabled: dump dir stays unset even if the dir var is given.
    let cfg = Config::from_lookup(lookup_from(&[("VIBE_BRIDGE_DUMP_DIR", "/tmp/bridge-dumps")]));
    assert!(cfg.dump_dir.is_none());
}

#[test]
#[should_panic(expected = "invalid VIBE_BRIDGE_LISTEN")]
fn test_invalid_listen_address_panics_loudly() {
    let _ = Config::from_lookup(lookup_from(&[("VIBE_BRIDGE_LISTEN", "not-an-address")]));
}
