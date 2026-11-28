"""Perceptual audio loss functions for guitar amp DSP optimization."""

from __future__ import annotations

import numpy as np
from audio import align_signals, match_rms

BANDS_HZ: list[tuple[str, float, float]] = [
    ("Sub (<100Hz)", 20.0, 100.0),
    ("Low (100-200Hz)", 100.0, 200.0),
    ("Mud (200-450Hz)", 200.0, 450.0),
    ("Mid (450-1.2kHz)", 450.0, 1200.0),
    ("Bite (1.2-4kHz)", 1200.0, 4000.0),
    ("Pres (4-6.5kHz)", 4000.0, 6500.0),
    ("Fizz (6.5-16kHz)", 6500.0, 16000.0),
]

BAND_WEIGHTS: dict[str, float] = {
    "Sub (<100Hz)": 1.0,
    "Low (100-200Hz)": 1.2,
    "Mud (200-450Hz)": 1.5,
    "Mid (450-1.2kHz)": 1.8,
    "Bite (1.2-4kHz)": 1.5,
    "Pres (4-6.5kHz)": 1.2,
    "Fizz (6.5-16kHz)": 1.3,
}


def _stft_mag(x: np.ndarray, n_fft: int, hop: int) -> np.ndarray:
    if len(x) < n_fft:
        x = np.pad(x, (0, n_fft - len(x)))
    n_frames = 1 + (len(x) - n_fft) // hop
    strides = (x.strides[0] * hop, x.strides[0])
    frames = np.lib.stride_tricks.as_strided(x, shape=(n_frames, n_fft), strides=strides)
    win = np.hanning(n_fft).astype(np.float32)
    spec = np.fft.rfft(frames * win, axis=1)
    return np.abs(spec)


def mr_stft_loss(ref: np.ndarray, cand: np.ndarray, fft_sizes: tuple[int, ...] = (256, 512, 1024, 2048)) -> float:
    """Multi-resolution STFT loss (spectral convergence + log-magnitude L1)."""
    eps = 1e-5
    total = 0.0
    for n_fft in fft_sizes:
        hop = n_fft // 4
        r_mag = _stft_mag(ref, n_fft, hop)
        c_mag = _stft_mag(cand, n_fft, hop)

        r_norm = float(np.linalg.norm(r_mag)) + eps
        sc = float(np.linalg.norm(r_mag - c_mag)) / r_norm
        log_l1 = float(np.mean(np.abs(np.log(r_mag + eps) - np.log(c_mag + eps))))
        total += sc + 0.5 * log_l1
    return total / len(fft_sizes)


def band_deltas_db(ref: np.ndarray, cand: np.ndarray, sr: int) -> dict[str, float]:
    """Per-band energy difference in dB (`cand - ref`)."""
    n_fft = 4096
    hop = 1024
    r_pow = np.mean(_stft_mag(ref, n_fft, hop) ** 2, axis=0) + 1e-15
    c_pow = np.mean(_stft_mag(cand, n_fft, hop) ** 2, axis=0) + 1e-15
    freqs = np.fft.rfftfreq(n_fft, d=1.0 / sr)

    deltas: dict[str, float] = {}
    for name, lo, hi in BANDS_HZ:
        mask = (freqs >= lo) & (freqs < min(hi, 0.49 * sr))
        if not np.any(mask):
            deltas[name] = 0.0
            continue
        r_e = float(np.sum(r_pow[mask]))
        c_e = float(np.sum(c_pow[mask]))
        deltas[name] = 10.0 * float(np.log10(c_e / r_e))
    return deltas


def envelope_loss(ref: np.ndarray, cand: np.ndarray, sr: int, frame_ms: float = 10.0) -> tuple[float, float]:
    """10 ms RMS envelope L1 error and crest-factor gap (dB)."""
    frame_len = max(16, int(round(sr * frame_ms * 0.001)))
    n_frames = len(ref) // frame_len
    if n_frames == 0:
        return 0.0, 0.0

    r_env = np.sqrt(np.mean(ref[: n_frames * frame_len].reshape(n_frames, frame_len) ** 2, axis=1) + 1e-12)
    c_env = np.sqrt(np.mean(cand[: n_frames * frame_len].reshape(n_frames, frame_len) ** 2, axis=1) + 1e-12)

    r_db = 20.0 * np.log10(r_env + 1e-6)
    c_db = 20.0 * np.log10(c_env + 1e-6)
    env_l1 = float(np.mean(np.abs(r_db - c_db)))

    r_crest = 20.0 * float(np.log10((np.max(np.abs(ref)) + 1e-9) / (np.sqrt(np.mean(ref ** 2)) + 1e-9)))
    c_crest = 20.0 * float(np.log10((np.max(np.abs(cand)) + 1e-9) / (np.sqrt(np.mean(cand ** 2)) + 1e-9)))
    return env_l1, c_crest - r_crest


def evaluate_pair(ref: np.ndarray, cand: np.ndarray, sr: int) -> dict:
    """Align, loudness-match, and score `cand` against `ref`."""
    r_al, c_al, lag = align_signals(ref, cand)
    c_matched = match_rms(c_al, r_al)

    stft = mr_stft_loss(r_al, c_matched)
    bands = band_deltas_db(r_al, c_matched, sr)
    band_tilt = float(
        np.mean([BAND_WEIGHTS[name] * abs(delta) for name, delta in bands.items()])
    )
    env_db, crest_gap = envelope_loss(r_al, c_matched, sr)

    total = stft + 0.15 * band_tilt + 0.15 * env_db + 0.05 * abs(crest_gap)
    return {
        "total": float(total),
        "mr_stft": float(stft),
        "band_tilt": float(band_tilt),
        "envelope_db": float(env_db),
        "crest_gap_db": float(crest_gap),
        "bands_db": bands,
        "lag_samples": int(lag),
    }
