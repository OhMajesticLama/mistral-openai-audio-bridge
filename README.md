# vibe-audio-bridge

WebSocket bridge from the Mistral realtime transcription protocol to an OpenAI-compatible `POST /v1/audio/transcriptions` endpoint -  the project was initially written to work with [audio.cpp](https://github.com/0xShug0/audio.cpp).

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
| `-l`, `--listen` | `VIBE_BRIDGE_LISTEN` | `127.0.0.1:8081` |
| `-u`, `--upstream` | `VIBE_BRIDGE_UPSTREAM` | `http://127.0.0.1:8080` |
| `--log-level` | `VIBE_BRIDGE_LOG_LEVEL` | `info` |
| `-d`, `--debug-dump` | `VIBE_BRIDGE_DEBUG_DUMP=1` | off |
| `-D`, `--dump-dir` | `VIBE_BRIDGE_DUMP_DIR` | `/tmp/vibe-audio-bridge-debug` |

Setting a dump directory — by flag or environment variable — enables dumps;
`VIBE_BRIDGE_DEBUG_DUMP=1` enables them with the default directory.

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
| in | `input_audio.flush` | Ignored (batch upstream) |
| in | `input_audio.end` | Transcribe the buffer, then reset it |
| out | `transcription.text.delta` | Partial transcript, forwarded from upstream SSE |
| out | `transcription.done` | Final transcript; empty buffer short-circuits without an upstream call |
| out | `error` | Upstream HTTP failure (status + truncated body) or transport error |

`input_audio.flush`, `session.update` and unknown message types are ignored;
non-JSON frames
are dropped.

## Troubleshooting

| Symptom | Likely cause | Check |
|---|---|---|
| Client cannot connect (connection refused) | Bridge not running, or wrong `--listen` | `ss -ltn \| grep 8081` |
| Empty transcripts / "no speech" | Mic capture too quiet — not a bridge bug | Run with `--debug-dump`, check the logged `peak=` level and inspect the dumped PCM |
| `error` event, `upstream HTTP <code>` | Transcription server down or wrong `--upstream` | `curl` the upstream `/v1/audio/transcriptions` directly |


## Deploy

Run the bridge as a systemd user service. Save this as
`~/.config/systemd/user/vibe-audio-bridge.service`:

```ini
[Unit]
Description=Vibe audio bridge (Mistral realtime WS -> OpenAI transcriptions)
After=network.target

[Service]
ExecStart=%h/.cargo/bin/vibe-audio-bridge
Restart=on-failure
RestartSec=2
# Optional configuration:
# Environment=VIBE_BRIDGE_UPSTREAM=http://127.0.0.1:8080
# Environment=VIBE_BRIDGE_LISTEN=127.0.0.1:8081
# Environment=VIBE_BRIDGE_DEBUG_DUMP=1

[Install]
WantedBy=default.target
```

`%h` expands to your home directory, so the unit works as long as the
binary is where `cargo install` put it. Then:

```bash
systemctl --user daemon-reload
systemctl --user enable --now vibe-audio-bridge
journalctl --user -u vibe-audio-bridge -f   # follow the logs
```

The service starts at login. To keep it running with no session open:
`loginctl enable-linger $USER`.

After reinstalling the binary (`cargo install --path . --force`), restart
the service: `systemctl --user restart vibe-audio-bridge`.

## Run tests

```bash
cargo test                                            # unit + integration + binary system tests
cargo test --test system_transcription -- --ignored   # against a live transcription server
cargo llvm-cov                                        # coverage
```
