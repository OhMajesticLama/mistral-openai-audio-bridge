# Agent conventions

## Coding standards
- Match existing style in the file you are editing.
- Keep diffs minimal; do not reformat untouched code.

## Testing
- Every story has acceptance tests under `tests/`.
- Test files are organized by the module they test, not by story (e.g. in Python, `tests/test_loader.py` for the loader module). A single test file may contain tests for multiple stories.
- Tests are split by level into subfolders: `tests/unit/`, `tests/integration/`, `tests/system/`. Each file is declared as a `[[test]]` target in `Cargo.toml` (Cargo only auto-discovers top-level `tests/*.rs`).
- Each test references the story it verifies, via a comment or docstring (e.g. `# Story: 002`).
- Each acceptance criterion in a story references its test explicitly, in whatever form the test framework uses to identify a test (e.g. `tests/test_loader.py::test_load_piece_from_disk` in Python).
- Write the test first (failing), then implement until it passes.
- Run the relevant tests before declaring a story done.
- A story moves to `project/stories/done/` only when its acceptance tests pass.

## Story workflow
- New ideas start in `project/stories/backlog/`.
- Create stories from `project/stories/TEMPLATE.md`.
- Move a story to `planned/` when committed to an iteration, `in-progress/` when work starts, `done/` when acceptance tests pass.
- One story file per story, named `NNN_<name>.md` (zero-padded number, snake_case name).
- When a story changes the architecture, update `docs/architecture.md` before moving it to `done/`.

## Commits
- NEVER EVER commit things yourself. When you think a commit is necessary, tell the user.