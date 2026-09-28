---
id: 011
title: Stream transcription deltas during recording
status: done
epic: rust-bridge
estimate: M
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story
<!-- One capability per story. If the "I want" clause has an "and", split it. -->
<!-- The "so that" clause is mandatory — no stated value, no story. -->
<!-- The role comes from project/personas.md (Speaker, Operator, Contributor). -->

As a **voice-mode user**,
I want **transcription text to appear while I am still speaking, not only after I stop the recording**,
so that **I can see my dictation live and know the bridge heard me before committing**.

## Acceptance criteria
<!-- Each criterion names the test that verifies it, in whatever form -->
<!-- the test framework uses to identify a test. -->
<!-- A criterion without a test reference is not done — it's not even a criterion. -->
<!-- Write the test first (failing), then implement until it passes. -->
<!-- Test files are organized by module, not by story. A test references -->
<!-- its story via a comment or docstring (e.g. # Story: NNN). -->

- [x] While audio is being appended and no `input_audio.end` has been sent, the bridge periodically flushes the accumulated buffer to the upstream and forwards the resulting text to the client as `transcription.text.delta` events.
      Test: tests/integration/protocol.rs::test_deltas_stream_before_input_audio_end (already written, currently failing)
- [x] Deltas concatenate to the final text: each flush emits only the suffix that is new since the previous flush (the upstream always receives the full buffer, so the bridge must diff against the last emitted transcript, not re-send it).
      Test: tests/integration/protocol.rs::test_streaming_deltas_concatenate_to_done (to write)
- [x] `input_audio.end` still emits the final `transcription.done` with the full recording's text, and the buffer and diff state reset for the next recording in the same session.
      Test: tests/integration/protocol.rs::test_two_recordings_in_one_session (existing, must keep passing)
- [x] The flush interval is configurable via `--flush-interval-ms` / `VIBE_BRIDGE_FLUSH_INTERVAL_MS`, with a default of 1000 ms; `0` disables streaming flushes (batch behavior of story 001).
      Test: tests/unit/cli.rs::test_flush_interval_default_and_flag (default + flag); tests/system/cli.rs::test_system_flush_interval_env_zero_disables_streaming (env + 0-disables, against the real binary — clap reads the process-global environment, which is racy to test in parallel in-process tests)
- [x] **Real-time transcription for audio up to at least 2 minutes****: during a 120 s recording streamed at real-time pace, deltas keep arriving while the speaker is still talking, and the final `transcription.done` arrives within a bounded time after `input_audio.end` (the upstream's processing tail, not a term growing with recording length).
      Test: tests/system/transcription.rs::test_two_minute_realtime_recording (against the live server, feature-gated `test-long`, ~2 min wall time); tests/system/transcription.rs::test_live_processing_is_incremental (fast ~2 s guard: consecutive 500 ms chunks must not process slower than 1.5x, `--ignored`)
- [x] Definition of Done met.

## Notes
<!-- Context, constraints, references to data/scripts, open questions. -->

- Root cause of the bug: the bridge is batch-only by design (story 001,
  `src/protocol.rs`: "buffer appends, transcribe on end"). Vibe's voice mode
  displays each `transcription.text.delta` as it arrives
  (`voice_manager.py` `TranscribeTextDelta`), so nothing can appear before
  the client sends `input_audio.end`.
- The upstream is a batch `/v1/audio/transcriptions` endpoint: it needs the
  whole recording. Streaming is therefore approximate — each flush
  re-transcribes the full accumulated buffer. The suffix-diff rule above
  is what keeps the client-side concatenation correct.
- ponytail: O(n²) audio re-sent over a session (each flush re-uploads the
  whole buffer). Fine for dictation-length recordings; switch to chunked
  incremental requests if a real upstream ever supports them.
- The flush interval is a calibration knob, not a constant: real servers
  have real latency, and 1000 ms is a guess. Leave it configurable.
- Open question: should a flush be triggered by detected silence as well?
  Deferred — time-based flush is the minimum that satisfies the story.

## Reopened (2026-09-27)

- The 2-minute criterion is added after measuring the flush design: a 120 s
  recording costs ~14 s per flush (upstream T(D) ~ 0.35 + 0.115*D), lags
  ~15 s behind speech, and makes the backend spend ~830 s re-transcribing
  120 s of audio. The flush approach cannot satisfy the criterion.
- Verified path forward: the backend exposes
  `POST /v1/audio/transcriptions/live` — chunked raw PCM in, transcript
  deltas out on the same connection, incremental (tested against the
  deployed server: correct transcription, same SSE shape). Routing the
  mid-recording deltas through the live endpoint (keeping the flush path
  as fallback for backends without the route) is the likely implementation.
