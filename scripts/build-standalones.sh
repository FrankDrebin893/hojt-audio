#!/usr/bin/env bash
# Builds the six standalone programs (release; on Windows with ASIO beside WASAPI and the dummy backend,
# from the SDK in third_party/asiosdk, see its README.md) and puts each at target/standalone/<BundleName>.exe, the names of
# bundler.toml, for the installer (scripts/build-installer.sh). Run in Git Bash on Windows:
#
#   scripts/build-standalones.sh
#
# On another system it makes the same programs without the .exe and without ASIO, which is how it is
# tried. HOJT_NO_ASIO=1 leaves ASIO out on Windows too (CPAL_ASIO_DIR, if set, names another SDK folder).
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
features="standalone"
if [[ -n "$exe" && -z "${HOJT_NO_ASIO:-}" ]]; then
  asio_dir="${CPAL_ASIO_DIR:-$root/third_party/asiosdk}"
  [[ -f "$asio_dir/common/asio.h" ]] || { echo "no ASIO SDK at $asio_dir" >&2; exit 1; }
  command -v cygpath > /dev/null && asio_dir="$(cygpath -w "$asio_dir")"
  export CPAL_ASIO_DIR="$asio_dir"
  features="standalone,asio"
fi
(cd "$root" && cargo build --release --locked "${args[@]}" --features "$features")

rm -rf "$out"
mkdir -p "$out"
while read -r pkg name; do
  src="$root/target/release/${pkg}_standalone$exe"
  [[ -f "$src" ]] || { echo "missing $src" >&2; exit 1; }
  cp "$src" "$out/$name$exe"
  echo "standalone $out/$name$exe"
done <<< "$pairs"
