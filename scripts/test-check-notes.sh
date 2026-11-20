#!/usr/bin/env bash
# Tests the amp notes check of scripts/check.sh (`check.sh notes`) on copies of the amp's router and notes:
# the real ones pass, a notes file the router does not link fails, a link to a missing file fails.
#
#   scripts/test-check-notes.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/test-check-notes.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

notes() { AMP_NOTES_DIR="$1" bash "$root/scripts/check.sh" notes 2>&1; }
fresh() {
    rm -rf "$tmp/amp"
    mkdir -p "$tmp/amp"
    cp "$root/plugins/amp/CLAUDE.md" "$tmp/amp/CLAUDE.md"
    cp -r "$root/plugins/amp/notes" "$tmp/amp/notes"
}

# The real notes pass
fresh
out="$(notes "$tmp/amp")" || fail "the real router and notes were refused: $out"

# A notes file nothing links to
fresh
echo "# Stray" > "$tmp/amp/notes/stray.md"
if out="$(notes "$tmp/amp")"; then fail "an unlinked notes file was accepted"; fi
grep -q "notes/stray.md is not linked" <<< "$out" || fail "the unlinked file is not named: $out"

# A link to a file that is not there
fresh
rm "$tmp/amp/notes/looper.md"
if out="$(notes "$tmp/amp")"; then fail "a link to a missing file was accepted"; fi
grep -q "notes/looper.md, which does not exist" <<< "$out" || fail "the dead link is not named: $out"

echo "scripts/test-check-notes.sh: all good"
