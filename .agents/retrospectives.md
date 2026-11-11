# Retrospectives

One entry per Builder pull request, written by `/retrospective`
(`.agents/skills/retrospective/SKILL.md`): what went well, what the reviewers, bots and CI
found, what made the build hard, and what came back as feedback after a merge. A cause that
appears twice becomes a rule in `.agents/learnings.md` (**Builds**). Read by the builder and by
`/retrospective`; no entry names a person.

## Patterns seen

(none yet)

## Entries

### 2026-11-11, #279 / PR #320 (feature, pad)
- **Went well:** the spec named files, ids and tests, so the build was one pass; the reviewer accepted with no findings
- **Friction:** the spec named `control_change::SUSTAIN_PEDAL`, which nih-plug does not have (it is `DAMPER_PEDAL`, found by grepping midi-consts in the cargo checkout); the first draft of the listening render re-pressed the pedal in the wrong place and was fixed before it was shipped; `scripts/check.sh` takes over ten minutes, run in the background as the brief said
- **Page:** a new control is unreleased, so the page was left alone (a first edit to `site/dsp/index.html` was reverted per /page)
- **Nothing recurring yet**
