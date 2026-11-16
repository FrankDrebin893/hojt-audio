#!/usr/bin/env bash
# Prints the distribution.xml of HojtDSP-macos.pkg on stdout: a choices page with one choice per
# plugin (from bundler.toml), each with a VST3 and a CLAP choice below it, all ticked. The component
# packages it points at are <BundleName>-vst3.pkg and <BundleName>-clap.pkg (build-pkg.sh).
# Runs anywhere (no macOS tools).
#
#   installer/macos/make-distribution.sh <version>
set -euo pipefail

version="${1:?usage: make-distribution.sh <version>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")
id_prefix=net.hojte.hojtaudio

# HojtGuitarAmp -> Guitar Amp
title() { echo "${1#Hojt}" | sed 's/\([a-z]\)\([A-Z]\)/\1 \2/g'; }

echo '<?xml version="1.0" encoding="utf-8"?>'
echo '<installer-gui-script minSpecVersion="2">'
echo '    <title>HØJT DSP</title>'
echo '    <welcome file="welcome.html" mime-type="text/html"/>'
echo '    <license file="license.txt" mime-type="text/plain"/>'
echo '    <options customize="always" require-scripts="false" hostArchitectures="x86_64,arm64"/>'
echo '    <domains enable_anywhere="false" enable_currentUserHome="false" enable_localSystem="true"/>'
echo '    <choices-outline>'
for name in $names; do
    echo "        <line choice=\"$name\">"
    echo "            <line choice=\"$name.vst3\"/>"
    echo "            <line choice=\"$name.clap\"/>"
    echo '        </line>'
done
echo '    </choices-outline>'
for name in $names; do
    echo "    <choice id=\"$name\" title=\"$(title "$name")\" description=\"$(title "$name"), in the formats ticked below.\" start_selected=\"true\"/>"
    echo "    <choice id=\"$name.vst3\" title=\"VST3\" description=\"For most DAWs: Reaper, Ableton Live, Cubase, FL Studio, Studio One. Goes to /Library/Audio/Plug-Ins/VST3.\" start_selected=\"true\">"
    echo "        <pkg-ref id=\"$id_prefix.$name.vst3\"/>"
    echo '    </choice>'
    echo "    <choice id=\"$name.clap\" title=\"CLAP\" description=\"For DAWs that support CLAP: Reaper, Bitwig, FL Studio, Studio One. Goes to /Library/Audio/Plug-Ins/CLAP.\" start_selected=\"true\">"
    echo "        <pkg-ref id=\"$id_prefix.$name.clap\"/>"
    echo '    </choice>'
done
for name in $names; do
    for format in vst3 clap; do
        echo "    <pkg-ref id=\"$id_prefix.$name.$format\" version=\"$version\" onConclusion=\"none\">$name-$format.pkg</pkg-ref>"
    done
done
echo '</installer-gui-script>'
