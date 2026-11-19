# Agent instructions are written for Claude only

Skills live in `.claude/skills/<name>/SKILL.md` and every instruction file is a `CLAUDE.md` (root, `piano/`, `plugins/*`). There is no `AGENTS.md`, no mirror for other agent tools, no `.agents/`, no `.cursorrules` and no Copilot file (PR #335, 2026-11-12). General skills come from Matt Pocock's plugin, enabled in `.claude/settings.json`, instead of being copied in.

The reason: keeping the repo usable by several agent tools meant keeping every instruction in two places. The engineer chose to drop that.

## Consequences

Do not re-create `AGENTS.md`, a mirror or a learnings file. A rule goes into the `CLAUDE.md` or the skill step it governs.
