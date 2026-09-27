---
id: 005
title: Support standard CLI arguments
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As an **operator running the bridge binary**,
I want **standard command-line arguments (`--help`, `--version`, and flags mirroring the environment configuration)**,
so that **the binary follows platform conventions instead of starting the server on every invocation, and ad-hoc runs don't require setting environment variables**.

## Acceptance criteria

- [x] `--help` (and `-h`) prints a usage text covering every option and exits with status 0; the server does not start.
      Test: tests/system/cli.rs::test_help_prints_usage_and_exits_zero
- [x] `--version` (and `-V`) prints the crate version and exits with status 0; the server does not start.
      Test: tests/system/cli.rs::test_version_prints_version_and_exits_zero
- [x] An unknown flag prints an error to stderr and exits non-zero; the server does not start.
      Test: tests/system/cli.rs::test_unknown_flag_fails_without_serving
- [x] Flags override environment variables, which override defaults, for the existing configuration surface: `--listen`, `--upstream`, `--log-level`, `--debug-dump` (implies `VOXTRAL_DEBUG_DUMP=1`), `--dump-dir`.
      Test: tests/unit/cli.rs::test_flags_override_env_and_defaults
- [x] With no arguments, behavior is unchanged from US001 (environment variables and defaults apply).
      Test: tests/unit/cli.rs::test_no_args_falls_back_to_env

## Notes

- Today even `--help` starts the server; the binary has no argument handling at all.
- Parsing belongs in the library (`src/cli.rs`, a `parse_args` function returning a small `Cli` struct) so it is unit-testable; `src/main.rs` only wires it to `Config`. The `--help`/`--version`/unknown-flag criteria are verified against the real binary via `CARGO_BIN_EXE_vibe-audio-bridge` subprocess tests.
- Likely dependency: `clap` with the `derive` feature (standard, generates help text). A hand-rolled parser is possible but error messages and help text are the product here — don't hand-roll them.
- Precedence rule: CLI > environment > defaults. The systemd unit (US002) keeps using environment variables and is unaffected.
- Deliberately out of scope: subcommands, config files, interactive mode. The bridge has one run mode.

## System test coverage (post-review)

Each config flag is also verified against the real binary
(`CARGO_BIN_EXE_vibe-audio-bridge`), not just the parse/merge unit:

- `--listen` + `--upstream`: `tests/system/cli.rs::test_system_listen_and_upstream_flags`
  (binary binds the given port and routes a full transcription to the
  given upstream).
- `--debug-dump` + `--dump-dir`: `tests/system/cli.rs::test_system_debug_dump_flags`
  (dump file written with the exact sent PCM).
- `--log-level`: `tests/system/cli.rs::test_system_log_level_flag`
  (with `error`, no INFO output is produced).
- `--help`/`--version`/unknown-flag were already subprocess tests.
