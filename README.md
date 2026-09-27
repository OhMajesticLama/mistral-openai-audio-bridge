# vibe-audio-bridge

WebSocket bridge from the Mistral realtime transcription protocol to an
OpenAI-compatible `POST /v1/audio/transcriptions` endpoint.

Clients that speak the Mistral realtime transcription protocol over
WebSocket (e.g. Vibe's voice mode) cannot talk to a transcription server
that only exposes the batch HTTP endpoint. This bridge accepts the
WebSocket protocol, buffers the recorded PCM audio, and on
`input_audio.end` submits it to the HTTP endpoint with `stream=true`,
forwarding SSE transcript deltas back to the client as they arrive.

```
client ──WS──> vibe-audio-bridge ──HTTP multipart──> v1/audio/transcriptions server
       <──events──                 <──SSE stream──
```

## Requirements

- Rust toolchain (rustup) — to build and install
- A running OpenAI-compatible transcription server exposing
  `POST /v1/audio/transcriptions` (with SSE streaming when `stream=true`)
- A client that speaks the Mistral realtime transcription protocol

## Install

```bash
cargo install --path .
```

The binary lands in `~/.cargo/bin/vibe-audio-bridge`.

## Run

```bash
vibe-audio-bridge
```

Listens on `ws://127.0.0.1:8081/v1/audio/transcriptions/realtime` and
forwards to `http://127.0.0.1:8080`. Point your client at the WebSocket
endpoint.

Configuration — CLI flags override environment variables, which override
defaults:

| Flag | Environment variable | Default |
|---|---|---|
| `--listen` | `VIBE_BRIDGE_LISTEN` | `127.0.0.1:8081` |
| `--upstream` | `VIBE_BRIDGE_UPSTREAM` | `http://127.0.0.1:8080` |
| `--log-level` | `VIBE_BRIDGE_LOG_LEVEL` | `info` |
| `--debug-dump` | `VOXTRAL_DEBUG_DUMP=1` | off |
| `--dump-dir` | `VIBE_BRIDGE_DUMP_DIR` | `/tmp/voxtral-debug` |

`--help` and `--version` are supported.

## Audio format

Audio must arrive as raw `pcm_s16le`, 16 kHz, mono, base64-encoded in
`input_audio.append` frames. The bridge wraps the buffered bytes in a WAV
header before uploading — no transcoding happens.

## Protocol

WebSocket endpoint: `/v1/audio/transcriptions/realtime?model=<name>`
(`model` defaults to `voxtral-realtime` and is passed through to the
upstream).

| Direction | Event | Meaning |
|---|---|---|
| out | `session.created` | Sent on connect; announces model and audio format |
| in | `input_audio.append` | Base64 PCM chunk, buffered |
| in | `input_audio.flush` | Accepted, no-op (batch upstream) |
| in | `input_audio.end` | Transcribe the buffer, then reset it |
| out | `transcription.text.delta` | Partial transcript, forwarded from upstream SSE |
| out | `transcription.done` | Final transcript; empty buffer short-circuits without an upstream call |
| out | `error` | Upstream HTTP failure (status + truncated body) or transport error |

`session.update` and unknown message types are ignored; non-JSON frames
are dropped.

## Troubleshooting

| Symptom | Likely cause | Check |
|---|---|---|
| Client cannot connect (connection refused) | Bridge not running, or wrong `--listen` | `ss -ltn \| grep 9932` |
| Empty transcripts / "no speech" | Mic capture too quiet — not a bridge bug | Run with `--debug-dump`, check the logged `peak=` level and inspect the dumped PCM |
| `error` event, `upstream HTTP <code>` | Transcription server down or wrong `--upstream` | `curl` the upstream `/v1/audio/transcriptions` directly |


## Run tests

```bash
cargo test                                            # unit + integration + binary system tests
cargo test --test system_transcription -- --ignored   # against a live transcription server
cargo llvm-cov                                        # coverage
```
