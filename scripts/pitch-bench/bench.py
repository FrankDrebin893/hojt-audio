#!/usr/bin/env python3
"""Scores pitch-shifted renders of a dry guitar take against the true shifted signal.

  bench.py <dry.wav> <semitones> <name>=<render.wav> ...        the pedal alone
  bench.py --rig <rig_of_slow.wav> <semitones> <name>=<render.wav> ...   through a rig

The truth is the dry take played slower or faster (every pitch exactly right, only the time
scaled), read at the matching moments. Through a rig the truth is the rig's output for that
slowed take, made by run.sh. Needs numpy, scipy and soundfile.

  tone   dB  how right the partials are: magnitude SNR, 8192-sample frames (186 ms at
             44.1 kHz), 60 Hz to 6 kHz
  fine   dB  the same in 12 ms frames: envelopes only (such a frame cannot tell a pitch, so
             it does not say whether the shift is right)
  junk   dB  energy where the truth has none between its partials (0 = as clean as the truth),
             at the level that fits the truth best
  shape  0-1 how much of the truth's waveform is left in each 30 ms piece, whatever its slip.
             What a drive or an amp after the pedal makes of the signal follows this
  attack %   height of the note attacks against the truth's
  level  dB  loudness against the truth
  lag    ms  delay of the render
Higher is better for tone and shape; nearer 0 for junk and level; 100 for attack.
"""
import sys
import numpy as np
import soundfile as sf
from scipy import signal


def load(path):
    x, rate = sf.read(path)
    if x.ndim > 1:
        x = x[:, 0]
    return x.astype(np.float64), rate


def frames(x, rate, size, times):
    """Magnitude spectra of Hann frames centred on `times` (seconds)"""
    window = signal.get_window("hann", size)
    padded = np.pad(x, (size, size))
    out = np.empty((size // 2 + 1, len(times)))
    for i, t in enumerate(times):
        centre = int(round(t * rate)) + size
        out[:, i] = np.abs(np.fft.rfft(padded[centre - size // 2 : centre + size // 2] * window))
    return out


def envelope(x, rate, ms=5.0):
    b, a = signal.butter(2, 1000.0 / ms / 2 / (rate / 2))
    return np.sqrt(np.maximum(signal.filtfilt(b, a, x * x), 0.0))


def snr_db(truth, got):
    return 10 * np.log10(np.sum(truth**2) / (np.sum((truth - got) ** 2) + 1e-20))


def score(slow, out, rate, ratio):
    """`slow` is the truth with its time scaled: slow(t) is the truth at ratio * t"""
    seconds = len(slow) * ratio / rate
    # tone, and the lag that fits best
    size = 8192
    freqs = np.fft.rfftfreq(size, 1 / rate)
    band = (freqs > 60) & (freqs < 6000)
    times = np.arange(0.5, seconds - 0.5, 0.02)
    truth = frames(slow, rate, size, times / ratio)[band]
    best = (-99.0, 0.0, None, 1.0)
    for lag in np.arange(0.0, 70.0, 2.0):
        got = frames(out, rate, size, times + lag / 1000)[band]
        gain = np.sum(truth * got) / (np.sum(got * got) + 1e-20)
        value = snr_db(truth, gain * got)
        if value > best[0]:
            best = (value, lag, got, gain)
    tone, lag, got, gain = best
    level = 10 * np.log10(np.sum(got**2) / np.sum(truth**2))
    got = gain * got

    # junk: bins 30 dB under the truth's partials nearby, in frames that sound
    near = signal.convolve2d(truth**2, np.ones((9, 1)), mode="same")
    wide = np.pad(near, ((12, 12), (0, 0)))
    around = np.maximum.reduce([wide[k : k + near.shape[0]] for k in range(25)])
    quiet = (truth**2 < 1e-3 * around) & (truth.sum(0) > 0.1 * np.median(truth.sum(0)))[None, :]
    junk = 10 * np.log10((np.sum((got**2)[quiet]) + 1e-12) / (np.sum((truth**2)[quiet]) + 1e-12))

    # fine: short frames
    size = 512
    freqs = np.fft.rfftfreq(size, 1 / rate)
    band = (freqs > 60) & (freqs < 6000)
    times = np.arange(0.5, seconds - 0.5, 0.004)
    truth = frames(slow, rate, size, times / ratio)[band]
    fine = -99.0
    for near_lag in np.arange(max(0.0, lag - 12), lag + 13, 2.0):
        got = frames(out, rate, size, times + near_lag / 1000)[band]
        gain = np.sum(truth * got) / (np.sum(got * got) + 1e-20)
        fine = max(fine, snr_db(truth, gain * got))

    # shape: each 30 ms piece of the truth against the render, with up to 12 ms of slip
    piece, slip, shift = int(0.03 * rate), int(0.012 * rate), int(lag * rate / 1000)
    kept = weight = 0.0
    for t in np.arange(0.5, seconds - 0.5, 0.05):
        a = slow[int(t / ratio * rate) :][:piece]
        at = int(t * rate) + shift
        if at - slip < 0 or at + piece + slip > len(out) or len(a) < piece:
            continue
        energy = np.dot(a, a)
        if energy < 1e-7 * piece:
            continue
        b = out[at - slip : at + piece + slip]
        cross = signal.correlate(b, a, mode="valid")
        under = np.convolve(b * b, np.ones(piece), mode="valid")
        kept += np.max(cross / np.sqrt(energy * under + 1e-20)) * energy
        weight += energy
    shape = kept / weight if weight else float("nan")

    # attack: the render's envelope around each onset of the truth
    env_truth, env_out = envelope(slow, rate), envelope(out, rate)
    top = env_truth.max()
    peaks, _ = signal.find_peaks(
        env_truth, height=0.15 * top, distance=int(0.12 * rate), prominence=0.08 * top
    )
    heights = []
    for p in peaks:
        at = int(p * ratio) + shift
        if at - int(0.015 * rate) > 0 and at + int(0.03 * rate) < len(env_out):
            heights.append(env_out[at - int(0.015 * rate) : at + int(0.03 * rate)].max() / env_truth[p])
    attack = 100 * np.median(heights) if heights else float("nan")
    return dict(tone=tone, fine=fine, junk=junk, shape=shape, attack=attack, level=level, lag=lag)


def main():
    args = sys.argv[1:]
    rig = args and args[0] == "--rig"
    if rig:
        args = args[1:]
    if len(args) < 3:
        sys.exit(__doc__)
    source, rate = load(args[0])
    ratio = 2 ** (float(args[1]) / 12)
    if rig:
        slow = source
    else:
        slow = signal.resample_poly(source, 100000, int(round(100000 * ratio)))
    print(f"{'':22s} {'tone':>6s} {'fine':>6s} {'junk':>6s} {'shape':>6s} {'attack':>6s} {'level':>6s} {'lag':>5s}")
    for arg in args[2:]:
        name, _, path = arg.rpartition("=")
        out, _ = load(path)
        s = score(slow, out, rate, ratio)
        print(
            f"{name or path:22s} {s['tone']:6.1f} {s['fine']:6.1f} {s['junk']:6.1f} {s['shape']:6.2f}"
            f" {s['attack']:6.0f} {s['level']:6.1f} {s['lag']:5.0f}"
        )


if __name__ == "__main__":
    main()
