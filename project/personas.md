# Personas

Short, fixed cast for user stories. Pick one per story; if a story serves
nobody in this list, question whether it should exist.

## Voice-mode user

Dictates into a voice-mode client (e.g. Vibe). Never sees, configures, or
runs the bridge — experiences only whether their words appear, quickly and
correctly. Cares about: reliability, latency, their audio staying local.

## Bridge operator

Installs, configures, and runs the bridge next to a transcription server.
Reads the logs when something breaks. Cares about: working out of the box,
safe defaults, clear diagnostics, easy deployment.

## Contributor

Changes the code. Cares about: clear architecture, leveled tests, the
story workflow. Most Contributor needs are covered by `AGENTS.md`; write
stories for them only when the value is not already captured there.
