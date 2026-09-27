---
id: 003
title: Amplify quiet microphone input before transcription
status: backlog
epic: audio-quality
estimate: M
priority: low
created: 2026-09-27
in_progress_date:
done_date:
---

## Story

As a **user with a low-gain microphone**,
I want **the bridge to boost quiet recordings before sending them upstream**,
so that **short or softly-spoken utterances still transcribe instead of returning empty text**.

## Acceptance criteria

- [ ] The bridge applies a configurable gain factor (default off, e.g. `VIBE_BRIDGE_GAIN`) to buffered PCM before WAV wrapping, with clipping protection.
      Test: tests/gain.rs::test_gain_amplifies_without_clipping
- [ ] A quiet fixture that the upstream transcribes as empty at gain 1x produces non-empty text with gain applied.
      Test: tests/gain.rs::test_quiet_fixture_transcribes_with_gain
- [ ] Gain leaves already-loud audio unchanged when disabled by default.
      Test: tests/gain.rs::test_disabled_gain_is_passthrough

## Notes

- Finding from the 2026-09-27 investigation: Vibe's capture on this machine (MacBook Air J313, ALSA path bypassing the PipeWire `j313-mic` effect chain) peaks at ~1–11% of full scale; clips below ~2% peak transcribe as empty while a louder 6 s recording transcribed fine.
- Candidate fix verified in principle: amplifying the failed debug dumps made them transcribable (test was interrupted before completion — re-verify when picking this up).
- Debug dumps from the incident live in `/tmp/voxtral-debug/` and make good fixtures if still present.
