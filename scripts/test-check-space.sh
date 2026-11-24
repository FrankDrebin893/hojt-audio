#!/usr/bin/env bash
# Tests the disk-space check of scripts/check.sh (`check.sh space`, the first step of a full run) with a
# stubbed `df` and `du` in a temporary repository with two worktrees: under the floor (8 GB free with a cold
# target/, 2 GB with a warm one; CHECK_MIN_FREE_GB overrides both) it stops with the target/ sizes, largest first,
# and what to clean; over it prints the free space and goes on. Also that the full run starts with that check.
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

# A `du -sk <dir>` that reads the size of a target/ directory from its size_kb file: known sizes, no gigabytes written
cat > "$tmp/bin/du" << 'STUB'
#!/usr/bin/env bash
dir="${*: -1}"
printf '%s\t%s\n' "$(cat "$dir/size_kb")" "$dir"
STUB
chmod +x "$tmp/bin/du"

# A temporary repository with a copy of check.sh (it works on the checkout it sits in) and two worktrees: the
# checkout's target/ is 5 GB, the worktree "big" holds 3 GB and the worktree "small" 1 GB
repo="$tmp/repo"
mkdir -p "$repo/scripts"
cp "$root/scripts/check.sh" "$repo/scripts/check.sh"
git -C "$repo" init -q -b main
git -C "$repo" -c user.name=t -c user.email=t@t commit -q --allow-empty -m init
git -C "$repo" worktree add -q "$tmp/big" -b big
git -C "$repo" worktree add -q "$tmp/small" -b small
sized() {
    mkdir -p "$1/target"
    echo "$2" > "$1/target/size_kb"
}
sized "$repo" 5242880
sized "$tmp/big" 3145728
sized "$tmp/small" 1048576
warm() { mkdir -p "$repo/target/debug/deps"; }
cold() { rm -rf "$repo/target/debug"; }
space() { FAKE_AVAIL_KB="$1" PATH="$tmp/bin:$PATH" bash "$repo/scripts/check.sh" space 2>&1; }
gb() { echo $(($1 * 1024 * 1024)); }

# Cold target/ (no debug/deps): 9 GB free passes and says how much, exactly 8 GB passes, 5 GB is refused
cold
out="$(space "$(gb 9)")" || fail "cold, 9 GB free was refused: $out"
grep -q "9.0 GB free" <<< "$out" || fail "the free space is not printed: $out"
grep -q "target/ cold" <<< "$out" || fail "the target/ is not called cold: $out"
space "$(gb 8)" > /dev/null || fail "cold, exactly 8 GB free was refused"
if out="$(space "$(gb 5)")"; then fail "cold, 5 GB free was accepted"; fi
grep -q "5.0 GB free" <<< "$out" || fail "the refusal does not say how much is free: $out"
grep -q "8 GB or more" <<< "$out" || fail "the refusal does not name the floor: $out"
grep -q "worktree" <<< "$out" || fail "the refusal does not name the worktrees to remove: $out"
grep -q "cargo clean" <<< "$out" || fail "the refusal does not name cargo clean: $out"
if grep -q "/tmp/targe[t]" <<< "$out"; then fail "the refusal still advises a shared temp target directory: $out"; fi

# The refusal lists every target/ with its size, largest first, then the total
grep -q "5.0 GB  $repo/target" <<< "$out" || fail "the checkout's target/ is not listed with its size: $out"
grep -q "3.0 GB  $tmp/big/target" <<< "$out" || fail "the worktree big is not listed with its size: $out"
grep -q "1.0 GB  $tmp/small/target" <<< "$out" || fail "the worktree small is not listed with its size: $out"
grep -q "9.0 GB  total" <<< "$out" || fail "the total is not listed: $out"
order="$(grep -o -E "($repo|$tmp/big|$tmp/small)/target" <<< "$out" | tr '\n' ' ')"
[[ "$order" == "$repo/target $tmp/big/target $tmp/small/target " ]] || fail "the sizes are not largest first: $order"

# Warm target/: 3 GB passes, exactly 2 GB passes, 1 GB is refused
warm
out="$(space "$(gb 3)")" || fail "warm, 3 GB free was refused: $out"
grep -q "target/ warm" <<< "$out" || fail "the target/ is not called warm: $out"
space "$(gb 2)" > /dev/null || fail "warm, exactly 2 GB free was refused"
if out="$(space "$(gb 1)")"; then fail "warm, 1 GB free was accepted"; fi
grep -q "1.0 GB free" <<< "$out" || fail "the refusal does not say how much is free: $out"
grep -q "2 GB or more" <<< "$out" || fail "the warm refusal does not name its floor: $out"

# The space mode lists the sizes when it passes too
out="$(space "$(gb 3)")" || fail "warm, 3 GB free was refused: $out"
grep -q "9.0 GB  total" <<< "$out" || fail "the space mode does not list the sizes: $out"

# CHECK_MIN_FREE_GB overrides the floor
cold
out="$(CHECK_MIN_FREE_GB=1 space "$(gb 3)")" || fail "CHECK_MIN_FREE_GB=1 did not let a cold 3 GB through: $out"
if out="$(CHECK_MIN_FREE_GB=4 space "$(gb 3)")"; then fail "CHECK_MIN_FREE_GB=4 let 3 GB through: $out"; fi

# A nearly empty disk is refused too
if space 0 > /dev/null; then fail "0 free was accepted"; fi

# The full run begins with the check, before any cargo step
grep -Eq '^full\(\) \{' "$root/scripts/check.sh" || fail "check.sh has no full()"
awk '/^full\(\) \{/,/^}/' "$root/scripts/check.sh" | grep -q 'check_disk_space' || fail "full() does not call check_disk_space"

echo "scripts/test-check-space.sh: all good"
