# Retrospectives

One entry per Builder pull request, written by `/retrospective`
(`.agents/skills/retrospective/SKILL.md`): what went well, what the reviewers, bots and CI
found, what made the build hard, and what came back as feedback after a merge. A cause that
appears twice becomes a rule in `.agents/learnings.md` (**Builds**). Read by the builder and by
`/retrospective`; no entry names a person.

Entries up to 2026-11-11 are below. From 2026-11-12 each entry is a file of its own in
`.agents/retrospectives/` (`<date>-<issue>.md`), so two pull requests never edit the same lines.

## Patterns seen

(none yet)

## Entries

### 2026-11-11, #270 / PR (kind:look, amp)
- **Went well:** reading main's tests first showed only the 16 + 2-amp proof was missing, so the change stayed to one test and two snapshots
- **Problems:** none from review beyond one test comment that left out the gate piece (fixed)
- **Friction:** `scripts/check.sh` takes over ten minutes, past the foreground limit, so it needs `run_in_background`; `scripts/page-check.sh` has no exec bit in the checkout, run it with `bash`
- **Recurring:** none

### 2026-11-11, #306 / PR #323 (feature, common)
- **Went well:** measuring three HTTP clients in a scratch crate first made the choice a number, not an opinion
- **Problems:** two tests depended on timing (a callback checked right after `fresh` was set, a 200 ms sleep); the reviewer found both, fixed before the PR
- **Friction:** `scripts/check.sh` runs over ten minutes, so it needs `run_in_background`; a monitor on its log is the way to wait
- **Recurring:** none
