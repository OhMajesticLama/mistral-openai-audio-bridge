---
# Frontmatter — fill in all fields before writing the story body.
# id:               zero-padded number, unique across all stories (e.g. 003)
# title:            short imperative phrase (e.g. "Load MSD piece from disk")
# status:           must match the folder: backlog | planned | in-progress | done
# epic:             theme or epic this belongs to, optional
# estimate:         story points or t-shirt size, optional
# created:          date you wrote this story YYYY-MM-DD
# in_progress_date: date this story is moved to "in progress"
# done_date:        date this story is moved to "done"
id: NNN
title: <short imperative title>
status: backlog
epic: <optional>
estimate: <optional>
created: YYYY-MM-DD
in_progress_date: YYYY-MM-DD
done_date: YYYY-MM-DD
---

## Story
<!-- One capability per story. If the "I want" clause has an "and", split it. -->
<!-- The "so that" clause is mandatory — no stated value, no story. -->
<!-- The role comes from project/personas.md (Speaker, Operator, Contributor). -->

As a **<persona from project/personas.md>**,
I want **<goal/capability>**,
so that **<value/benefit>**.

## Acceptance criteria
<!-- Each criterion names the test that verifies it, in whatever form -->
<!-- the test framework uses to identify a test. -->
<!-- A criterion without a test reference is not done — it's not even a criterion. -->
<!-- Write the test first (failing), then implement until it passes. -->
<!-- Test files are organized by module, not by story. A test references -->
<!-- its story via a comment or docstring (e.g. # Story: NNN). -->

- [ ] <criterion 1 — concrete and testable>
      Test: <test reference, e.g. tests/test_loader.py::test_load_piece_from_disk in Python>
- [ ] <criterion 2>
      Test: <test reference>
- [ ] <criterion 3>
      Test: <test reference>
- [ ] Definition of Done met.

## Notes
<!-- Context, constraints, references to data/scripts, open questions. -->
<!-- Spikes skip acceptance criteria — their output is a finding, not a feature. -->

<notes>
