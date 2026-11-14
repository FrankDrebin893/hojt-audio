#!/usr/bin/env bash
# Runs the reports that describe how the plugins sound, without the lines that time them,
# into <out>/<plugin>_<report>.txt. Run from the root of a checkout. With --renders, the
# render tests of the plugins named after it write their WAV files to target/renders too.
# The reports and renders run side by side, as many as the machine has cores (SOUND_JOBS=<n>
# for another number): each is one test on one core, and one after another they took 12 minutes.
# Their timings are left out of what is kept, so a busy machine changes nothing.
# Usage: scripts/sound-reports.sh <out> [--renders <plugin>...]
set -uo pipefail

out=$1
shift
mkdir -p "$out"

# The Pitch Shifter's algorithm and its reports live in suite_common (the amp's pitch pedal
# plays it too); every other report is in the package of its plugin
package_of() {
    case "$1" in
        pitch_shifter) echo suite_common ;;
        *) echo "$1" ;;
    esac
}

reports='
amp fingerprint_report
amp loudness_report
amp mic_report
amp preset_report
amp preset_gate_report
amp effects_report
amp comp_report
amp mods_report
amp pitch_report
drums kit_report
bass bass_report
pad pad_report
bells bells_report
pitch_shifter quality_report
'

jobs="${SOUND_JOBS:-$( (nproc || sysctl -n hw.ncpu) 2> /dev/null || echo 2)}"

# Waits until fewer than $jobs of this script's background jobs run
wait_for_room() {
    while (($(jobs -pr | wc -l) >= jobs)); do
        wait -n 2> /dev/null || true
    done
}

one_report() {
    local plugin=$1 report=$2
    local file="$out/${plugin}_${report}.txt"
    # Only what the report prints (stdout): cargo's build output and warnings go to stderr and
    # carry source line numbers, which move with any edit
    if cargo test -q -p "$(package_of "$plugin")" --release --locked "$report" -- --ignored --nocapture > "$file.raw" 2>/dev/null; then
        # Timings differ from run to run and machine to machine: leave them out (the cost tables
        # that end effects_report, pitch_report and quality_report, the ns columns of comp_report),
        # and the test harness's "has been running for over 60 seconds", which lands in the
        # middle of whatever line is being printed
        perl -0pe 's/test \S+ has been running for over \d+ seconds\n//g' "$file.raw" \
            | grep -vE 'of a core|ns per|us per|µs|finished in|^running [0-9]|^$| [0-9.]+ ns( |$)' \
            | sed '/^Cost[ :]/,$d' > "$file"
    else
        echo "(this report failed or does not exist here)" > "$file"
    fi
    rm -f "$file.raw"
}

# Build once, so that the runs below find everything built and do not queue for cargo's lock.
# A build that fails here fails again in each report, which then says so in its file
packages=$(while read -r plugin _; do [ -n "$plugin" ] && echo "-p $(package_of "$plugin")"; done <<< "$reports" | sort -u)
# shellcheck disable=SC2086
cargo test -q $packages --release --locked --no-run > /dev/null 2>&1 || true

while read -r plugin report; do
    [ -z "$plugin" ] && continue
    wait_for_room
    one_report "$plugin" "$report" &
done <<< "$reports"

if [ "${1:-}" = "--renders" ]; then
    shift
    for plugin in "$@"; do
        wait_for_room
        cargo test -q -p "$(package_of "$plugin")" --release --locked render_wavs -- --ignored > /dev/null 2>&1 &
    done
fi
wait
