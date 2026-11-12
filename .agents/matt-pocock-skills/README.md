# Matt Pocock's skills in this repo

Source: https://github.com/mattpocock/skills (MIT, `LICENSE` here). Vendored copy of
**mattpocock-skills 1.3.2**, commit `4ae2317ddb22b046e26455e4f9c9b3aaaf62212d` (2026-10-31),
taken as editable files into `.agents/skills/`, each with its `.claude/commands/` mirror
(`xtask/tests/skills_mirror.rs`). Supporting files (`tdd/tests.md`, `domain-modeling/*-FORMAT.md`...)
sit beside their `SKILL.md` and are read from there.

## Installed

Engineering: `code-review`, `codebase-design`, `diagnosing-bugs`, `domain-modeling`,
`grill-with-docs`, `improve-codebase-architecture`, `prototype`, `research`, `tdd`,
`setup-matt-pocock-skills`. Productivity: `grill-me`, `grilling`, `handoff`, `teach`,
`to-questionnaire`, `wait-what`, `writing-for-agents`.

## Not installed, and why

The repo's own flow (`AGENTS.md`, "How work flows": state labels, owner-only approval) already
has a skill for each of these jobs, and the Pocock versions would bypass it:

| Pocock skill | The repo's own |
|---|---|
| `to-spec`, `to-tickets` | `/spec`, `/issue` |
| `triage` | `/groom`, `/specwriter` (state labels, not triage labels) |
| `implement`, `implement-spec`, `pr` | `/next` (one sub-session per issue, a reviewer, the PR template and attribution) |
| `wayfinder` | `/wayfinder`, an earlier adaptation (maps are `wayfinder:map` issues, `Map:` not `Parent:`) |
| `retro` | `/retrospective`: its improvement categories are step 2 "Environment"; the retro issue, the rules it writes into AGENTS.md or a skill and the shipping are ours |
| `ask-matt`, `wizard` | they route to and scaffold the skills above, so they would point at skills that are not here |
| `in-progress/`, `misc/` | not part of the promoted set |

`/feedback` is ours: it writes a rule into AGENTS.md or the skill it governs.

## Setup

`/setup-matt-pocock-skills` was done by hand: `docs/agents/issue-tracker.md` (GitHub, written for
this repo's labels and cloud tools), `docs/agents/domain.md` (single-context, the package's
text), and the `## Agent skills` block in `AGENTS.md`. No `triage-labels.md`: `triage` is not installed.

## Updating

Clone the source, check `package.json` `version` and `CHANGELOG.md` against 1.3.2, then copy each
installed skill folder over `.agents/skills/<name>/` (keep a local edit if there is one: none yet),
regenerate the mirrors (a command is the `SKILL.md` without its `name:` line), and update the
version and commit above. Changelog entries that say "re-run `/setup-matt-pocock-skills`" mean
comparing `docs/agents/` with the new templates by hand.
