#!/usr/bin/env bash
# Is a release pull request ready to ship? Reads the pull request's description (stdin, or the
# file named as the argument) and fails while the section "## Ready to ship" has an unticked
# box, or has none. Other sections ("## After the merge", "## Not checked") are not looked at.
# Used by .github/workflows/release-ready.yml; the rules are in .claude/skills/release/RELEASING.md.
set -euo pipefail

if [ $# -gt 0 ]; then body="$(tr -d '\r' <"$1")"; else body="$(tr -d '\r')"; fi

section="$(printf '%s\n' "$body" | awk '
  /^## / { on = ($0 ~ /^## Ready to ship[[:space:]]*$/); next }
  on { print }')"

if ! printf '%s\n' "$section" | grep -qE '^[[:space:]]*- \[[ xX]\]'; then
  echo "Release not ready: the description has no section '## Ready to ship' with boxes in it." >&2
  exit 1
fi

open="$(printf '%s\n' "$section" | grep -E '^[[:space:]]*- \[ \]' || true)"
if [ -n "$open" ]; then
  echo "Release not ready: these boxes under '## Ready to ship' are still open:" >&2
  printf '%s\n' "$open" >&2
  exit 1
fi
echo "Release ready: every box under '## Ready to ship' is ticked."
