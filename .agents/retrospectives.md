# Retrospectives

One entry per Builder pull request, written by `/retrospective`
(`.agents/skills/retrospective/SKILL.md`): what went well, what the reviewers, bots and CI
found, what made the build hard, and what came back as feedback after a merge. A cause that
appears twice becomes a rule in `.agents/learnings.md` (**Builds**). Read by the builder and by
`/retrospective`; no entry names a person.

Entries up to 2026-11-11 are below. From 2026-11-12 each retro is an issue labelled `from:retro`
(filed by `/retrospective`), so two pull requests never edit the same lines.

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

### 2026-11-11, #280 / PR #322 (feature, amp, milestone 1 of 3)
- **Went well:** a small `Looper` with a control handle made every state testable without an editor; the existing bit-for-bit click test doubled as the empty-looper proof.
- **Problems:** the first order test (loop after the chain, before the click) only asserted "differs" and would pass with the looper after the click (reviewer, round 1): a test for an order rule must fail when the order is wrong. An empty test range (`FADE..100`) checked nothing until clippy found it (own gate).
- **Friction:** `scripts/check.sh` failed twice on clippy only after 10+ minutes (a deprecated atomic method on 1.99, the range): run `cargo +1.99.1 clippy -p amp --all-targets -- -D warnings` before the full gate. A new parameter touches four places that are easy to miss: `ids.txt`, the position test in `lib.rs`, `LEFT_ALONE` in `user_presets.rs` (found only by the full test run, 11 minutes) and the AGENTS table.
- **Session:** the shared `no_alloc` host cannot send commands mid-run, so the test drives `process` itself; a helper that takes a per-block hook would help. Waiting on long gates cost several foreground time-outs: start them in the background at once.
- **Recurring:** none

### 2026-11-11, #279 / PR #320 (feature, pad)
- **Went well:** the spec named files, ids and tests, so the build was one pass; the reviewer accepted with no findings
- **Friction:** the spec named `control_change::SUSTAIN_PEDAL`, which nih-plug does not have (it is `DAMPER_PEDAL`, found by grepping midi-consts in the cargo checkout); the first draft of the listening render re-pressed the pedal in the wrong place and was fixed before it was shipped; `scripts/check.sh` takes over ten minutes, run in the background as the brief said
- **Page:** a new control is unreleased, so the page was left alone (a first edit to `site/dsp/index.html` was reverted per /page)
- **Nothing recurring yet**
