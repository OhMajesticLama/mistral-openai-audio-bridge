---
id: 001
title: Serve Vibe realtime transcription protocol over WebSocket
status: done
epic: rust-bridge
estimate: M
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **Vibe voice-mode user**,
I want **the bridge (as a Rust service) to accept the Mistral realtime transcription WebSocket protocol and forward audio to a v1/audio/transcription HTTP endpoint**,
so that **voice dictation transcribes through my local voxtral server running audio.cpp**.

## Acceptance criteria

- [x] On WebSocket connect, the bridge sends `session.created` with the `model` query parameter (default `voxtral-realtime`) and `audio_format` `pcm_s16le` / 16 kHz.
      Test: tests/integration/protocol.rs::test_session_created_on_connect
- [x] `input_audio.append` payloads are base64-decoded and buffered; on `input_audio.end` the buffered PCM is wrapped in a WAV header (pcm_s16le, 16 kHz, mono) and POSTed to the upstream `/v1/audio/transcriptions` with `stream=true`.
      Test: tests/integration/protocol.rs::test_append_then_end_posts_wav_upstream
- [x] Upstream SSE `transcript.text.delta` events are forwarded to the client as `transcription.text.delta`, and `transcript.text.done` as `transcription.done` carrying the final text.
      Test: tests/integration/protocol.rs::test_upstream_deltas_forwarded_as_transcription_events
- [x] An `input_audio.end` with an empty buffer answers `transcription.done` with empty text and makes no upstream call.
      Test: tests/integration/protocol.rs::test_empty_recording_short_circuits
- [x] A non-200 upstream response is reported to the client as a protocol `error` event including the status code and a truncated body.
      Test: tests/integration/protocol.rs::test_upstream_error_reported_to_client
- [x] Listen address, upstream URL, and log level are configurable via environment variables with defaults matching the Python bridge (127.0.0.1:9932, http://127.0.0.1:9931, info).
      Test: tests/system/cli.rs::test_system_env_configures_and_cli_overrides; tests/unit/cli.rs::test_defaults_without_env
      Note: defaults later changed to 127.0.0.1:8081 / http://127.0.0.1:8080 (audio.cpp's default port); the tests assert the current defaults.
- [x] Each transcription logs the byte count, approximate duration, and peak sample level; `VOXTRAL_DEBUG_DUMP=1` dumps the raw PCM under `/tmp/voxtral-debug/`.
      Test: tests/integration/protocol.rs::test_debug_dump_written
- [x] Definition of Done met.

## Notes

- Port of `voxtral-bridge.py` (commit d45923a); behavior must be protocol-identical.
- The WebSocket endpoint is `/v1/audio/transcriptions/realtime` — Vibe's voice mode requires it; the REST side is only the upstream call.
- Likely dependencies: axum (WS + server), reqwest (upstream), serde/serde_json, base64, tracing.
- Tests use a mock upstream HTTP server fixture; no live voxtral server needed in tests.
