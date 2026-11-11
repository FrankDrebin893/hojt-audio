# Retrospectives

One entry per Builder pull request, written by `/retrospective`
(`.agents/skills/retrospective/SKILL.md`): what went well, what the reviewers, bots and CI
found, what made the build hard, and what came back as feedback after a merge. A cause that
appears twice becomes a rule in `.agents/learnings.md` (**Builds**). Read by the builder and by
`/retrospective`; no entry names a person.

## Patterns seen

(none yet)

## Entries

### 2026-11-11, #280 / PR #322 (feature, amp, milestone 1 of 3)
- **Went well:** a small `Looper` with a control handle made every state testable without an editor; the existing bit-for-bit click test doubled as the empty-looper proof.
- **Problems:** the first order test (loop after the chain, before the click) only asserted "differs" and would pass with the looper after the click (reviewer, round 1): a test for an order rule must fail when the order is wrong. An empty test range (`FADE..100`) checked nothing until clippy found it (own gate).
- **Friction:** `scripts/check.sh` failed twice on clippy only after 10+ minutes (a deprecated atomic method on 1.99, the range): run `cargo +1.99.1 clippy -p amp --all-targets -- -D warnings` before the full gate. A new parameter touches four places that are easy to miss: `ids.txt`, the position test in `lib.rs`, `LEFT_ALONE` in `user_presets.rs` (found only by the full test run, 11 minutes) and the AGENTS table.
- **Session:** the shared `no_alloc` host cannot send commands mid-run, so the test drives `process` itself; a helper that takes a per-block hook would help. Waiting on long gates cost several foreground time-outs: start them in the background at once.
- **Recurring:** none
