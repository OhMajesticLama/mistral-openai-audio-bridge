---
id: 004
title: Expose a REST transcription passthrough endpoint
status: backlog
epic: rest-api
estimate: S
priority: low
created: 2026-09-27
in_progress_date:
done_date:
---

## Story

As a **client other than Vibe voice mode**,
I want **the bridge to expose `POST /v1/audio/transcriptions` as a plain REST endpoint that forwards to the upstream**,
so that **scripts and tools can transcribe audio files through the same bridge without speaking the WebSocket protocol**.

## Acceptance criteria

- [ ] `POST /v1/audio/transcriptions` with multipart form data (`file`, `model`, `stream`) returns the upstream's response (SSE when `stream=true`, JSON otherwise).
      Test: tests/rest.rs::test_passthrough_forwards_to_upstream
- [ ] An upstream failure is surfaced with the upstream status code.
      Test: tests/rest.rs::test_passthrough_propagates_upstream_error

## Notes

- Optional scope extension from the "proper REST package" goal; the WebSocket endpoint remains the primary interface (story 001).
- Only worth doing if a concrete consumer exists — otherwise skip (YAGNI).
