#!/usr/bin/env bash
# Builds dist/HojtDSP-Setup.exe, the Windows installer, from the bundles in target/bundled
# (Inno Setup 6; the windows-latest runner has it). Run from Git Bash after `cargo xtask bundle`:
#
#   scripts/build-installer.sh
#
# The version is the one in Cargo.toml. It stops if a bundle named in bundler.toml is missing or
# has no line in installer/hojt-dsp.iss. To sign, set HOJT_SIGN_COMMAND to a command that signs
# the file it is given as its last argument (the sign step of release.yml is switched on in CI later);
# unset, the installer is unsigned.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bundled="$root/target/bundled"
script="$root/installer/hojt-dsp.iss"
dist="$root/dist"

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || { echo "no version in Cargo.toml" >&2; exit 1; }
numeric="${version%%-*}"
[[ "$numeric" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version $version is not x.y.z" >&2; exit 1; }

# Every bundle of bundler.toml is built and in the installer, in both formats
names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")
[[ -n "$names" ]] || { echo "no bundle names in bundler.toml" >&2; exit 1; }
for name in $names; do
  [[ -f "$bundled/$name.vst3/Contents/x86_64-win/$name.vst3" ]] || { echo "missing $bundled/$name.vst3 (cargo xtask bundle first)" >&2; exit 1; }
  [[ -f "$bundled/$name.clap" ]] || { echo "missing $bundled/$name.clap, or it is not a file" >&2; exit 1; }
  for format in vst3 clap; do
    grep -qF "$name.$format" "$script" || { echo "$name.$format has no line in installer/hojt-dsp.iss" >&2; exit 1; }
  done
done

iscc=""
for candidate in "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" "/c/Program Files/Inno Setup 6/ISCC.exe"; do
  if [[ -x "$candidate" ]]; then iscc="$candidate"; break; fi
done
[[ -n "$iscc" ]] || iscc="$(command -v iscc || command -v ISCC.exe || true)"
[[ -n "$iscc" ]] || { echo "Inno Setup 6 (ISCC.exe) not found" >&2; exit 1; }

# The wizard's licence and notice pages show text with Windows line endings
stage="$dist/installer-stage"
rm -rf "$stage"
mkdir -p "$stage"
# and a UTF-8 byte order mark, or Inno reads the HØJT and Højte in them as ANSI
for f in LICENSE NOTICE.md; do
  { printf '\xEF\xBB\xBF'; sed 's/$/\r/' "$root/$f"; } > "$stage/$f"
done

# Git Bash turns arguments that look like paths (/D...) into Windows paths unless told not to
export MSYS2_ARG_CONV_EXCL='*'
winpath() { if command -v cygpath > /dev/null; then cygpath -w "$1"; else echo "$1"; fi; }

args=(
  "/DAppVersion=$version"
  "/DNumericVersion=$numeric.0"
  "/DBundleDir=$(winpath "$bundled")"
  "/DOutDir=$(winpath "$dist")"
  "/DStageDir=$(winpath "$stage")"
)
if [[ -n "${HOJT_SIGN_COMMAND:-}" ]]; then
  args+=("/DSign=1" "/Shojtsign=$HOJT_SIGN_COMMAND \$f")
else
  echo "Unsigned: HOJT_SIGN_COMMAND is not set"
fi

mkdir -p "$dist"
"$iscc" "${args[@]}" "$(winpath "$script")"
rm -rf "$stage"
[[ -f "$dist/HojtDSP-Setup.exe" ]] || { echo "ISCC made no dist/HojtDSP-Setup.exe" >&2; exit 1; }
echo "built $dist/HojtDSP-Setup.exe ($version)"
