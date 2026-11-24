#!/usr/bin/env bash
# Tests the target-directory check of scripts/check.sh (`check.sh target`, the first step of a full run): a
# CARGO_TARGET_DIR outside the checkout is refused with the fix named, unless CHECK_SHARED_TARGET=1; unset or
# inside the checkout it passes and prints the directory. Also that the full run starts with that check.
#
#   scripts/test-check-target.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/test-check-target.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

target() { env "$@" bash "$root/scripts/check.sh" target 2>&1; }

# Unset: passes and prints the checkout's own target/
out="$(target -u CARGO_TARGET_DIR -u CHECK_SHARED_TARGET)" || fail "an unset CARGO_TARGET_DIR was refused: $out"
grep -q "Target directory: $root/target" <<< "$out" || fail "the directory in use is not printed: $out"

# Inside the checkout: passes
out="$(target -u CHECK_SHARED_TARGET CARGO_TARGET_DIR="$root/target-x")" || fail "a target inside the checkout was refused: $out"
grep -q "target-x" <<< "$out" || fail "the directory in use is not printed: $out"

# Outside: refused, naming the variable and the fix
if out="$(target -u CHECK_SHARED_TARGET CARGO_TARGET_DIR=/tmp/elsewhere)"; then fail "CARGO_TARGET_DIR=/tmp/elsewhere was accepted: $out"; fi
grep -q "CARGO_TARGET_DIR" <<< "$out" || fail "the refusal does not name the variable: $out"
grep -q "unset CARGO_TARGET_DIR" <<< "$out" || fail "the refusal does not name the fix: $out"

# A relative path that leaves the checkout is refused too
if out="$(cd "$root" && env -u CHECK_SHARED_TARGET CARGO_TARGET_DIR=../elsewhere bash scripts/check.sh target 2>&1)"; then
    fail "a relative target outside the checkout was accepted: $out"
fi

# CHECK_SHARED_TARGET=1 goes on
out="$(target CARGO_TARGET_DIR=/tmp/elsewhere CHECK_SHARED_TARGET=1)" || fail "CHECK_SHARED_TARGET=1 was refused: $out"
grep -q "/tmp/elsewhere" <<< "$out" || fail "the directory in use is not printed: $out"

# Cold: no debug/deps in the target directory says so and still exits 0; warm: says warm
cold="$root/target-cold-test"
rm -rf "$cold"
trap 'rm -rf "$cold"' EXIT
out="$(target -u CHECK_SHARED_TARGET CARGO_TARGET_DIR="$cold")" || fail "a cold target was refused: $out"
grep -q "Target directory is cold" <<< "$out" || fail "a target without debug/deps is not called cold: $out"
grep -q "cargo test -p <touched crates>" <<< "$out" || fail "the cold line does not name the quick gate: $out"
mkdir -p "$cold/debug/deps"
out="$(target -u CHECK_SHARED_TARGET CARGO_TARGET_DIR="$cold")" || fail "a warm target was refused: $out"
grep -q "Target directory is warm" <<< "$out" || fail "a target with debug/deps is not called warm: $out"
if grep -q "is cold" <<< "$out"; then fail "a warm target also says cold: $out"; fi

# The full run begins with the check, before any cargo step
awk '/^full\(\) \{/,/^}/' "$root/scripts/check.sh" | grep -q 'check_target_dir' || fail "full() does not call check_target_dir"

echo "scripts/test-check-target.sh: all good"
