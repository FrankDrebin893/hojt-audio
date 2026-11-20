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

# The numbers of the logs count up, one per step
run CLAUDECODE=1 -- step bash "$tmp/pass.sh"
grep -q "/01-" <<< "$out" || fail "a new run does not start at 01: $out"

# The commit hook stays verbose inside (it reports a failure's last 60 lines itself)
awk '/^hook\(\) \{/,/^}/' "$root/scripts/check.sh" | grep -q 'CHECK_VERBOSE=1' || fail "hook() does not force the full output"

echo "scripts/test-check-quiet.sh: all good"
