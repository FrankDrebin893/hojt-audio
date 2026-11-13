#!/usr/bin/env bash
# Packs the bundles in target/bundled into one zip per plugin: dist/<BundleName>-<system>.zip.
# The names carry no version: the page links to releases/latest/download/<name>.
#
#   scripts/package.sh windows|macos|linux
set -euo pipefail

system="${1:?usage: package.sh windows|macos|linux}"
root="$(cd "$(dirname "$0")/.." && pwd)"
bundled="${HOJT_BUNDLED:-$root/target/bundled}"
dist="${HOJT_DIST:-$root/dist}"
fonts="$root/crates/suite_common/assets/fonts"

case "$system" in
  windows)
    vst3_dir='C:\Program Files\Common Files\VST3'
    clap_dir='C:\Program Files\Common Files\CLAP'
    extra=''
    ;;
  macos)
    vst3_dir='~/Library/Audio/Plug-Ins/VST3'
    clap_dir='~/Library/Audio/Plug-Ins/CLAP'
    extra='
macOS stops plugins that were downloaded and are not from a registered
developer. After copying, open Terminal and run this once per plugin, with
the path of the file you copied:

    xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/NAME.vst3
    xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/NAME.clap

This build runs on Intel and Apple Silicon. It has not been tried in a DAW
yet: please report what you find.
'
    ;;
  linux)
    vst3_dir='~/.vst3'
    clap_dir='~/.clap'
    extra='
This build has not been tried in a DAW yet: please report what you find.
'
    ;;
  *)
    echo "unknown system: $system" >&2
    exit 1
    ;;
esac

rm -rf "$dist"
mkdir -p "$dist"

# Bundle names, from bundler.toml
names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")

for name in $names; do
  stage="$dist/stage/$name"
  mkdir -p "$stage/licences"

  for format in vst3 clap; do
    if [ ! -e "$bundled/$name.$format" ]; then
      echo "missing $bundled/$name.$format" >&2
      exit 1
    fi
    cp -R "$bundled/$name.$format" "$stage/"
  done

  cp "$root/LICENSE" "$root/NOTICE.md" "$stage/"
  cp "$fonts"/*-OFL.txt "$stage/licences/"

  cat > "$stage/INSTALL.txt" <<EOF
$name
HOJT Audio - https://github.com/FrankDrebin893/hojt-audio

Install
-------
Copy ONE of these, whichever your DAW uses (most use VST3):

    $name.vst3   to   $vst3_dir
    $name.clap   to   $clap_dir

Create the folder if it is not there. Then let your DAW scan for plugins.
A new version goes over the old one: your projects keep their settings.
${extra//NAME/$name}
Licence
-------
Free software under the GNU General Public License, version 3 or later.
See LICENSE and NOTICE.md. The source is at the address above.
EOF

  zip_path="$dist/$name-$system.zip"
  case "$system" in
    windows) (cd "$dist/stage" && 7z a -tzip -bso0 "../$name-$system.zip" "$name") ;;
    macos) (cd "$dist/stage" && ditto -c -k --keepParent "$name" "../$name-$system.zip") ;;
    linux) (cd "$dist/stage" && zip -qry "../$name-$system.zip" "$name") ;;
  esac
  echo "packed $zip_path"
done

# Linux also gets one zip of the whole suite with an install script (installer/linux/install.sh)
if [ "$system" = linux ]; then
  suite="$dist/stage/HojtDSP-linux"
  mkdir -p "$suite/vst3" "$suite/clap" "$suite/licences"
  for name in $names; do
    cp -R "$bundled/$name.vst3" "$suite/vst3/"
    cp -R "$bundled/$name.clap" "$suite/clap/"
  done
  cp "$root/LICENSE" "$root/NOTICE.md" "$suite/"
  cp "$fonts"/*-OFL.txt "$suite/licences/"
  cp "$root/installer/linux/install.sh" "$suite/install.sh"
  chmod +x "$suite/install.sh"
  cat > "$suite/INSTALL.txt" <<EOF
HOJT DSP - https://github.com/FrankDrebin893/hojt-audio

Install
-------
Unpack the zip, open a terminal in the folder and run:

    ./install.sh

It copies the .vst3 bundles to ~/.vst3 and the .clap files to ~/.clap, creating the
folders, and puts a new build over an old one. Then let your DAW scan for plugins.

    ./install.sh --only amp      one plugin (amp, drums, pitch_shifter, bass, pad, bells)
    ./install.sh --ask           ask before replacing a file that is already there
    ./install.sh --uninstall     remove what it installed
    ./install.sh --help

Or copy by hand, as in the zip of one plugin: a .vst3 to ~/.vst3 or a .clap to ~/.clap.

This build has not been tried in a DAW yet: please report what you find.

Licence
-------
Free software under the GNU General Public License, version 3 or later.
See LICENSE and NOTICE.md. The source is at the address above.
EOF
  (cd "$dist/stage" && zip -qry "../HojtDSP-linux.zip" "HojtDSP-linux")
  echo "packed $dist/HojtDSP-linux.zip"
fi

rm -rf "$dist/stage"
