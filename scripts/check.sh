#!/usr/bin/env bash
# What CI runs (.github/workflows/ci.yml), run here before a push.
#
#   scripts/check.sh            every step of CI: format, clippy, the tests, the amp's tests
#                               optimised. Exits 1 at the first step that fails
#                               when the branch changes only light paths (scripts/light-paths.txt: the page,
#                               markdown, .claude/...) it does what CI does for them: quick, the page's links,
#                               and xtask's tests if a skill changed
#   scripts/check.sh full       every step, whatever the diff
#   scripts/check.sh scope      prints light, skills or full: what the run above would do
#   scripts/check.sh quick      the package check, the script modes, format and clippy only
#   scripts/check.sh deps       installs the Linux packages CI installs that are missing (sudo apt-get, no sudo
#                               as root); quick and the full run only name them when they are missing
#   scripts/check.sh space      only the disk-space check a full run starts with: prints the free space and every worktree's
#                               target/ size, stops under 8 GB (cold target/) or 2 GB (warm); CHECK_MIN_FREE_GB overrides
#   scripts/check.sh notes      only that every plugins/amp/notes/*.md is linked from the amp's router and every link exists
#   scripts/check.sh modes      only that every tracked scripts/*.sh is executable in git's index
#   scripts/check.sh --verbose  (before any mode) the full output of every step, also under an agent: where
#                               CLAUDECODE=1 and CI is not set, a step prints one line (passed, seconds, its log
#                               in target/check-logs) and, when it fails, the failing lines and the log's tail
#                               full and quick end with one line, `CHECK DONE rc=<exit code>`, pass or fail
#   scripts/check.sh hook       the Claude Code PreToolUse hook (.claude/settings.json): the quick
#                               part before a `git commit`, blocking the commit when it fails
#                               in the tree the commit is made in (a worktree's own, not this checkout's)
#
# On Linux it first says which of the packages CI installs are missing. Format and clippy run on
# the Rust that ci.yml pins when rustup has it, as CI does; otherwise it says so and uses the default.
# Runs from Git Bash on Windows too.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
ci=.github/workflows/ci.yml

# The packages CI installs on Linux, read from ci.yml so they are listed in one place.
linux_packages() {
    grep -m1 'apt-get install -y' "$ci" | sed 's/.*apt-get install -y//'
}

check_linux_packages() {
    [[ "$(uname -s)" == Linux ]] || return 0
    command -v dpkg-query > /dev/null || {
        echo "Not a Debian-like system: make sure the equivalents of these are installed:$(linux_packages)"
        return 0
    }
    local missing=()
    for p in $(linux_packages); do
        dpkg-query -W -f='${Status}' "$p" 2> /dev/null | grep -q 'install ok installed' || missing+=("$p")
    done
    if ((${#missing[@]})); then
        echo "Missing Linux packages, which the build needs:" >&2
        echo "  sudo apt-get install -y ${missing[*]}" >&2
        echo "  or: scripts/check.sh deps" >&2
        exit 1
    fi
}

# Installs the packages CI installs that are missing (sudo unless already root). Never run on its own by
# another mode: an unasked apt-get before every commit would be a surprise.
deps() {
    if [[ "$(uname -s)" != Linux ]] || ! command -v apt-get > /dev/null; then
        echo "scripts/check.sh deps: needs a Debian-like Linux with apt-get" >&2
        exit 1
    fi
    local missing=() sudo=""
    for p in $(linux_packages); do
        dpkg-query -W -f='${Status}' "$p" 2> /dev/null | grep -q 'install ok installed' || missing+=("$p")
    done
    if ((${#missing[@]} == 0)); then
        echo "== All of CI's Linux packages are installed"
        return 0
    fi
    if [[ "$(id -u)" != 0 ]]; then
        command -v sudo > /dev/null || {
            echo "scripts/check.sh deps: not root and no sudo; install as root: apt-get install -y ${missing[*]}" >&2
            exit 1
        }
        sudo=sudo
    fi
    step $sudo apt-get update -qq
    DEBIAN_FRONTEND=noninteractive step $sudo apt-get install -y -qq --no-install-recommends "${missing[@]}"
}

# The toolchain of the lint job ("dtolnay/rust-toolchain@1.100.0"), as `+1.100.0` if rustup has it.
lint_toolchain() {
    local version
    version="$(grep -o 'rust-toolchain@[0-9][0-9.]*' "$ci" | head -1 | cut -d@ -f2)"
    if [[ -n "$version" ]] && rustup run "$version" rustc --version > /dev/null 2>&1; then
        echo "+$version"
    else
        echo "Rust $version, which CI lints with, is not installed (rustup toolchain install $version" \
            "--component rustfmt clippy); format and clippy run on the default toolchain, whose clippy may" \
            "report lints CI does not" >&2
    fi
}

# Quiet mode, for an agent (issue 376): Claude Code sets CLAUDECODE=1 in its shell. A step then keeps its
# output in a log and prints one line; a person's terminal, CI (CI is set) and --verbose / CHECK_VERBOSE=1
# print everything as before.
quiet_steps() {
    [[ "${CLAUDECODE:-}" == 1 && -z "${CI:-}" && "${CHECK_VERBOSE:-}" != 1 ]]
}

# The expected times of the long steps, in minutes, written once: the start line of a step (STEP_EXPECT) and the
# cold-target message both read them. Measured in scripts/README.md, Testing; "about", since the suite grows.
EXPECT_TEST_WARM_MIN=11
EXPECT_OPT_WARM_MIN=8
EXPECT_COLD_MIN=30

step_number=0

# The log of a quiet step: <log dir>/NN-<the command, as a slug>.log. The log dir is target/check-logs (not committed).
step_log() {
    local slug
    slug="$(printf '%s' "$*" | tr -c 'A-Za-z0-9.\n' '-' | tr -s '-' | cut -c1-40)"
    printf '%s/%02d-%s.log' "${CHECK_LOG_DIR:-target/check-logs}" "$step_number" "${slug#-}"
}

# Stops at the first failure itself: `set -e` does not hold inside the hook's `if`.
# An optional STEP_EXPECT text (set on the line before the call, the step clears it) is the step's expected time: printed with the start line, which
# a quiet step otherwise does not have.
step() {
    local expect=""
    [[ -z "${STEP_EXPECT:-}" ]] || expect=" (expected ${STEP_EXPECT})"
    STEP_EXPECT=
    if ! quiet_steps; then
        echo "== $*$expect"
        "$@" || {
            echo "== Failed: $*" >&2
            exit 1
        }
        return 0
    fi
    local log started
    step_number=$((step_number + 1))
    log="$(step_log "$@")"
    mkdir -p "$(dirname "$log")"
    [[ -z "$expect" ]] || echo "== $*$expect"
    started=$SECONDS
    if "$@" > "$log" 2>&1; then
        echo "== $*: passed in $((SECONDS - started)) s, log $log"
    else
        echo "== Failed: $* (after $((SECONDS - started)) s, log $log)" >&2
        echo "-- Failing lines:" >&2
        { grep -E 'FAILED|panicked|error' "$log" || true; } | head -n 40 >&2
        echo "-- Last 60 lines of the log:" >&2
        tail -n 60 "$log" >&2
        exit 1
    fi
}

# Every tracked scripts/*.sh has mode 100755 in the index, so it runs without `bash` in front. Read from
# the index (`git ls-files -s`), not the file system, so it holds on Windows too.
script_modes() {
    local bad=0 mode path
    while read -r mode _ _ path; do
        if [[ "$mode" != 100755 ]]; then
            echo "scripts/check.sh: $path is mode $mode, not 100755; fix it with: git update-index --chmod=+x $path" >&2
            bad=1
        fi
    done < <(git ls-files -s -- 'scripts/*.sh')
    ((bad == 0)) || exit 1
    echo "== Every scripts/*.sh is executable"
}

# The amp's router (plugins/amp/CLAUDE.md) and its topic files (plugins/amp/notes/*.md, issue 370): every
# topic file is named in the router, and every `notes/<name>.md` the router names is there.
amp_notes() {
    local dir="${AMP_NOTES_DIR:-plugins/amp}" bad=0 f name router
    router="$dir/CLAUDE.md" # AMP_NOTES_DIR: a copy, for scripts/test-check-notes.sh
    for f in "$dir"/notes/*.md; do
        name="notes/$(basename "$f")"
        if ! grep -Fq "\`$name\`" "$router"; then
            echo "scripts/check.sh: $name is not linked from $router; add it to the index" >&2
            bad=1
        fi
    done
    # shellcheck disable=SC2016 # the backticks are literal: the router writes its links in code spans
    while IFS= read -r name; do
        if [[ ! -f "$dir/$name" ]]; then
            echo "scripts/check.sh: $router names $name, which does not exist" >&2
            bad=1
        fi
    done < <(grep -o '`notes/[A-Za-z0-9_.-]*\.md`' "$router" | tr -d '`' | sort -u)
    ((bad == 0)) || exit 1
    echo "== The amp's router and its topic files agree"
}

quick() {
    local tc
    check_linux_packages
    script_modes
    amp_notes
    tc="$(lint_toolchain)"
    # $tc is `+1.100.0` or empty, so it stays unquoted
    # shellcheck disable=SC2086
    step cargo $tc fmt --all --check
    # shellcheck disable=SC2086
    step cargo $tc clippy --workspace --all-targets --locked -- -D warnings
}

# What a run has to do, from the files this branch changes against origin/main (CHECK_BASE, to test it):
#   light   only light paths (scripts/light-paths.txt, the list the `changes` job of ci.yml reads too)
#   skills  light, and some of it under .claude/skills/, which xtask's tests read
#   full    anything else, no diff, or no base to compare with
# Committed, uncommitted and untracked files all count.
scope() {
    local base files rest
    [[ "${1:-}" == full ]] && {
        echo full
        return 0
    }
    base="$(git merge-base "${CHECK_BASE:-origin/main}" HEAD 2> /dev/null)" || {
        echo full
        return 0
    }
    files="$({
        git diff --name-only "$base"
        git ls-files --others --exclude-standard
    } | sort -u)"
    if [[ -z "$files" ]]; then
        echo full
        return 0
    fi
    rest="$(printf '%s\n' "$files" | grep -vEf scripts/light-paths.txt || true)"
    if [[ -n "$rest" ]]; then
        echo full
    elif printf '%s\n' "$files" | grep -qE '^\.claude/skills/'; then
        echo skills
    else
        echo light
    fi
}

# Prints one line per target/ directory of the main checkout and every worktree, largest first, then the total
# (`du -sk`, no network): what to free first, without reading du by hand.
list_worktree_targets() {
    local path kb total=0 rows=""
    while IFS= read -r path; do
        [[ -d "$path/target" ]] || continue
        kb="$(du -sk "$path/target" 2> /dev/null | awk '{print $1}')"
        [[ "$kb" =~ ^[0-9]+$ ]] || continue
        total=$((total + kb))
        rows+="$kb $path/target"$'\n'
    done < <(git worktree list --porcelain 2> /dev/null | sed -n 's/^worktree //p')
    echo "== target/ directories of the worktrees, largest first:"
    if [[ -z "$rows" ]]; then
        echo "   (none)"
        return 0
    fi
    printf '%s' "$rows" | sort -rn | awk '{ k = $1; sub(/^[0-9]+ /, ""); printf "  %5.1f GB  %s\n", k / 1024 / 1024, $0 }'
    awk -v k="$total" 'BEGIN { printf "  %5.1f GB  total\n", k / 1024 / 1024 }'
}

# Prints the free disk space of the checkout's file system and stops under a floor: a full build fills a disk
# mid-test otherwise (every issue with a target directory of its own did exactly that). The floor is 8 GB when the
# checkout's target/ is cold (no debug/deps) and 2 GB when it is warm. One cold build is 2 to 7 GB, with its .rmeta
# and incremental data; an incremental rebuild needs little. CHECK_MIN_FREE_GB overrides both. The sizes of every
# worktree's target/ are printed when it stops, and ($1 = list, the `space` mode) also when it does not.
check_disk_space() {
    local kind=warm floor_gb=2 min_kb avail_kb gb
    if target_is_cold "$root/target"; then kind=cold floor_gb=8; fi
    if [[ "${CHECK_MIN_FREE_GB:-}" =~ ^[0-9]+$ ]]; then floor_gb="$CHECK_MIN_FREE_GB"; fi
    min_kb=$((floor_gb * 1024 * 1024))
    avail_kb="$(df -Pk "$root" 2> /dev/null | awk 'NR==2 {print $4}')"
    if [[ ! "$avail_kb" =~ ^[0-9]+$ ]]; then
        echo "== Free disk space: unknown (df gave no number), going on"
        return 0
    fi
    gb="$(awk -v k="$avail_kb" 'BEGIN { printf "%.1f", k / 1024 / 1024 }')"
    echo "== Free disk space: $gb GB free (floor $floor_gb GB, target/ $kind)"
    if ((avail_kb < min_kb)); then
        {
            echo "scripts/check.sh: only $gb GB free, a $kind target/ needs $floor_gb GB or more. Free some first:"
            list_worktree_targets
            echo "  - the largest first: cargo clean in your own worktree (a build uses its worktree's own target/, never a directory another session can name)"
            echo "  - a worktree whose branch is merged: git worktree remove <path>"
            echo "  - cargo's cache: rm -rf ~/.cargo/registry/cache"
            echo "  - a person whose disk is large: CHECK_MIN_FREE_GB=<n> scripts/check.sh"
        } >&2
        exit 1
    fi
    if [[ "${1:-}" == list ]]; then list_worktree_targets; fi
}

# Succeeds when the target directory ($1) is cold: no debug/deps, so the test step compiles everything. The one
# test check_cold_target and check_disk_space share.
target_is_cold() { [[ ! -d "$1/debug/deps" ]]; }

# Says whether the target directory ($1) is cold. Only prints.
check_cold_target() {
    if ! target_is_cold "$1"; then
        echo "== Target directory is warm"
    else
        echo "== Target directory is cold: the test step compiles the whole workspace from scratch (about ${EXPECT_COLD_MIN} minutes in a cloud session)."
        echo "   For a change confined to some crates: scripts/check.sh quick, then cargo test -p <touched crates>; CI's Test is the proof of the rest."
    fi
}

# Prints the target directory in use and how many other cargo processes run, and stops when CARGO_TARGET_DIR
# points outside the checkout: two sessions naming one directory fight over cargo's build lock (a 45-minute
# gate). CHECK_SHARED_TARGET=1 is for a person whose machine is theirs alone.
check_target_dir() {
    local target="${CARGO_TARGET_DIR:-$root/target}" abs
    if [[ "$target" == /* ]]; then abs="$target"; else abs="$PWD/$target"; fi
    abs="$(realpath -m "$abs" 2> /dev/null || echo "$abs")"
    echo "== Target directory: $abs"
    if command -v pgrep > /dev/null; then
        echo "== Other cargo processes running: $(pgrep -x cargo | wc -l | tr -d ' ')"
    fi
    check_cold_target "$abs"
    if [[ -n "${CARGO_TARGET_DIR:-}" && "$abs" != "$root" && "$abs" != "$root"/* && "${CHECK_SHARED_TARGET:-}" != 1 ]]; then
        {
            echo "scripts/check.sh: CARGO_TARGET_DIR is $abs, outside this checkout ($root)."
            echo "  Another session can name the same directory and hold cargo's build lock for the whole gate."
            echo "  Fix: unset CARGO_TARGET_DIR (the checkout's own target/ is used)."
            echo "  On a machine that is yours alone: CHECK_SHARED_TARGET=1 scripts/check.sh"
        } >&2
        exit 1
    fi
}

full() {
    local kind
    kind="$(scope "${1:-}")"
    if [[ "$kind" != full ]]; then
        echo "== Only light paths changed against origin/main: the same as CI, no Rust tests (scripts/check.sh full forces them)"
        quick
        step bash scripts/page-check.sh links
        [[ "$kind" == skills ]] && step cargo test -p xtask --locked
        echo "== All of CI's steps for this change pass"
        return 0
    fi
    check_target_dir
    check_disk_space
    quick
    STEP_EXPECT="about ${EXPECT_TEST_WARM_MIN} min warm, about ${EXPECT_COLD_MIN} min cold"
    step cargo test --workspace --locked
    # The clippers' two-at-a-time path is only compiled in optimised x86-64 builds, and the bass's
    # cost bound only means something optimised
    STEP_EXPECT="about ${EXPECT_OPT_WARM_MIN} min warm, about ${EXPECT_COLD_MIN} min cold"
    step cargo test -p amp -p bass --profile optimised --locked
    [[ "$(uname -s)" == Linux ]] && step bash scripts/test-linux-install.sh
    step bash scripts/test-macos-pkg.sh
    step bash scripts/test-pin-nih-plug.sh
    step bash scripts/test-check-light.sh
    step bash scripts/test-check-space.sh
    step bash scripts/test-check-target.sh
    step bash scripts/test-check-notes.sh
    step bash scripts/test-check-quiet.sh
    echo "== All of CI's steps pass"
}

# Claude Code passes the tool call (Bash or PowerShell) as JSON on stdin; only a `git commit`
# (or `git -C <dir> commit`) is checked, not `git commit-tree`. Exit 2 blocks the commit and hands
# what failed back to the agent.
#
# The tree that is checked is the one the commit is made in: `git -C <dir>` when the command names
# one, otherwise the tool's working directory (`cwd`). A session in a worktree has this script's
# own checkout as its project directory, and that one may hold another branch altogether.
hook() {
    local input out dir top
    export CHECK_VERBOSE=1 # the output is reported only on a failure, and its tail is cut below
    input="$(cat)"
    grep -Eq '"command".*git( +-C +[^ ]+)? +commit([^-a-z]|$)' <<< "$input" || exit 0
    dir="$(sed -nE 's/.*"command".*git +-C +\\?"?([^ "\\]+).* +commit([^-a-z].*|$)/\1/p' <<< "$input" | head -1)"
    [[ -n "$dir" ]] || dir="$(sed -nE 's/.*"cwd" *: *"(([^"\\]|\\.)*)".*/\1/p' <<< "$input" | head -1)"
    dir="${dir//\\\\//}" # JSON writes a Windows path's backslash twice; Git Bash takes D:/a/b
    if [[ -n "$dir" ]] && top="$(git -C "$dir" rev-parse --show-toplevel 2> /dev/null)" &&
        [[ -f "$top/scripts/check.sh" && -f "$top/$ci" ]]; then
        cd "$top"
    fi
    if ! out="$(quick 2>&1)"; then
        {
            echo "scripts/check.sh quick failed in $PWD, so the commit was stopped. Fix this, then commit again"
            echo "(\`cargo fmt --all\` fixes the format):"
            echo "$out" | tail -n 60
        } >&2
        exit 2
    fi
}

if [[ "${1:-}" == --verbose ]]; then
    export CHECK_VERBOSE=1
    shift
fi

# Runs a gate ($@) and ends with one line on stdout whatever happens, a pass, a failed step or a stop at the disk or
# target check: `CHECK DONE rc=<exit code>`. A run started in the background and redirected to a file is waited on
# with `until grep -q 'CHECK DONE' <file>` (.claude/skills/next/SKILL.md, "The builder"). The exit code is unchanged.
# Only the modes that run a gate use it: `scope` is parsed by other scripts, `hook` reports to the commit hook.
gate() {
    trap 'echo "CHECK DONE rc=$?"' EXIT
    "$@"
}

case "${1:-}" in
"") gate full ;;
full) gate full full ;;
scope) scope "${2:-}" ;;
quick) gate quick ;;
modes) script_modes ;;
notes) amp_notes ;;
space) check_disk_space list ;;
target) check_target_dir ;;
deps) deps ;;
hook) hook ;;
step) # one step on its own, what scripts/test-check-quiet.sh runs
    shift
    step "$@"
    ;;
gate-step) # one step on its own inside `gate`, what scripts/test-check-quiet.sh runs to test the last line
    shift
    gate step "$@"
    ;;
*)
    echo "usage: scripts/check.sh [--verbose] [full|quick|scope|modes|notes|space|deps|hook]" >&2
    exit 64
    ;;
esac
