#!/usr/bin/env bash
# Builds the six standalone programs (release, WASAPI and the dummy backend; ASIO is built by hand,
# see plugins/amp/CLAUDE.md) and puts each at target/standalone/<BundleName>.exe, the names of
# bundler.toml, for the installer (scripts/build-installer.sh). Run in Git Bash on Windows:
#
#   scripts/build-standalones.sh
#
# On another system it makes the same programs without the .exe, which is how it is tried.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/standalone"
exe=""
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exe=".exe" ;; esac

# The packages and their bundle names, from bundler.toml: "[pkg]" then "name = ..."
pairs=$(awk -F'"' '/^\[/ { gsub(/[\[\]]/, ""); pkg = $0 } /^name = / { print pkg " " $2 }' "$root/bundler.toml")
[[ -n "$pairs" ]] || { echo "no bundles in bundler.toml" >&2; exit 1; }

args=()
while read -r pkg _; do args+=(-p "$pkg"); done <<< "$pairs"
(cd "$root" && cargo build --release --locked "${args[@]}" --features standalone)

rm -rf "$out"
mkdir -p "$out"
while read -r pkg name; do
  src="$root/target/release/${pkg}_standalone$exe"
  [[ -f "$src" ]] || { echo "missing $src" >&2; exit 1; }
  cp "$src" "$out/$name$exe"
  echo "standalone $out/$name$exe"
done <<< "$pairs"
