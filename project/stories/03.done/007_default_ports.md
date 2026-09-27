---
id: 007
title: Default to audio.cpp's port scheme (8080 upstream, 8081 listen) on loopback
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **bridge operator**,
I want **the bridge's out-of-the-box defaults to match audio.cpp's default port (8080 upstream) and to listen on 8081 bound to loopback only**,
so that **it works with a default audio.cpp setup without writing any configuration, and is never exposed on my network by default**.

## Acceptance criteria

- [x] With no flags or environment variables, the bridge listens on `127.0.0.1:8081`.
      Test: tests/unit/cli.rs::test_defaults_without_env
- [x] With no flags or environment variables, the bridge forwards to `http://127.0.0.1:8080` (audio.cpp's default port).
      Test: tests/unit/cli.rs::test_defaults_without_env
- [x] A no-argument run uses these defaults (CLI > env > defaults chain ends at the new values).
      Test: tests/unit/cli.rs::test_defaults_without_env
- [x] `--help` shows the new defaults in the environment hints.
      Test: tests/system/cli.rs::test_help_prints_usage_and_exits_zero

## Notes

- Previous defaults (`127.0.0.1:9932` / `http://127.0.0.1:9931`) matched the Python prototype's ports. Deployments still using them must set `VIBE_BRIDGE_LISTEN` / `VIBE_BRIDGE_UPSTREAM` (or the equivalent flags) or move their client to the new port.
- Loopback-only by default is the safe choice: `0.0.0.0` would expose the unauthenticated WebSocket endpoint on every network interface.
- US001's criterion recorded the old defaults; a note there points at this story so the historical record and the current test don't contradict each other.
