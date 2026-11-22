#!/usr/bin/env python3
"""The truth with the guitar's tone kept: a take played slower or faster, equalised so its
long-term spectrum is the dry take's again.

  keep_tone.py <dry.wav> <slowed.wav> <out.wav>

Playing a take slower moves every partial, and with them the guitar's own tone (its pickup's
treble peak, its body): what was at 4 kHz is at 2.8 kHz six semitones down. The same notes
played on the guitar keep that tone where it is in Hz. This puts it back with one static,
zero-phase EQ: the ratio of the two takes' long-term spectra, smoothed over a third of an
octave. What it leaves is a take whose notes are where the shifter should put them and whose
tone is the guitar's, which is what the same notes played there sound like.
Needs numpy, scipy and soundfile.
"""
import sys
import numpy as np
import soundfile as sf
from scipy import signal


def long_term(x, rate, size=8192):
    freqs, power = signal.welch(x, rate, nperseg=size)
    return freqs, power


def smoothed(freqs, power, share=2 ** (1 / 6)):
    out = np.empty_like(power)
    for i, f in enumerate(freqs):
        band = (freqs >= f / share) & (freqs <= f * share) if f > 0 else freqs == 0
        out[i] = power[band].mean()
    return out


def main():
    dry, rate = sf.read(sys.argv[1])
    slow, rate_slow = sf.read(sys.argv[2])
    if dry.ndim > 1:
        dry = dry[:, 0]
    if slow.ndim > 1:
        slow = slow[:, 0]
    assert rate == rate_slow
    freqs, want = long_term(dry, rate)
    _, have = long_term(slow, rate)
    floor = 1e-12 * want.max()
    gain = np.sqrt((smoothed(freqs, want) + floor) / (smoothed(freqs, have) + floor))
    # Leave the very top and bottom alone: there is next to nothing there to match
    gain[(freqs < 40) | (freqs > 0.45 * rate)] = 1.0
    gain = np.clip(gain, 0.1, 10.0)
    n = len(slow)
    size = 1 << int(np.ceil(np.log2(n + 8192)))
    spectrum = np.fft.rfft(slow, size)
    out = np.fft.irfft(spectrum * np.interp(np.fft.rfftfreq(size, 1 / rate), freqs, gain), size)[:n]
    sf.write(sys.argv[3], out, rate, subtype="FLOAT")


if __name__ == "__main__":
    main()
