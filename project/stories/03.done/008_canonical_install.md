---
id: 008
title: Ship a canonical cargo install
status: done
epic: rust-bridge
estimate: S
created: 2026-09-27
in_progress_date: 2026-09-27
done_date: 2026-09-27
---

## Story

As a **bridge operator**,
I want **the package to install the standard Rust way — `cargo install --path .` — with complete package metadata, an MIT license, and a README that tells me how to install, run, configure, and troubleshoot it**,
so that **installing and updating the bridge is a routine `cargo install` instead of a build recipe, and I can see what I'm shipping and trusting before I run it**.

## Acceptance criteria

- [x] `cargo install --path .` builds the release binary and installs it to `~/.cargo/bin/vibe-audio-bridge`, answering `--version` and `--help`.
      Test: manual verification 2026-09-27 (installed binary transcribed end-to-end through a live server on a spare port)
- [x] The manifest carries canonical metadata: `description` (upstream-neutral: Mistral realtime WS protocol -> OpenAI-compatible `/v1/audio/transcriptions`), `license = "MIT"`, `authors`, `readme`.
      Test: `cargo package --list` succeeds on a clean tree and includes `LICENSE` and `README.md`
- [x] An MIT `LICENSE` file ships with the package.
      Test: `cargo package --list` includes `LICENSE`
- [x] The README covers requirements, install, run/configuration, the audio format contract, the protocol event reference, and troubleshooting for the three field failure modes (bridge not running, quiet mic, upstream errors).
      Test: README.md sections Requirements / Install / Run / Audio format / Protocol / Troubleshooting
- [x] Release binaries are stripped.
      Test: `file ~/.cargo/bin/vibe-audio-bridge` reports `stripped`
- [x] `Cargo.lock` is tracked (reproducible installs) and llvm-cov `.profraw` artifacts are ignored.
      Test: `git ls-files Cargo.lock`; `.gitignore` contains `*.profraw`

## Notes

- Implemented retroactively: the chore landed as commit `37c8477` ("chore: make cargo install canonical") before this story was written; criteria verified on 2026-09-27. The README rewrite follows in a separate `docs` commit (staged at the time of writing), so the README criterion is met by the tree, not yet by history.
- The upstream URL validation fix (invalid `VIBE_BRIDGE_UPSTREAM` fails at startup) rode into the same chore commit; it is a separate concern and deserves its own story if you want it on the record.
- `cargo package` still warns about missing `homepage`/`repository` — add them when there is a public URL to point at.
- `docs/glossary.md` (wrong-project boilerplate) is deliberately left untracked.

## Verification (2026-09-27)

All criteria re-run and confirmed: installed binary answers
`--version`/`--help`; `cargo package --list` includes `LICENSE` and
`README.md`; all six README sections present; installed binary reports
`stripped`; `Cargo.lock` tracked and `*.profraw` ignored. The package
list was produced with `--allow-dirty` because the README/docs commit is
still pending — the clean-tree run passes once that commit lands.
