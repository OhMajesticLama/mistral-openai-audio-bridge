---
id: 006
title: Adopt a leveled test strategy (unit, integration, system)
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **voice-mode user**,
I want **every change to the bridge verified automatically at unit, integration, and system levels — including against a real transcription server like mine — before it reaches me**,
so that **my speech transcribes reliably every time: my words are not lost, mangled, or silently dropped, and each new version of the bridge works the same as the last**.

## Acceptance criteria

- [x] The pieces my dictation depends on are proven in isolation: WAV wrapping (my audio must arrive as valid 16 kHz PCM), configuration (my `VIBE_BRIDGE_*` settings must be honored), and CLI parsing — under `tests/unit/`, one file per module.
      Test: `cargo test --test unit_wav` (3), `cargo test --test unit_config` (3), `cargo test --test unit_cli` (3)
- [x] A whole dictation session is proven end-to-end against a mock transcription server: connect, stream audio, receive my transcript — under `tests/integration/`, no dependence on my environment.
      Test: `cargo test --test integration_protocol` (9)
- [x] The artifacts I actually run are proven for real: the shipped binary honors every CLI argument, and a live voxtral server transcribes real speech through the bridge — under `tests/system/`, the live-server test `--ignored` so it runs against *my* server when I choose.
      Test: `cargo test --test system_cli` (6), `cargo test --test system_transcription -- --ignored` (1)
- [x] One command shows the whole picture: every test file is a declared `[[test]]` target in `Cargo.toml` and `cargo test` runs all levels green, so "does this version work?" always has an answer.
      Test: `cargo test` (25 passed, 0 failed, 1 ignored)
- [x] The strategy is written down where it stays true: `AGENTS.md` (testing section) and `docs/architecture.md` (Tests section) describe the level split, the subfolder layout, and the `[[test]]` requirement.
      Test: docs/architecture.md#tests; AGENTS.md#testing

## Notes

- Implemented retroactively: the restructure landed during the US005 review (splitting the mixed `tests/cli.rs` by level) before this story was written; criteria were verified on 2026-09-27 and the story enters `done/` directly.
- Value chain from user to test level:
  - **unit** — my audio is wrapped correctly and my settings are honored; failures here never reach my microphone.
  - **integration** — a dictation session works as a whole; failures mean the protocol broke, not my setup.
  - **system** — the binary I run, with the arguments I pass, against a server like mine; failures mean "this version would have failed on my machine."
- Level boundaries, for future stories:
  - **unit** — one module, in isolation. New module `foo` gets `tests/unit/foo.rs`.
  - **integration** — several modules together, in-process, external services mocked (axum mock upstream on an ephemeral port).
  - **system** — the real binary or the real server. Slow or environment-dependent tests are `#[ignore]`d with a run hint.
- Shared fixtures live in `tests/fixtures/` (e.g. `bonjour.pcm`, the 16 kHz s16le "Bonjour, comment ça va?" recording); paths are relative to the package root, which `cargo test` guarantees as cwd.
- A test file may contain tests for multiple stories; each test references its story in a header comment (e.g. `// Story: 005`).
