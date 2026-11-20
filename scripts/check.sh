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
#   scripts/check.sh space      only the disk-space check a full run starts with: prints the free space, stops under 2 GB
#   scripts/check.sh notes      only that every plugins/amp/notes/*.md is linked from the amp's router and every link exists
#   scripts/check.sh modes      only that every tracked scripts/*.sh is executable in git's index
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

# The toolchain of the lint job ("dtolnay/rust-toolchain@1.99.1"), as `+1.99.1` if rustup has it.
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

# Stops at the first failure itself: `set -e` does not hold inside the hook's `if`.
step() {
    echo "== $*"
    "$@" || {
        echo "== Failed: $*" >&2
        exit 1
    }
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
    local router=plugins/amp/CLAUDE.md dir=plugins/amp bad=0 f name
    for f in "$dir"/notes/*.md; do
        name="notes/$(basename "$f")"
        if ! grep -Fq "\`$name\`" "$router"; then
            echo "scripts/check.sh: $name is not linked from $router; add it to the index" >&2
            bad=1
        fi
    done
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
    # $tc is `+1.99.1` or empty, so it stays unquoted
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

# Prints the free disk space of the checkout's file system and stops under 2 GB: a full build fills a disk
# mid-test otherwise (every issue with a target directory of its own did exactly that).
check_disk_space() {
    local min_kb=$((2 * 1024 * 1024)) avail_kb
    avail_kb="$(df -Pk "$root" 2> /dev/null | awk 'NR==2 {print $4}')"
    if [[ ! "$avail_kb" =~ ^[0-9]+$ ]]; then
        echo "== Free disk space: unknown (df gave no number), going on"
        return 0
    fi
    local gb
    gb="$(awk -v k="$avail_kb" 'BEGIN { printf "%.1f", k / 1024 / 1024 }')"
    echo "== Free disk space: $gb GB free"
    if ((avail_kb < min_kb)); then
        {
            echo "scripts/check.sh: only $gb GB free, a full run needs 2 GB or more. Free some first:"
            echo "  - the target directories of merged issues: rm -rf /tmp/target-<issue> (a cloud session uses one"
            echo "    shared target directory, the worktree's own target/ or /tmp/target, never one per issue)"
            echo "  - worktrees of merged branches: git worktree remove <path>, then their target/ directories"
            echo "  - cargo's cache: cargo clean, or rm -rf ~/.cargo/registry/cache"
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
    check_disk_space
    quick
    step cargo test --workspace --locked
    # The clippers' two-at-a-time path is only compiled in optimised x86-64 builds, and the bass's
    # cost bound only means something optimised
    step cargo test -p amp -p bass --profile optimised --locked
    [[ "$(uname -s)" == Linux ]] && step bash scripts/test-linux-install.sh
    step bash scripts/test-macos-pkg.sh
    step bash scripts/test-pin-nih-plug.sh
    step bash scripts/test-check-light.sh
    step bash scripts/test-check-space.sh
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

case "${1:-}" in
"") full ;;
full) full full ;;
scope) scope "${2:-}" ;;
quick) quick ;;
modes) script_modes ;;
notes) amp_notes ;;
space) check_disk_space ;;
deps) deps ;;
hook) hook ;;
*)
    echo "usage: scripts/check.sh [full|quick|scope|modes|notes|space|deps|hook]" >&2
    exit 64
    ;;
esac
