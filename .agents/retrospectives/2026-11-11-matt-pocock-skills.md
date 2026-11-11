### 2026-11-11, no issue / PR (chore, skills)
- **Went well:** the vendored set passed `skills_mirror` at the first run, because the mirror is a one-line rule (the `SKILL.md` without its `name:` line) that a loop can generate
- **Problems:** none seen in review (no review feedback yet at the time of writing)
- **Friction:** two probes of the upstream repo failed before the right route (`curl` of the GitHub API gives a 403-style "not enabled", and the npm registry says not found) because the cloud session has to attach a public repo with `add_repo` (read) and clone it; stacked on the open PR #326 because it edits the same retrospective skill
- **Session:** 2 failed probes, cause: no note says how to read someone else's public repo from a cloud session (fix: a line in `.agents/skills/scout/SKILL.md` and here would help, filed as nothing yet since it is 1 occurrence); the mirror test checks names, not that relative links inside a mirrored command (`./tests.md`) resolve, so vendored commands point at supporting files that live only in `.agents/skills/<name>/`
- **Late feedback:** none read (the by-hand run was on an unmerged PR)
- **Recurring:** none
