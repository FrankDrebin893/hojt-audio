#!/usr/bin/env bash
# Tests the disk-space check of scripts/check.sh (`check.sh space`, the first step of a full run) with a
# stubbed `df`: under 2 GB free it stops with a message naming the directories to clean, over it prints the
# free space and goes on. Also that the full run starts with that check.
#
#   scripts/test-check-space.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/test-check-space.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

# A `df` that reports $FAKE_AVAIL_KB available, in the POSIX format (-P, 1024-byte blocks) check.sh asks for
mkdir -p "$tmp/bin"
cat > "$tmp/bin/df" << 'STUB'
#!/usr/bin/env bash
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "/dev/fake 100000000 1 ${FAKE_AVAIL_KB} 99% /"
STUB
chmod +x "$tmp/bin/df"
space() { FAKE_AVAIL_KB="$1" PATH="$tmp/bin:$PATH" bash "$root/scripts/check.sh" space 2>&1; }

# Over 2 GB: passes and says how much is free
out="$(space 5242880)" || fail "5 GB free was refused: $out"
grep -q "5.0 GB free" <<< "$out" || fail "the free space is not printed: $out"

# Exactly 2 GB is enough
space 2097152 > /dev/null || fail "exactly 2 GB free was refused"

# Under 2 GB: refused, with the free space and the directories to clean
if out="$(space 1048576)"; then fail "1 GB free was accepted"; fi
grep -q "1.0 GB free" <<< "$out" || fail "the refusal does not say how much is free: $out"
grep -q "target" <<< "$out" || fail "the refusal does not name the target directories: $out"
grep -q "worktree" <<< "$out" || fail "the refusal does not name the worktrees to remove: $out"
if grep -q "/tmp/targe[t]" <<< "$out"; then fail "the refusal still advises a shared temp target directory: $out"; fi

# A nearly empty disk is refused too
if space 0 > /dev/null; then fail "0 free was accepted"; fi

# The full run begins with the check, before any cargo step
grep -Eq '^full\(\) \{' "$root/scripts/check.sh" || fail "check.sh has no full()"
awk '/^full\(\) \{/,/^}/' "$root/scripts/check.sh" | grep -q 'check_disk_space' || fail "full() does not call check_disk_space"

echo "scripts/test-check-space.sh: all good"
