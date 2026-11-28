#!/usr/bin/env bash
# Tests how scripts/check.sh decides between a light and a full run, on a temporary git repo: a diff of only
# the page selects "light", one that touches .claude/skills/ selects "skills", one with a .rs file selects
# "full", and the list of light paths is written once (scripts/light-paths.txt), read by ci.yml and check.sh.
#
#   scripts/test-check-light.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/check.sh" "$root/scripts/test-check-light.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

# One list, two readers: neither ci.yml nor check.sh holds a list of its own
ci="$root/.github/workflows/ci.yml"
grep -q 'scripts/light-paths.txt' "$ci" || fail "ci.yml does not read scripts/light-paths.txt"
grep -q 'light-paths.txt' "$root/scripts/check.sh" || fail "check.sh does not read light-paths.txt"
grep -q 'site-sources/' "$ci" && fail "ci.yml holds a list of light paths of its own"
grep -q 'site-sources/' "$root/scripts/check.sh" && fail "check.sh holds a list of light paths of its own"
[[ -s "$root/scripts/light-paths.txt" ]] || fail "scripts/light-paths.txt is missing or empty"

# A repo with the script and the list, and a base commit that stands for origin/main
repo="$tmp/repo"
mkdir -p "$repo/scripts" "$repo/.github/workflows" "$repo/site" "$repo/src" "$repo/.claude/skills/x" "$repo/xtask/tests" "$repo/xtask/src"
cp "$root/scripts/check.sh" "$root/scripts/light-paths.txt" "$root/scripts/xtask-test-paths.txt" "$repo/scripts/"
cp "$ci" "$repo/.github/workflows/ci.yml"
git -C "$repo" init -q -b main
git -C "$repo" config user.email t@example.com
git -C "$repo" config user.name t
git -C "$repo" config commit.gpgsign false
echo a > "$repo/README.md"
echo a > "$repo/src/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m base
git -C "$repo" update-ref refs/remotes/origin/main HEAD

scope() { bash "$repo/scripts/check.sh" scope; }
reset() {
    git -C "$repo" reset -q --hard origin/main
    git -C "$repo" clean -qfd
}
expect() {
    local want="$1" got
    got="$(scope)"
    [[ "$got" == "$want" ]] || fail "$2: expected $want, got $got"
}

expect full "no change at all"

echo b > "$repo/site/index.html"
echo b > "$repo/site/hojt.css"
echo b >> "$repo/README.md"
git -C "$repo" add -A
git -C "$repo" commit -q -m page
expect light "a page-only diff"

echo b >> "$repo/src/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m code
expect full "a diff with one .rs file"
reset

mkdir -p "$repo/.claude/skills/x"
echo b > "$repo/.claude/skills/x/SKILL.md"
git -C "$repo" add -A
git -C "$repo" commit -q -m skill
expect skills "a diff of a skill"
echo b >> "$repo/src/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m code
expect full "a skill and a .rs file"
reset

mkdir -p "$repo/site"
echo b > "$repo/site/new.html"
expect light "an untracked page file"
echo b > "$repo/src/new.rs"
expect full "an untracked .rs file beside a page file"
reset

echo b > "$repo/CHANGELOG.md"
mkdir -p "$repo/piano"
echo b > "$repo/piano/x.cs"
expect full "a C# file beside markdown"
reset

# xtask's own test files count as skills scope (only the local gate shrinks), xtask's source and other .rs files do not
mkdir -p "$repo/.claude/skills/x" "$repo/xtask/tests" "$repo/xtask/src"
echo b > "$repo/.claude/skills/x/SKILL.md"
echo b > "$repo/xtask/tests/x.rs"
expect skills "a skill and an xtask test file"
echo b > "$repo/src/lib.rs"
expect full "an xtask test file beside a .rs file elsewhere"
reset

mkdir -p "$repo/xtask/tests"
echo b > "$repo/xtask/tests/x.rs"
expect skills "an xtask test file alone"
reset

mkdir -p "$repo/xtask/src"
echo b > "$repo/xtask/src/lib.rs"
expect full "xtask/src alone"
reset

# The first line of a run names the scope, and for full the first file that is neither light nor an xtask test
mkdir -p "$repo/xtask/tests" "$repo/xtask/src"
echo b > "$repo/xtask/tests/x.rs"
[[ "$(bash "$repo/scripts/check.sh" scope-line)" == "== Scope: skills" ]] || fail "scope line of a skills run"
echo b > "$repo/xtask/src/lib.rs"
[[ "$(bash "$repo/scripts/check.sh" scope-line)" == "== Scope: full because xtask/src/lib.rs" ]] || fail "scope line of a full run"
reset
[[ "$(bash "$repo/scripts/check.sh" scope-line)" == "== Scope: full because no diff" ]] || fail "scope line without a diff"
[[ "$(CHECK_BASE=refs/nothing bash "$repo/scripts/check.sh" scope-line)" == "== Scope: full because no base to compare with" ]] || fail "scope line without a base"
grep -A2 '^full() {' "$root/scripts/check.sh" | grep -q 'scope_line' || fail "a run does not start with the scope line"

# A base that does not exist is a full run, never a skipped one
[[ "$(CHECK_BASE=refs/nothing bash "$repo/scripts/check.sh" scope)" == full ]] || fail "a missing base is not full"

# `full` forces everything, whatever the diff is
mkdir -p "$repo/site"
echo b > "$repo/site/index.html"
git -C "$repo" add -A
git -C "$repo" commit -q -m page
[[ "$(bash "$repo/scripts/check.sh" scope full)" == full ]] || fail "scope full is not full"

echo "scripts/test-check-light.sh: all good"
