#!/usr/bin/env bash
# What CI runs (.github/workflows/ci.yml), run here before a push, the same for every agent tool.
#
#   scripts/check.sh            every step of CI: format, clippy, the tests, the amp's tests
#                               optimised. Exits 1 at the first step that fails
#   scripts/check.sh quick      format and clippy only
#   scripts/check.sh hook       the Claude Code PreToolUse hook (.claude/settings.json): the quick
#                               part before a `git commit`, blocking the commit when it fails
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
        exit 1
    fi
}

# The toolchain of the lint job ("dtolnay/rust-toolchain@1.93.0"), as `+1.93.0` if rustup has it.
lint_toolchain() {
    local version
    version="$(grep -o 'rust-toolchain@[0-9][0-9.]*' "$ci" | head -1 | cut -d@ -f2)"
    if [[ -n "$version" ]] && rustup run "$version" rustc --version > /dev/null 2>&1; then
        echo "+$version"
    else
        echo "Rust $version, which CI lints with, is not installed (rustup toolchain install $version" \
            "--component rustfmt clippy); format and clippy run on the default toolchain" >&2
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

quick() {
    local tc
    tc="$(lint_toolchain)"
    step cargo $tc fmt --all --check
    step cargo $tc clippy --workspace --all-targets --locked -- -D warnings
}

full() {
    check_linux_packages
    quick
    step cargo test --workspace --locked
    # The clippers' two-at-a-time path is only compiled in optimised x86-64 builds
    step cargo test -p amp --release --locked
    echo "== All of CI's steps pass"
}

# Claude Code passes the tool call (Bash or PowerShell) as JSON on stdin; only a `git commit`
# (or `git -C <dir> commit`) is checked, not `git commit-tree`. Exit 2 blocks the commit and hands
# what failed back to the agent.
hook() {
    grep -Eq '"command".*git( +-C +[^ ]+)? +commit([^-a-z]|$)' || exit 0
    local out
    if ! out="$(quick 2>&1)"; then
        {
            echo "scripts/check.sh quick failed, so the commit was stopped. Fix this, then commit again"
            echo "(\`cargo fmt --all\` fixes the format):"
            echo "$out" | tail -n 60
        } >&2
        exit 2
    fi
}

case "${1:-}" in
"") full ;;
quick) quick ;;
hook) hook ;;
*)
    echo "usage: scripts/check.sh [quick|hook]" >&2
    exit 64
    ;;
esac
