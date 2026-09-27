#!/usr/bin/env python3
"""WebSocket bridge: Mistral realtime transcription protocol -> voxtral HTTP endpoint.

Vibe's voice mode connects to ws://<api_base>/v1/audio/transcriptions/realtime
and speaks the Mistral realtime transcription protocol. The voxtral server only
exposes the batch HTTP POST /v1/audio/transcriptions endpoint. This bridge
accepts the WebSocket protocol, buffers appended PCM audio, and on
input_audio.end submits it to the HTTP endpoint with stream=true, forwarding
SSE transcript.text.delta events as transcription.text.delta.

Audio is expected as raw pcm_s16le 16 kHz mono (what Vibe sends); the bridge
wraps it in a WAV header before uploading.
"""

from __future__ import annotations

import asyncio
import array
import base64
import json
import logging
import os
import time
import uuid

import httpx
import websockets
from urllib.parse import parse_qs, urlparse

LISTEN_HOST = "127.0.0.1"
LISTEN_PORT = 9932
UPSTREAM = "http://127.0.0.1:9931"
SAMPLE_RATE = 16000
CHANNELS = 1
SAMPLE_WIDTH = 2

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
log = logging.getLogger("voxtral-bridge")


def wav_header(data_len: int) -> bytes:
    byte_rate = SAMPLE_RATE * CHANNELS * SAMPLE_WIDTH
    block_align = CHANNELS * SAMPLE_WIDTH
    return (
        b"RIFF"
        + (36 + data_len).to_bytes(4, "little")
        + b"WAVE"
        + b"fmt "
        + (16).to_bytes(4, "little")
        + (1).to_bytes(2, "little")
        + CHANNELS.to_bytes(2, "little")
        + SAMPLE_RATE.to_bytes(4, "little")
        + byte_rate.to_bytes(4, "little")
        + block_align.to_bytes(2, "little")
        + (SAMPLE_WIDTH * 8).to_bytes(2, "little")
        + b"data"
        + data_len.to_bytes(4, "little")
    )


def session_created(model: str) -> str:
    return json.dumps(
        {
            "type": "session.created",
            "session": {
                "request_id": str(uuid.uuid4()),
                "model": model,
                "audio_format": {
                    "encoding": "pcm_s16le",
                    "sample_rate": SAMPLE_RATE,
                },
                "target_streaming_delay_ms": 500,
            },
        }
    )


async def send_error(ws, message: str) -> None:
    await ws.send(
        json.dumps({"type": "error", "error": {"message": message, "code": 0}})
    )


async def transcribe(ws, model: str, pcm: bytes) -> None:
    if not pcm:
        # Empty recording (user released without speaking): benign done.
        await ws.send(
            json.dumps(
                {
                    "type": "transcription.done",
                    "model": model,
                    "text": "",
                    "usage": {
                        "prompt_tokens": 0,
                        "completion_tokens": 0,
                        "total_tokens": 0,
                    },
                    "language": None,
                }
            )
        )
        return

    # Diagnostics: level check catches silent/garbled mic capture, and a raw
    # dump (VOXTRAL_DEBUG_DUMP=1) lets us inspect what the client actually sent.
    samples = array.array("h")
    samples.frombytes(pcm[: len(pcm) - len(pcm) % 2])
    peak = max((abs(s) for s in samples), default=0)
    log.info(
        "transcribing %d bytes (~%.2fs) peak=%d/32767",
        len(pcm), len(pcm) / (SAMPLE_RATE * SAMPLE_WIDTH), peak,
    )
    if os.environ.get("VOXTRAL_DEBUG_DUMP"):
        os.makedirs("/tmp/voxtral-debug", exist_ok=True)
        dump_path = f"/tmp/voxtral-debug/{int(time.time() * 1000)}.pcm"
        with open(dump_path, "wb") as f:
            f.write(pcm)
        log.info("dumped pcm to %s", dump_path)

    wav = wav_header(len(pcm)) + pcm
    try:
        async with httpx.AsyncClient(timeout=120.0) as client:
            async with client.stream(
                "POST",
                f"{UPSTREAM}/v1/audio/transcriptions",
                data={"model": model, "stream": "true"},
                files={"file": ("audio.wav", wav, "audio/wav")},
            ) as resp:
                if resp.status_code != 200:
                    body = (await resp.aread()).decode("utf-8", "replace")[:500]
                    await send_error(
                        ws, f"upstream HTTP {resp.status_code}: {body}"
                    )
                    return
                async for line in resp.aiter_lines():
                    if not line.startswith("data: "):
                        continue
                    payload = line[len("data: "):]
                    if payload == "[DONE]":
                        break
                    try:
                        ev = json.loads(payload)
                    except json.JSONDecodeError:
                        continue
                    etype = ev.get("type")
                    if etype == "transcript.text.delta":
                        await ws.send(
                            json.dumps(
                                {
                                    "type": "transcription.text.delta",
                                    "text": ev.get("delta", ""),
                                }
                            )
                        )
                    elif etype == "transcript.text.done":
                        text = ev.get("text", "")
                        log.info("upstream done text=%r", text[:200])
                        await ws.send(
                            json.dumps(
                                {
                                    "type": "transcription.done",
                                    "model": model,
                                    "text": text,
                                    "usage": {
                                        "prompt_tokens": 0,
                                        "completion_tokens": 0,
                                        "total_tokens": 0,
                                    },
                                    "language": None,
                                }
                            )
                        )
                        return
    except Exception as exc:  # noqa: BLE001 - report any failure to the client
        await send_error(ws, f"bridge error: {exc}")


async def handle(ws) -> None:
    # websockets >= 13 new asyncio API: the handshake request path carries
    # the query string; Request has no query_params attribute.
    path = getattr(getattr(ws, "request", None), "path", "") or ""
    query = parse_qs(urlparse(path).query)
    model = query.get("model", ["voxtral-realtime"])[0]
    peer = getattr(ws, "remote_address", "?")
    log.info("session start model=%s peer=%s", model, peer)

    await ws.send(session_created(model))

    buf = bytearray()
    try:
        async for raw in ws:
            try:
                msg = json.loads(raw)
            except json.JSONDecodeError:
                continue
            mtype = msg.get("type")
            if mtype == "input_audio.append":
                buf += base64.b64decode(msg.get("audio", ""))
            elif mtype == "input_audio.flush":
                pass  # batch upstream: nothing to flush mid-stream
            elif mtype == "input_audio.end":
                pcm = bytes(buf)
                buf.clear()
                await transcribe(ws, model, pcm)
            # session.update and unknown types are ignored
    except websockets.ConnectionClosed:
        pass
    finally:
        log.info("session end model=%s peer=%s", model, peer)


async def main() -> None:
    async with websockets.serve(handle, LISTEN_HOST, LISTEN_PORT):
        log.info(
            "listening on ws://%s:%d/v1/audio/transcriptions/realtime -> %s",
            LISTEN_HOST,
            LISTEN_PORT,
            UPSTREAM,
        )
        await asyncio.Future()


if __name__ == "__main__":
    asyncio.run(main())
