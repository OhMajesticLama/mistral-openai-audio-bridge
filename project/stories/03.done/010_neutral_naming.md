---
id: 010
title: Keep voxtral out of configuration names
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **bridge operator**,
I want **the configuration surface (environment variables, dump directory, docs) to name the bridge, not a specific transcription backend**,
so that **the bridge is what it claims to be — a generic proxy between the Mistral WS protocol and OpenAI `/v1/audio/transcriptions` — and my configuration reads consistently**.

## Acceptance criteria

- [x] The debug-dump env var is `VIBE_BRIDGE_DEBUG_DUMP`, joining the `VIBE_BRIDGE_*` family; no `VOXTRAL_*` variables remain.
      Test: src/cli.rs env attributes; tests/system/cli.rs dump tests
- [x] The default dump directory is `/tmp/vibe-audio-bridge-debug`.
      Test: tests/unit/config.rs
- [x] The default model stays `voxtral-realtime` — the one place the backend's name is relevant, since it is the model the server serves.
      Test: tests/integration/protocol.rs::test_session_created_on_connect
- [x] Docs and comments say "transcription server", not a backend brand.
      Test: docs/architecture.md, README.md

## Notes

- Implemented retroactively alongside the clap env-handling refactor; criteria verified on 2026-09-27 (all tests green). Enters `done/` directly.
- Breaking for existing deployments: `VOXTRAL_DEBUG_DUMP=1` must become `VIBE_BRIDGE_DEBUG_DUMP=1` (the known case is the systemd debug drop-in, tracked in US002).
- Integration-test model fixtures were renamed to `test-model` (arbitrary against the mock); the live-server system test keeps `voxtral-realtime` because the real backend rejects unknown model ids.
