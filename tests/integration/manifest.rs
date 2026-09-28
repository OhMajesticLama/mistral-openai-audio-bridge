// Story: 012 — guard: Cargo.toml [[test]] targets must match the tests/ tree.
// Cargo auto-discovers only top-level tests/*.rs; a file in a subfolder
// without a [[test]] entry is silently never compiled or run. These tests
// fail on such gaps, on stale entries, and on duplicate target names.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Parsed [[test]] targets: (name, path) pairs from Cargo.toml.
fn test_targets() -> Vec<(String, PathBuf)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))
        .expect("Cargo.toml readable next to the test");
    let parsed: toml::Value = toml::from_str(&raw).expect("Cargo.toml parses as TOML");
    parsed["test"]
        .as_array()
        .expect("[[test]] entries parse as an array")
        .iter()
        .map(|t| {
            let name = t["name"].as_str().expect("[[test]] name").to_string();
            let path = t["path"].as_str().expect("[[test]] path").to_string();
            (name, manifest_dir.join(path))
        })
        .collect()
}

/// Every .rs file under tests/, excluding tests/fixtures/ (data, not code).
fn test_files() -> Vec<PathBuf> {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = Vec::new();
    let mut stack = vec![tests_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("tests/ readable") {
            let entry = entry.expect("dir entry readable").path();
            if entry.is_dir() {
                if entry.file_name().and_then(|n| n.to_str()) != Some("fixtures") {
                    stack.push(entry);
                }
            } else if entry.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(entry);
            }
        }
    }
    files
}

#[test]
fn test_every_test_file_is_declared() {
    let declared: HashSet<PathBuf> = test_targets().iter().map(|(_, p)| p.clone()).collect();
    let undeclared: Vec<String> = test_files()
        .iter()
        .filter(|f| !declared.contains(*f))
        .map(|f| f.display().to_string())
        .collect();
    assert!(
        undeclared.is_empty(),
        "test files not declared as [[test]] in Cargo.toml (cargo test silently \
         skips them): {undeclared:?}"
    );
}

#[test]
fn test_every_declared_path_exists() {
    let missing: Vec<String> = test_targets()
        .iter()
        .filter(|(_, p)| !p.is_file())
        .map(|(name, p)| format!("{name} -> {}", p.display()))
        .collect();
    assert!(
        missing.is_empty(),
        "[[test]] entries pointing at missing files (stale after a rename or \
         delete): {missing:?}"
    );
}

#[test]
fn test_target_names_unique() {
    let mut seen = HashSet::new();
    let duplicates: Vec<String> = test_targets()
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !seen.insert(*name))
        .map(str::to_string)
        .collect();
    assert!(
        duplicates.is_empty(),
        "duplicate [[test]] names collide in target/debug/deps and shadow a \
         suite: {duplicates:?}"
    );
}
