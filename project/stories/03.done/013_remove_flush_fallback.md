---
id: 013
title: Remove the flush fallback and the flush-interval parameter
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-28
---

## Story

As a **Contributor**,
I want **the bridge to rely solely on the upstream's live-ingest route for streaming deltas, with batch transcription on `input_audio.end` as the only fallback, and no flush-interval configuration**,
so that **the code carries no O(n²) re-transcription path that no real upstream needs, and no parameter that pretends to tune latency it cannot control**.

## Acceptance criteria

- [x] On the first `input_audio.append` the bridge always attempts the live-ingest route; there is no periodic flush path (no ticker, no suffix-diff emission).
      Test: tests/integration/protocol.rs::test_live_endpoint_streams_deltas (existing, must keep passing)
- [x] An upstream without a live route gets batch behavior: no events during recording, one batch transcription on `input_audio.end`.
      Test: tests/integration/protocol.rs::test_batch_upstream_silent_until_end (new; fails against the current flush behavior)
- [x] `--flush-interval-ms` / `VIBE_BRIDGE_FLUSH_INTERVAL_MS` no longer exist in the CLI, `Config`, README, or architecture doc.
      Test: tests/unit/cli.rs::test_flags_parse (updated field list); tests/system/cli.rs::test_help_prints_usage_and_exits_zero (usage must not mention it)
- [x] Definition of Done met.

## Notes

- Decision: there is no sane way to simulate live transcription against a backend that does not support it — the flush fallback re-transcribed the whole buffer per tick (O(n²)), produced mid-word deltas and duplicated-word artifacts on revisions, and only ever fired for hypothetical non-live upstreams.
- The flush-interval value paced nothing on the live path (deltas arrive at the upstream's pace); after removal the parameter carries zero information, so it goes away entirely rather than becoming a misleading no-op.
- `input_audio.flush` client messages remain ignored (parse returns None) — clients may still send them.
- Removed with the fallback: `emit_suffix`, the `emitted` diff state, the flush ticker, `test_deltas_stream_before_input_audio_end`, `test_streaming_deltas_concatenate_to_done`, `test_flush_interval_default_and_flag`, `test_system_flush_interval_env_zero_disables_streaming`.
- Also removed: `test_upstream_deltas_forwarded_as_transcription_events` (asserted a delta from the batch path, contradicting the new batch semantics; delta forwarding is covered by `test_live_endpoint_streams_deltas`) and the orphaned `spawn_upstream_seq` mock with its `SSE_BON`/`SSE_BONJOUR` bodies (only the flush tests used them).
