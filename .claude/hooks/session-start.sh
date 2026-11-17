#!/bin/bash
# The SessionStart hook of a Claude Code cloud session (.claude/settings.json): what CI has and a
# fresh cloud container lacks, so `scripts/check.sh` runs there in full and nothing is left
# "for CI to check". Elsewhere (a desktop, a CI runner) it does nothing.
#
#   - the Linux packages ci.yml installs (read from ci.yml, so they are listed in one place)
#   - shellcheck, which scripts/test-linux-install.sh and scripts/test-macos-pkg.sh run when it is there
#   - the Rust toolchain ci.yml pins for format and clippy
#
# Only what is missing is installed, so a second start costs nothing.
set -euo pipefail

[ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || exit 0

cd "${CLAUDE_PROJECT_DIR:-.}"
ci=.github/workflows/ci.yml
[ -f "$ci" ] || exit 0

as_root() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

packages="$(grep -m1 'apt-get install -y' "$ci" | sed 's/.*apt-get install -y//') shellcheck"
missing=()
for p in $packages; do
    dpkg-query -W -f='${Status}' "$p" 2> /dev/null | grep -q 'install ok installed' || missing+=("$p")
done
if ((${#missing[@]})); then
    echo "session-start: installing ${missing[*]}" >&2
    as_root apt-get update -qq
    DEBIAN_FRONTEND=noninteractive as_root apt-get install -y -qq --no-install-recommends "${missing[@]}" > /dev/null
fi

version="$(grep -o 'rust-toolchain@[0-9][0-9.]*' "$ci" | head -1 | cut -d@ -f2)"
if [ -n "$version" ] && command -v rustup > /dev/null &&
    ! rustup run "$version" cargo clippy --version > /dev/null 2>&1; then
    echo "session-start: installing Rust $version (rustfmt, clippy)" >&2
    rustup toolchain install "$version" --profile minimal --component rustfmt --component clippy --no-self-update
fi
