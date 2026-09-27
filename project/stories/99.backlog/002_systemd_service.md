---
id: 002
title: Run the bridge as a systemd user service
status: planned
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date:
done_date:
---

## Story

As a **user of Vibe voice mode**,
I want **the Rust bridge to start automatically as a systemd user service**,
so that **voice dictation works after login without manually launching the bridge**.

## Acceptance criteria

- [ ] The repo's unit file `ExecStart` points at the built Rust binary (not the Python script), with `Restart=on-failure`.
      Test: tests/deployment.rs::test_unit_execstart_targets_built_binary
- [ ] `cargo build --release` produces the binary at the path the unit file references.
      Test: tests/deployment.rs::test_release_binary_exists_at_unit_path
- [ ] `docs/architecture.md` describes the bridge components (WS endpoint, upstream client, WAV wrapping, diagnostics) and the deployment path.
      Test: manual review (documentation criterion)
- [ ] Definition of Done met.

## Notes

- Replaces the currently installed `~/.config/systemd/user/voxtral-bridge.service`, which still points at `/usr/bin/python3 .../voxtral-bridge.py`.
- Depends on story 001.
- The debug drop-in (`voxtral-bridge.service.d/debug.conf` with `VOXTRAL_DEBUG_DUMP=1`) stays as-is.
