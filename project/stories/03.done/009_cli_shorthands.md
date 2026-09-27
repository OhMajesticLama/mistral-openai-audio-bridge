---
id: 009
title: Add shorthand flags for CLI options
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **bridge operator**,
I want **single-character shorthands for the CLI options**,
so that **repeated manual runs from the terminal are faster to type and easier to iterate on while debugging**.

## Acceptance criteria

- [x] Every option has a shorthand: `-l`/`--listen`, `-u`/`--upstream`, `-d`/`--debug-dump`, `-D`/`--dump-dir`; `--log-level` keeps no shorthand (`-v` is reserved by `-V`/`--version`).
      Test: tests/system/cli.rs::test_help_prints_usage_and_exits_zero
- [x] A shorthand parses to the same `Cli` value as its long form.
      Test: tests/unit/cli.rs::test_shorthands_match_long_forms
- [x] Long forms are unchanged; existing invocations keep working.
      Test: tests/unit/cli.rs::test_flags_override_env_and_defaults (existing, must stay green)

## Notes

- clap: add `short` to the `#[arg]` attributes; no new dependencies.
- Deliberately no shorthand for `--log-level`: the natural `-v`/`-l` collisions make it worse, and it is set once per deployment, not iterated on.
