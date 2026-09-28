---
id: 012
title: Guard test: Cargo.toml test targets match the tests/ tree
status: done
epic: test-infra
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
done_date:
---

## Story

As a **Contributor**,
I want **a test that fails when a test file under `tests/` is not declared as a `[[test]]` target in `Cargo.toml` (or a declared target points to a missing file)**,
so that **a new test file can never be silently skipped by `cargo test` — an unnoticed gap that quietly reduces coverage while the suite stays green**.

## Acceptance criteria

- [x] Every `.rs` file under `tests/` (excluding `tests/fixtures/`) is declared as a `[[test]]` target `path` in `Cargo.toml`. Cargo only auto-discovers top-level `tests/*.rs`, so an undeclared file in a subfolder is never compiled or run — the test fails listing the undeclared files.
      Test: tests/integration/manifest.rs::test_every_test_file_is_declared
- [x] Every declared `[[test]]` `path` points to an existing file — a stale entry after a rename or delete fails with the offending path.
      Test: tests/integration/manifest.rs::test_every_declared_path_exists
- [x] `[[test]]` target names are unique — duplicate names make Cargo collide in `target/debug/deps` and silently shadow one suite.
      Test: tests/integration/manifest.rs::test_target_names_unique
- [x]  The integration test check runs with every `cargo test` to check tests consistency.
      Test: tests/integration/manifest.rs (declared as the `integration_manifest` target, runs by default with `cargo test`)
- [x] Definition of Done met.

## Notes

- The guard test is itself a `[[test]]` target and must be declared in the same commit that adds it — an undeclared guard does not run, which is exactly the failure mode it guards against. Criterion 1 covers it like any other file.
- Integration level: it reads `Cargo.toml` and walks `tests/` from the crate root (`CARGO_MANIFEST_DIR`), no network, no server, runs in milliseconds.
- Parse `Cargo.toml` with the `toml` crate (dev-dependency) rather than hand-rolled line matching — `[[test]]` arrays are trivial to extract as `toml::Value::Array`, and hand-parsing invites false positives on comments or nested tables.
- Known limitation: the check runs only when the guard target itself is declared and compiled. It cannot catch the case where someone deletes its own `[[test]]` entry — that is undetectable from inside the suite by construction.
- Alternative rejected: a `build.rs` failing the build on mismatch. Harder to test, runs on every build, and the story workflow prefers a testable acceptance criterion under `tests/`.
