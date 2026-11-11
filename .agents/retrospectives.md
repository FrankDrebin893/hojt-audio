# Retrospectives

One entry per Builder pull request, written by `/retrospective`
(`.agents/skills/retrospective/SKILL.md`): what went well, what the reviewers, bots and CI
found, what made the build hard, and what came back as feedback after a merge. A cause that
appears twice becomes a rule in `.agents/learnings.md` (**Builds**). Read by the builder and by
`/retrospective`; no entry names a person.

## Patterns seen

(none yet)

## Entries

### 2026-11-11, #270 / PR (kind:look, amp)
- **Went well:** reading main's tests first showed only the 16 + 2-amp proof was missing, so the change stayed to one test and two snapshots
- **Problems:** none from review beyond one test comment that left out the gate piece (fixed)
- **Friction:** `scripts/check.sh` takes over ten minutes, past the foreground limit, so it needs `run_in_background`; `scripts/page-check.sh` has no exec bit in the checkout, run it with `bash`
- **Recurring:** none

### 2026-11-11, #309 / PR #324 (kind:docs, infra)
- **Went well:** reading main first showed #304 had already done most of the docs (AGENTS.md, RELEASING.md, scripts/README.md, changelog), so only the skills, mirrors and the link check were left
- **Problems:** the independent reviewer sent the first version back: the page section would be live on merge while the installer and `SHA256SUMS.txt` are in no release yet (a dead button), against `/page` ("the page describes the latest release"). The issue text asked for the page now and it was followed over that rule; the section was parked as a patch for the release commit
- **Friction:** `scripts/check.sh` takes over ten minutes (`run_in_background` and one wait); `scripts/page-check.sh` is run with `bash`; the "Waits for" and "Done when" of a sub-issue can ask for something an existing page rule forbids until the next tag, and nothing in the spec said so
- **Recurring:** none yet; a second case of a spec asking for page text about an unreleased asset would become a rule in `/spec` (say what goes in the release commit)
