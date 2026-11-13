#!/usr/bin/env bash
# Tests the Linux suite zip: packs stand-in bundles with scripts/package.sh, unpacks
# HojtDSP-linux.zip and runs its install.sh into a temporary HOME: every file lands, --only picks
# one, a second run replaces in place, --ask can keep a file, --uninstall leaves nothing, and
# a wrong name fails. Runs shellcheck on the scripts when it is installed. No build needed.
#
#   scripts/test-linux-install.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/installer/linux/install.sh" "$root/scripts/test-linux-install.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")
[ "$(echo "$names" | wc -l)" -eq 6 ] || fail "bundler.toml does not name six plugins"

# Stand-in bundles: a VST3 is a folder, a CLAP a file
bundled="$tmp/bundled"
for name in $names; do
    mkdir -p "$bundled/$name.vst3/Contents/x86_64-linux"
    echo "so of $name" > "$bundled/$name.vst3/Contents/x86_64-linux/$name.so"
    echo "clap of $name" > "$bundled/$name.clap"
done

HOJT_BUNDLED="$bundled" HOJT_DIST="$tmp/dist" bash "$root/scripts/package.sh" linux > /dev/null
[ -f "$tmp/dist/HojtDSP-linux.zip" ] || fail "no HojtDSP-linux.zip"
for name in $names; do
    [ -f "$tmp/dist/$name-linux.zip" ] || fail "the zip of $name is gone"
done

mkdir "$tmp/unpacked"
unzip -q "$tmp/dist/HojtDSP-linux.zip" -d "$tmp/unpacked"
suite="$tmp/unpacked/HojtDSP-linux"
for f in install.sh INSTALL.txt LICENSE NOTICE.md licences; do
    [ -e "$suite/$f" ] || fail "the zip has no $f"
done
[ -x "$suite/install.sh" ] || fail "install.sh is not executable in the zip"

home="$tmp/home"
mkdir "$home"
run() {
    HOME="$home" "$suite/install.sh" "$@" > "$tmp/out" 2>&1 || {
        cat "$tmp/out" >&2
        fail "install.sh $* failed"
    }
}
empty() { [ -z "$(find "$home/.vst3" "$home/.clap" -mindepth 1)" ]; }

# Everything, into folders that do not exist yet
run
for name in $names; do
    [ -f "$home/.vst3/$name.vst3/Contents/x86_64-linux/$name.so" ] || fail "$name.vst3 not installed"
    [ -f "$home/.clap/$name.clap" ] || fail "$name.clap not installed"
done

# Again, over the old one: replaced in place, nothing of the old bundle left
echo old > "$home/.clap/HojtGuitarAmp.clap"
echo stray > "$home/.vst3/HojtGuitarAmp.vst3/stray"
run
grep -q "clap of HojtGuitarAmp" "$home/.clap/HojtGuitarAmp.clap" || fail "a second run did not replace the file"
[ ! -e "$home/.vst3/HojtGuitarAmp.vst3/stray" ] || fail "a second run left a file of the old bundle"

# --ask with a no keeps the file, with a yes replaces it
echo mine > "$home/.clap/HojtGuitarAmp.clap"
printf 'n\nn\n' | HOME="$home" "$suite/install.sh" --only amp --ask > /dev/null
[ "$(cat "$home/.clap/HojtGuitarAmp.clap")" = mine ] || fail "--ask with no replaced the file"
printf 'y\ny\n' | HOME="$home" "$suite/install.sh" --only amp --ask > /dev/null
grep -q "clap of HojtGuitarAmp" "$home/.clap/HojtGuitarAmp.clap" || fail "--ask with yes did not replace"

# Uninstall, then one plugin only (by short name and by file name), then uninstall only that one
run --uninstall
empty || fail "--uninstall left files"
run --only bells --only HojtDrumSynth
[ -f "$home/.clap/HojtGlassBells.clap" ] || fail "--only did not install bells"
[ -d "$home/.vst3/HojtDrumSynth.vst3" ] || fail "--only did not install drums"
[ "$(find "$home/.clap" -type f | wc -l)" -eq 2 ] || fail "--only installed more than it named"
run --uninstall --only bells
[ ! -e "$home/.clap/HojtGlassBells.clap" ] || fail "--uninstall --only kept the named plugin"
[ -e "$home/.clap/HojtDrumSynth.clap" ] || fail "--uninstall --only removed another plugin"

# Everything out again, and nothing of ours left
run
run --uninstall
empty || fail "--uninstall left files"

# A name that is not a plugin fails and installs nothing
if HOME="$home" "$suite/install.sh" --only nope > /dev/null 2>&1; then fail "--only nope succeeded"; fi
empty || fail "--only nope installed something"

echo "Linux install test passed"
