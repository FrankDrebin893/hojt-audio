#!/usr/bin/env bash
# Tests the quiet mode of scripts/check.sh (issue 376): under an agent (CLAUDECODE=1, no CI) a step keeps its
# output in a log and prints one line; a failing step prints the failing lines and the log's tail; a person's
# terminal, CI and --verbose print everything.
#
#   scripts/test-check-quiet.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/test-check-quiet.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

# A passing step that prints a line per "test", and a failing one that prints a long log, a failing test and an error
cat > "$tmp/pass.sh" << 'EOF'
for i in 1 2 3 4 5; do echo "test t$i ... ok"; done
EOF
cat > "$tmp/failing.sh" << 'EOF'
for i in $(seq 1 100); do echo "test t$i ... ok"; done
echo "test broken ... FAILED"
echo "thread 'broken' panicked at src/lib.rs:1:1"
for i in $(seq 1 70); do echo "tail line $i"; done
exit 3
EOF

# run <env words...> -- <step command...>: stdout and stderr together, check.sh's own exit code in $code
logs="$tmp/logs"
run() {
    local envs=()
    while [[ "$1" != -- ]]; do
        envs+=("$1")
        shift
    done
    shift
    code=0
    out="$(env -u CI -u CLAUDECODE -u CHECK_VERBOSE CHECK_LOG_DIR="$logs" "${envs[@]}" bash "$root/scripts/check.sh" "$@" 2>&1)" || code=$?
}

# Under an agent: one line for a pass, with the time and the log, and the output only in the log
run CLAUDECODE=1 -- step bash "$tmp/pass.sh"
[[ "$code" == 0 ]] || fail "a passing step exited $code: $out"
[[ "$(wc -l <<< "$out")" == 1 ]] || fail "a passing step printed more than one line: $out"
grep -Eq "^== bash .*pass\.sh: passed in [0-9]+ s, log .*/01-bash-.*\.log$" <<< "$out" || fail "the line is not as specified: $out"
log="$(sed -E 's/.*, log //' <<< "$out")"
[[ "$(grep -c 'ok$' "$log")" == 5 ]] || fail "the log does not hold the output of the step: $log"

# Under an agent: a failing step shows what failed and the tail, not the whole log, and exits 1
run CLAUDECODE=1 -- step bash "$tmp/failing.sh"
[[ "$code" == 1 ]] || fail "a failing step exited $code, not 1: $out"
grep -q "== Failed: bash .*failing\.sh" <<< "$out" || fail "no 'Failed' line: $out"
grep -q "test broken ... FAILED" <<< "$out" || fail "the failing test is not shown: $out"
grep -q "panicked at" <<< "$out" || fail "the panic is not shown: $out"
grep -q "tail line 70" <<< "$out" || fail "the tail of the log is not shown: $out"
if grep -q "test t1 ... ok" <<< "$out"; then fail "the passing output is shown: $out"; fi

# Without CLAUDECODE (a person's terminal), under CI, and with --verbose or CHECK_VERBOSE=1: everything, as before
run -- step bash "$tmp/pass.sh"
[[ "$(grep -c 'ok$' <<< "$out")" == 5 ]] || fail "a terminal run is not verbose: $out"
grep -q "^== bash .*pass\.sh$" <<< "$out" || fail "a terminal run does not print the command: $out"
run CLAUDECODE=1 CI=true -- step bash "$tmp/pass.sh"
[[ "$(grep -c 'ok$' <<< "$out")" == 5 ]] || fail "a run under CI is not verbose: $out"
run CLAUDECODE=1 CHECK_VERBOSE=1 -- step bash "$tmp/pass.sh"
[[ "$(grep -c 'ok$' <<< "$out")" == 5 ]] || fail "CHECK_VERBOSE=1 is not verbose: $out"
run CLAUDECODE=1 -- --verbose step bash "$tmp/pass.sh"
[[ "$(grep -c 'ok$' <<< "$out")" == 5 ]] || fail "--verbose is not verbose: $out"
run -- step bash "$tmp/failing.sh"
[[ "$code" == 1 ]] || fail "a failing terminal step exited $code: $out"
grep -q "test t1 ... ok" <<< "$out" || fail "a failing terminal step does not print everything: $out"

# A step with STEP_EXPECT prints a start line with the expected time, also under an agent; one without prints none
run CLAUDECODE=1 STEP_EXPECT="about 3 min warm" -- step bash "$tmp/pass.sh"
[[ "$code" == 0 ]] || fail "a step with an expectation exited $code: $out"
[[ "$(wc -l <<< "$out")" == 2 ]] || fail "a quiet step with an expectation is not a start line and an end line: $out"
head -n 1 <<< "$out" | grep -Eq "^== bash .*pass\.sh \(expected about 3 min warm\)$" || fail "the start line has no hint: $out"
tail -n 1 <<< "$out" | grep -Eq "pass\.sh: passed in [0-9]+ s, log " || fail "the end line changed: $out"
run STEP_EXPECT="about 3 min warm" -- step bash "$tmp/pass.sh"
grep -q "^== bash .*pass\.sh (expected about 3 min warm)$" <<< "$out" || fail "the verbose start line has no hint: $out"
run CLAUDECODE=1 STEP_EXPECT="about 3 min warm" -- step bash "$tmp/failing.sh"
[[ "$code" == 1 ]] || fail "a failing step with an expectation exited $code: $out"
grep -q "== Failed: bash .*failing\.sh (after" <<< "$out" || fail "the failure line changed: $out"

# The constants of the long steps' hints are the ones of the cold-target message, so they cannot drift apart
cold="$(grep -E '^EXPECT_COLD_MIN=' "$root/scripts/check.sh" | cut -d= -f2)"
test_warm="$(grep -E '^EXPECT_TEST_WARM_MIN=' "$root/scripts/check.sh" | cut -d= -f2)"
[[ -n "$cold" && -n "$test_warm" ]] || fail "check.sh names no EXPECT_COLD_MIN or EXPECT_TEST_WARM_MIN"
# shellcheck disable=SC2016 # the text searched for holds literal ${...}
grep -q 'STEP_EXPECT="about ${EXPECT_TEST_WARM_MIN} min warm, about ${EXPECT_COLD_MIN} min cold"' "$root/scripts/check.sh" || fail "the workspace test step does not use the constants"
# shellcheck disable=SC2016
grep -q 'about ${EXPECT_COLD_MIN} minutes in a cloud session' "$root/scripts/check.sh" || fail "the cold-target message does not use EXPECT_COLD_MIN"

# The builder's rule names git -C as refused
[[ "$(grep -c 'git -C' "$root/.claude/skills/next/SKILL.md")" -ge 1 ]] || fail "SKILL.md does not name git -C"

# The numbers of the logs count up, one per step
run CLAUDECODE=1 -- step bash "$tmp/pass.sh"
grep -q "/01-" <<< "$out" || fail "a new run does not start at 01: $out"

# A gate ends with one last line, CHECK DONE rc=<exit code>, in every mode; the exit code is unchanged
run CLAUDECODE=1 -- gate-step bash "$tmp/pass.sh"
[[ "$code" == 0 ]] || fail "a passing gate exited $code: $out"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=0" ]] || fail "a passing gate does not end with CHECK DONE rc=0: $out"
[[ "$(grep -c 'CHECK DONE' <<< "$out")" == 1 ]] || fail "CHECK DONE is not printed exactly once: $out"
run CLAUDECODE=1 -- gate-step bash "$tmp/failing.sh"
[[ "$code" == 1 ]] || fail "a failing gate exited $code, not 1: $out"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=1" ]] || fail "a failing gate does not end with CHECK DONE rc=1: $out"
[[ "$(grep -n 'CHECK DONE' <<< "$out" | cut -d: -f1)" -gt "$(grep -n '== Failed' <<< "$out" | cut -d: -f1)" ]] || fail "CHECK DONE is not after the Failed lines: $out"
run -- gate-step bash "$tmp/pass.sh"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=0" ]] || fail "a terminal gate does not end with CHECK DONE: $out"
run CLAUDECODE=1 CI=true -- gate-step bash "$tmp/pass.sh"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=0" ]] || fail "a gate under CI does not end with CHECK DONE: $out"
run CLAUDECODE=1 -- --verbose gate-step bash "$tmp/failing.sh"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=1" ]] || fail "a --verbose gate does not end with CHECK DONE: $out"
# A stop at the disk check is a gate that ends too: the real full run, with an impossible floor
run CLAUDECODE=1 CHECK_MIN_FREE_GB=99999999 -- full
[[ "$code" == 1 ]] || fail "a run stopped at the disk check exited $code: $out"
[[ "$(tail -n 1 <<< "$out")" == "CHECK DONE rc=1" ]] || fail "a run stopped at the disk check does not end with CHECK DONE rc=1: $out"
# The modes that are parsed or report to the hook, and the one-liners, do not print it
run -- scope full
[[ "$out" == full ]] || fail "scope printed more than its answer: $out"
run -- step bash "$tmp/pass.sh"
if grep -q 'CHECK DONE' <<< "$out"; then fail "step prints CHECK DONE: $out"; fi
# The real branches of the final case use the gate: "", full and quick, and no other mode
branches="$(awk '/^case "\$\{1:-\}" in/,/^esac/' "$root/scripts/check.sh")"
[[ "$(grep -cE '^("\"|full|quick)\) gate ' <<< "$branches")" == 3 ]] || fail "the \"\", full and quick branches do not all run through gate"
if grep -E '^(scope|modes|notes|space|target|deps|hook|step)\)' <<< "$branches" | grep -q 'gate'; then fail "a one-line mode, scope or hook runs through gate"; fi
# The two places of the builder's rule name the line, so the rule cannot drift from the script
[[ "$(grep -c 'CHECK DONE' "$root/.claude/skills/next/SKILL.md")" -ge 2 ]] || fail "SKILL.md does not name CHECK DONE in both places"
grep -q "wait for its \`CHECK DONE\` line" "$root/.claude/skills/next/SKILL.md" || fail "the gates step of SKILL.md does not name CHECK DONE"

# A step that fails on the lock file prints one hint line, in quiet and in verbose mode; another failure prints none
cat > "$tmp/locked.sh" << 'EOF'
echo "error: the lock file /x/Cargo.lock needs to be updated but --locked was passed to prevent this"
exit 101
EOF
hint="Cargo.lock is out of date: run cargo update -p <package> --offline (the package you changed), then run the gate again"
run CLAUDECODE=1 -- step bash "$tmp/locked.sh"
[[ "$code" == 1 ]] || fail "a lock-file failure exited $code, not 1: $out"
[[ "$(grep -cF "$hint" <<< "$out")" == 1 ]] || fail "quiet mode does not print the lock hint exactly once: $out"
run -- step bash "$tmp/locked.sh"
[[ "$code" == 1 ]] || fail "a verbose lock-file failure exited $code, not 1: $out"
[[ "$(grep -cF "$hint" <<< "$out")" == 1 ]] || fail "verbose mode does not print the lock hint exactly once: $out"
run CLAUDECODE=1 -- step bash "$tmp/failing.sh"
if grep -qF "Cargo.lock is out of date" <<< "$out"; then fail "a failure without the lock error prints the hint: $out"; fi
run -- step bash "$tmp/failing.sh"
if grep -qF "Cargo.lock is out of date" <<< "$out"; then fail "a verbose failure without the lock error prints the hint: $out"; fi

# The commit hook stays verbose inside (it reports a failure's last 60 lines itself)
awk '/^hook\(\) \{/,/^}/' "$root/scripts/check.sh" | grep -q 'CHECK_VERBOSE=1' || fail "hook() does not force the full output"

echo "scripts/test-check-quiet.sh: all good"
