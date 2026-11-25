#!/usr/bin/env bash
# Tests scripts/seed-target.sh (the pre-warmed build cache, issue 466) in a temporary repository with a stub `cargo`,
# `rustc`, `df` and `setsid`: `start` writes the lock and returns before the stub build ends; a second `start` while
# locked does nothing; a done marker for the same Rust and Cargo.lock skips, a changed one rebuilds; under the cold
# floor it skips; `wait` returns when the lock goes and exits 1 on a failed build; `copy` leaves out `incremental` and
# `check-logs`, leaves a warm worktree alone and never writes into the seed; without CLAUDE_CODE_REMOTE=true every
# mode but `copy` does nothing.
#
#   scripts/test-seed-target.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/test-seed-target.sh" "$root/scripts/seed-target.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

mkdir -p "$tmp/bin"
# cargo: logs its arguments, takes STUB_SLEEP seconds, fails when STUB_FAIL is set, and leaves build products
cat > "$tmp/bin/cargo" << 'STUB'
#!/usr/bin/env bash
echo "$*" >> "$STUB_LOG"
sleep "${STUB_SLEEP:-0}"
[[ -z "${STUB_FAIL:-}" ]] || { echo "error: stub build failed" >&2; exit 101; }
mkdir -p target/debug/deps target/debug/incremental/x target/optimised/deps target/optimised/incremental/y
echo built > target/debug/deps/libfoo.rlib
echo built > target/optimised/deps/libfoo.rlib
echo incr > target/debug/incremental/x/data
STUB
cat > "$tmp/bin/rustc" << 'STUB'
#!/usr/bin/env bash
echo "rustc ${FAKE_RUSTC:-1.100.0}"
STUB
# df: $FAKE_AVAIL_KB available, in the POSIX format
cat > "$tmp/bin/df" << 'STUB'
#!/usr/bin/env bash
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "/dev/fake 100000000 1 ${FAKE_AVAIL_KB:-52428800} 99% /"
STUB
# setsid: no new session needed in a test, the build runs as a plain child
cat > "$tmp/bin/setsid" << 'STUB'
#!/usr/bin/env bash
exec "$@"
STUB
chmod +x "$tmp/bin"/*

repo="$tmp/repo"
mkdir -p "$repo/scripts"
cp "$root/scripts/seed-target.sh" "$root/scripts/check.sh" "$repo/scripts/"
echo 'version = 3' > "$repo/Cargo.lock"
git -C "$repo" init -q -b main
git -C "$repo" -c user.name=t -c user.email=t@t add -A
git -C "$repo" -c user.name=t -c user.email=t@t commit -q -m init
git -C "$repo" worktree add -q "$tmp/wt" -b wt

export STUB_LOG="$tmp/cargo.log"
seedsh() { env -u CLAUDE_CODE_REMOTE PATH="$tmp/bin:$PATH" bash "$repo/scripts/seed-target.sh" "$@" 2>&1; }
remote() { CLAUDE_CODE_REMOTE=true PATH="$tmp/bin:$PATH" bash "$repo/scripts/seed-target.sh" "$@" 2>&1; }
builds() { grep -c 'no-run' "$STUB_LOG" 2> /dev/null || true; }
reset() {
    rm -rf "$repo/target" "$tmp/wt/target" "$STUB_LOG"
    : > "$STUB_LOG"
}

# Without CLAUDE_CODE_REMOTE=true start and wait do nothing
reset
out="$(seedsh start)" || fail "start outside a cloud session failed: $out"
grep -q "not a cloud session" <<< "$out" || fail "start outside a cloud session does not say so: $out"
seedsh wait > /dev/null || fail "wait outside a cloud session failed"
[[ ! -e "$repo/target" ]] || fail "start outside a cloud session made a target/"
[[ "$(builds)" == 0 ]] || fail "start outside a cloud session built"

# start returns before the build ends, and writes the lock
reset
export STUB_SLEEP=3
started=$SECONDS
out="$(remote start)" || fail "start failed: $out"
((SECONDS - started < 3)) || fail "start did not return before the build ended"
grep -q "started" <<< "$out" || fail "start does not say it started: $out"
[[ -f "$repo/target/.seed.lock" ]] || fail "start wrote no lock"
kill -0 "$(cat "$repo/target/.seed.lock")" || fail "the lock does not hold the pid of a running build"

# A second start while locked does nothing
out="$(remote start)" || fail "second start failed: $out"
grep -q "running already" <<< "$out" || fail "second start did not see the lock: $out"

# copy of a seed that is still being built does nothing
out="$(seedsh copy "$tmp/wt")" || fail "copy during the build failed: $out"
grep -q "no finished seed" <<< "$out" || fail "copy during the build did not refuse: $out"
[[ ! -e "$tmp/wt/target" ]] || fail "copy during the build wrote a target/"

# wait returns when the build ends; both builds ran, the lock is gone, the marker holds Rust and the lock file
remote wait > /dev/null || fail "wait failed after a good build"
[[ ! -e "$repo/target/.seed.lock" ]] || fail "the lock is still there after wait"
[[ "$(builds)" == 2 ]] || fail "expected the debug and the optimised build, got: $(cat "$STUB_LOG")"
grep -q -- '--workspace --no-run --locked' "$STUB_LOG" || fail "no workspace build: $(cat "$STUB_LOG")"
grep -q -- '-p amp -p bass --profile optimised --no-run --locked' "$STUB_LOG" || fail "no optimised build: $(cat "$STUB_LOG")"
grep -q "^rustc: rustc 1.100.0" "$repo/target/.seed-done" || fail "the marker does not name the Rust: $(cat "$repo/target/.seed-done")"
grep -q "^cargo-lock: " "$repo/target/.seed-done" || fail "the marker does not hold the lock file's hash"

# The same Rust and Cargo.lock: start is skipped
unset STUB_SLEEP
out="$(remote start)" || fail "start with a done marker failed: $out"
grep -q "current" <<< "$out" || fail "start with a done marker did not skip: $out"
[[ "$(builds)" == 2 ]] || fail "a current seed was rebuilt"

# copy: leaves out incremental and check-logs and the seed's own lock and log, keeps the rest
mkdir -p "$repo/target/check-logs"
echo log > "$repo/target/check-logs/01.log"
echo keep > "$repo/target/.rustc_info.json"
out="$(seedsh copy "$tmp/wt")" || fail "copy failed: $out"
[[ -f "$tmp/wt/target/debug/deps/libfoo.rlib" ]] || fail "copy did not copy debug/deps"
[[ -f "$tmp/wt/target/optimised/deps/libfoo.rlib" ]] || fail "copy did not copy optimised/deps"
[[ -f "$tmp/wt/target/.rustc_info.json" ]] || fail "copy did not copy a dot file"
[[ ! -e "$tmp/wt/target/debug/incremental" ]] || fail "copy took debug/incremental"
[[ ! -e "$tmp/wt/target/optimised/incremental" ]] || fail "copy took optimised/incremental"
[[ ! -e "$tmp/wt/target/check-logs" ]] || fail "copy took check-logs"
[[ ! -e "$tmp/wt/target/seed.log" && ! -e "$tmp/wt/target/.seed-done" ]] || fail "copy took the seed's log or marker"
[[ -d "$repo/target/debug/incremental" ]] || fail "copy removed something from the seed"

# The copy is a copy: writing in it does not write into the seed (no hardlinks)
echo changed > "$tmp/wt/target/debug/deps/libfoo.rlib"
[[ "$(cat "$repo/target/debug/deps/libfoo.rlib")" == built ]] || fail "writing in the copy changed the seed"

# A warm worktree is left alone
echo mine > "$tmp/wt/target/debug/deps/mine"
out="$(seedsh copy "$tmp/wt")" || fail "copy into a warm worktree failed: $out"
grep -q "warm already" <<< "$out" || fail "copy into a warm worktree did not say so: $out"
[[ "$(cat "$tmp/wt/target/debug/deps/libfoo.rlib")" == changed ]] || fail "copy overwrote a warm worktree"

# copy works without CLAUDE_CODE_REMOTE (it is what a builder or a person runs), and refuses a missing directory
rm -rf "$tmp/wt/target"
seedsh copy "$tmp/wt" > /dev/null || fail "copy without CLAUDE_CODE_REMOTE failed"
[[ -f "$tmp/wt/target/debug/deps/libfoo.rlib" ]] || fail "copy without CLAUDE_CODE_REMOTE copied nothing"
if seedsh copy "$tmp/nowhere" > /dev/null; then fail "copy of a missing directory was accepted"; fi

# copy refuses under the cold floor (8 GB), also when the seed's own target/ is warm, and leaves nothing behind
rm -rf "$tmp/wt/target"
out="$(FAKE_AVAIL_KB=$((5 * 1024 * 1024)) seedsh copy "$tmp/wt")" || fail "copy under the floor failed: $out"
grep -q "not enough disk space" <<< "$out" || fail "copy under the floor did not say why: $out"
[[ ! -e "$tmp/wt/target" && ! -e "$tmp/wt/target.seed-copy" ]] || fail "copy under the floor left something behind"

# A copy that fails half-way leaves no target/ and no temporary directory: a cp that fails on the second part
mkdir "$tmp/cpbin"
cat > "$tmp/cpbin/cp" << 'STUB'
#!/usr/bin/env bash
n=$(cat "$STUB_CP_COUNT" 2> /dev/null || echo 0)
echo $((n + 1)) > "$STUB_CP_COUNT"
((n < 1)) || exit 1
exec /bin/cp "$@"
STUB
chmod +x "$tmp/cpbin/cp"
rm -f "$tmp/cpcount"
if out="$(STUB_CP_COUNT="$tmp/cpcount" PATH="$tmp/cpbin:$PATH" seedsh copy "$tmp/wt")"; then fail "a failing cp was accepted: $out"; fi
[[ ! -e "$tmp/wt/target" && ! -e "$tmp/wt/target.seed-copy" ]] || fail "a failed copy left a half-copied tree behind"

# A changed Cargo.lock rebuilds; so does a changed Rust
echo 'version = 4' > "$repo/Cargo.lock"
remote start > /dev/null || fail "start after a Cargo.lock change failed"
remote wait > /dev/null || fail "wait after a Cargo.lock change failed"
[[ "$(builds)" == 4 ]] || fail "a changed Cargo.lock did not rebuild: $(cat "$STUB_LOG")"
FAKE_RUSTC=1.101.0 remote start > /dev/null || fail "start after a Rust change failed"
FAKE_RUSTC=1.101.0 remote wait > /dev/null || fail "wait after a Rust change failed"
[[ "$(builds)" == 6 ]] || fail "a changed Rust did not rebuild: $(cat "$STUB_LOG")"

# Under the cold floor (8 GB) it skips and says why
reset
out="$(FAKE_AVAIL_KB=$((5 * 1024 * 1024)) remote start)" || fail "start under the floor failed: $out"
grep -q "not enough disk space" <<< "$out" || fail "start under the floor did not say why: $out"
[[ ! -e "$repo/target/.seed.lock" ]] || fail "start under the floor wrote a lock"
[[ "$(builds)" == 0 ]] || fail "start under the floor built"

# A failed build: wait exits 1 and prints the log's last line; no marker; the lock is gone
reset
out="$(STUB_FAIL=1 remote start)" || fail "start with a failing build failed: $out"
if out="$(STUB_FAIL=1 remote wait)"; then fail "wait accepted a failed build: $out"; fi
grep -q "no finished seed" <<< "$out" || fail "wait does not say there is no seed: $out"
grep -q "stub build failed" <<< "$out" || fail "wait does not print the log's last line: $out"
[[ ! -e "$repo/target/.seed-done" ]] || fail "a failed build wrote the marker"
[[ ! -e "$repo/target/.seed.lock" ]] || fail "a failed build left the lock"

echo "scripts/test-seed-target.sh: all good"
