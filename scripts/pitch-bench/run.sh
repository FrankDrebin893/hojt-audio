#!/usr/bin/env bash
# Renders every take of a folder through the pitch shifter of this checkout, alone and through
# a driven rig, and scores the renders against the true shifted signal (bench.py):
#
#   scripts/pitch-bench/run.sh <folder of dry mono WAVs> <output folder> [semitones] [latency ms] [name]
#
# Run it on two checkouts with the same output folder and two names to compare two engines:
# renders are kept as <take>.<name>.wav and <take>.<name>.rig.wav, and the table shows every
# name found there. One output folder per shift: a render's name does not say its semitones, so
# a second shift in the same folder would be scored against the wrong truth. A name must not
# start with "slow". A take's 24-bit WAV at 44.1 or 48 kHz is what a DAW records.
set -euo pipefail
takes=$(realpath "$1"); out=$(realpath -m "$2"); semitones=${3:--4}; latency=${4:-15}; name=${5:-here}
here=$(cd "$(dirname "$0")" && pwd); root=$(cd "$here/../.." && pwd)
mkdir -p "$out"
cd "$root"
cargo build -q -p suite_common --release --example pitch_render
cargo test -q -p amp --release --no-run
target=${CARGO_TARGET_DIR:-$root/target}
rig() { AMP_INPUT_WAV=$1 AMP_OUTPUT_WAV=$2 PITCH_SEMITONES=$3 PITCH_LATENCY_MS=$latency \
  cargo test -q -p amp --release render_pitch_rig -- --ignored >/dev/null 2>&1 ||
  { echo "the rig render of $1 failed: run cargo test -p amp --release render_pitch_rig -- --ignored" >&2; exit 1; }; }
for take in "$takes"/*.wav; do
  base=$(basename "$take" .wav)
  slow=$out/$base.slow$semitones.wav
  if [ ! -f "$slow" ] || [ ! -f "$out/$base.slow$semitones.rig.wav" ]; then
    python3 - "$take" "$slow" "$semitones" <<'PY'
import sys, soundfile as sf
from scipy import signal
x, rate = sf.read(sys.argv[1])
ratio = 2 ** (float(sys.argv[3]) / 12)
sf.write(sys.argv[2], signal.resample_poly(x, 100000, int(round(100000 * ratio))), rate, subtype="PCM_24")
PY
    rig "$slow" "$out/$base.slow$semitones.rig.wav" 0
  fi
  "$target/release/examples/pitch_render" "$take" "$out/$base.$name.wav" "$semitones" "$latency" 4
  rig "$take" "$out/$base.$name.rig.wav" "$semitones"
  echo "== $base, $semitones semitones: the pedal alone"
  renders=(); rigs=()
  for f in "$out/$base".*.wav; do
    n=${f#"$out/$base."}; n=${n%.wav}
    case $n in slow*) ;; *.rig) rigs+=("${n%.rig}=$f") ;; *) renders+=("$n=$f") ;; esac
  done
  python3 "$here/bench.py" "$take" "$semitones" "${renders[@]}"
  echo "== $base, $semitones semitones: through the rig"
  python3 "$here/bench.py" --rig "$out/$base.slow$semitones.rig.wav" "$semitones" "${rigs[@]}"
done
