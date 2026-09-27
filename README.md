# Music Recognition

## Vision

<One paragraph: what problem this project solves and what "done" looks
like for the whole effort.>

## Structure

```
docs/      product documentation (architecture, glossary)
project/   planning artifacts (roadmap, stories, definition of done)
tests/     executable spec — acceptance and unit tests
scripts/   data prep, training, and inference entry points
data/      datasets (submodules)
```

See `project/roadmap.md` for where the project is headed and `project/definition-of-done.md` for what "done" means for a story.

## Setup

```sh
python -m venv venv
source venv/bin/activate
# install dependencies (to be added)
```

## Workflow

Stories live in `project/stories/` and move through state folders as they advance:

`backlog/` → `planned/` → `in-progress/` → `done/`

A story is done when its acceptance tests pass and it meets the criteria in `project/definition-of-done.md`.
