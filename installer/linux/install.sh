#!/usr/bin/env bash
# Installs HOJT DSP for the current user: the .vst3 bundles to ~/.vst3 and the .clap files to
# ~/.clap, creating the folders. Run it from the unpacked HojtDSP-linux folder.
#
#   ./install.sh                 install every plugin, VST3 and CLAP, over an older build
#   ./install.sh --only amp      only one plugin: amp, drums, pitch_shifter, bass, pad or bells
#                                (or its file name, such as HojtGuitarAmp); repeat it for several
#   ./install.sh --ask           ask before replacing a file that is already there
#   ./install.sh --uninstall     remove what this installed (with --only, only that plugin)
#   ./install.sh --help
#
# HOME decides where it installs, so a test can point it at a temporary folder.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
vst3_dir="${HOME:?HOME is not set}/.vst3"
clap_dir="$HOME/.clap"

# The six plugins: short name and the name of the file. A new build replaces the old one in
# place, so a file name here never changes (see CLAUDE.md, Versioning)
declare -A bundle=(
    [pitch_shifter]=HojtPitchShifter
    [drums]=HojtDrumSynth
    [amp]=HojtGuitarAmp
    [bass]=HojtBassSynth
    [bells]=HojtGlassBells
    [pad]=HojtShimmerPad
)
all=(pitch_shifter drums amp bass bells pad)

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
}

ask=false
uninstall=false
chosen=()

while (($#)); do
    case "$1" in
    --ask) ask=true ;;
    --uninstall) uninstall=true ;;
    --only)
        [[ $# -ge 2 ]] || {
            echo "--only needs a plugin name" >&2
            exit 64
        }
        shift
        chosen+=("$1")
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "unknown option: $1 (try --help)" >&2
        exit 64
        ;;
    esac
    shift
done

# The file name of a plugin, from what was given to --only
resolve() {
    local want="${1,,}" key
    for key in "${all[@]}"; do
        if [[ "$want" == "$key" || "$want" == "${bundle[$key],,}" ]]; then
            echo "${bundle[$key]}"
            return 0
        fi
    done
    echo "unknown plugin: $1 (one of: ${all[*]})" >&2
    return 1
}

names=()
if ((${#chosen[@]})); then
    for c in "${chosen[@]}"; do names+=("$(resolve "$c")"); done
else
    for key in "${all[@]}"; do names+=("${bundle[$key]}"); done
fi

if $uninstall; then
    for name in "${names[@]}"; do
        for target in "$vst3_dir/$name.vst3" "$clap_dir/$name.clap"; do
            if [[ -e "$target" ]]; then
                rm -rf "$target"
                echo "removed $target"
            fi
        done
    done
    exit 0
fi

mkdir -p "$vst3_dir" "$clap_dir"

# Copies a file or a bundle folder to a folder, replacing what is there (after a yes with --ask)
place() {
    local from="$1" dir="$2" to
    to="$dir/$(basename "$from")"
    [[ -e "$from" ]] || {
        echo "missing $from: run this from the unpacked HojtDSP-linux folder" >&2
        exit 1
    }
    if $ask && [[ -e "$to" ]]; then
        local answer
        read -r -p "$to is already there. Replace it? [y/N] " answer || answer=n
        if [[ "$answer" != [yY]* ]]; then
            echo "kept $to"
            return 0
        fi
    fi
    rm -rf "$to"
    cp -R "$from" "$dir/"
    echo "installed $to"
}

for name in "${names[@]}"; do
    place "$here/vst3/$name.vst3" "$vst3_dir"
    place "$here/clap/$name.clap" "$clap_dir"
done

echo "Done. Let your DAW scan for plugins: they are listed under Hojt Audio."
