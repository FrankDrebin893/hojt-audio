"""Pure-NumPy WAV I/O, latency alignment, and loudness matching utilities."""

from __future__ import annotations

import struct
from pathlib import Path
import numpy as np


def read_mono_wav(path: str | Path) -> tuple[np.ndarray, int]:
    """Read 16/24/32-bit PCM or 32-bit float WAV into a mono float32 array."""
    data = Path(path).read_bytes()
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError(f"Not a RIFF/WAVE file: {path}")

    pos = 12
    fmt_tag = None
    channels = None
    sample_rate = None
    bits_per_sample = None
    pcm_bytes = None

    while pos + 8 <= len(data):
        chunk_id = data[pos : pos + 4]
        chunk_size = struct.unpack_from("<I", data, pos + 4)[0]
        chunk_start = pos + 8
        chunk_end = min(chunk_start + chunk_size, len(data))

        if chunk_id == b"fmt " and chunk_size >= 16:
            fmt_tag, channels, sample_rate, _, _, bits_per_sample = struct.unpack_from(
                "<HHIIHH", data, chunk_start
            )
            if fmt_tag == 0xFFFE and chunk_size >= 40:
                # WAVE_FORMAT_EXTENSIBLE: subformat GUID starts at offset 24
                fmt_tag = struct.unpack_from("<H", data, chunk_start + 24)[0]
        elif chunk_id == b"data":
            pcm_bytes = data[chunk_start:chunk_end]

        pos = chunk_start + chunk_size + (chunk_size & 1)

    if fmt_tag is None or pcm_bytes is None or not channels or not sample_rate:
        raise ValueError(f"Missing fmt or data chunk in WAV: {path}")

    if fmt_tag == 3 and bits_per_sample == 32:
        samples = np.frombuffer(pcm_bytes, dtype="<f4").astype(np.float32)
    elif fmt_tag == 1 and bits_per_sample == 16:
        samples = np.frombuffer(pcm_bytes, dtype="<i2").astype(np.float32) / 32768.0
    elif fmt_tag == 1 and bits_per_sample == 32:
        samples = np.frombuffer(pcm_bytes, dtype="<i4").astype(np.float32) / 2147483648.0
    elif fmt_tag == 1 and bits_per_sample == 24:
        n_frames = len(pcm_bytes) // 3
        raw = np.frombuffer(pcm_bytes[: n_frames * 3], dtype=np.uint8).reshape(-1, 3)
        signed = (
            raw[:, 0].astype(np.int32)
            | (raw[:, 1].astype(np.int32) << 8)
            | (raw[:, 2].astype(np.int32) << 16)
        )
        signed = np.where(signed & 0x800000, signed - 0x1000000, signed)
        samples = signed.astype(np.float32) / 8388608.0
    else:
        raise ValueError(
            f"Unsupported WAV format tag={fmt_tag}, bits={bits_per_sample} in {path}"
        )

    if channels > 1:
        usable = (len(samples) // channels) * channels
        samples = samples[:usable].reshape(-1, channels)[:, 0]

    return np.ascontiguousarray(samples, dtype=np.float32), int(sample_rate)


def write_mono_wav(path: str | Path, samples: np.ndarray, sample_rate: int) -> None:
    """Write a mono 32-bit float WAV file."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    pcm = np.asarray(samples, dtype="<f4").tobytes()
    n_bytes = len(pcm)
    riff_size = 36 + n_bytes
    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        riff_size,
        b"WAVE",
        b"fmt ",
        16,
        3,  # IEEE float
        1,  # mono
        sample_rate,
        sample_rate * 4,
        4,
        32,
        b"data",
        n_bytes,
    )
    path.write_bytes(header + pcm)


def rms_db(x: np.ndarray) -> float:
    rms = float(np.sqrt(np.mean(np.square(x, dtype=np.float64)) + 1e-18))
    return 20.0 * float(np.log10(rms))


def match_rms(cand: np.ndarray, ref: np.ndarray) -> np.ndarray:
    """Scale `cand` to match the RMS energy of `ref`."""
    ref_rms = float(np.sqrt(np.mean(np.square(ref, dtype=np.float64)) + 1e-18))
    cand_rms = float(np.sqrt(np.mean(np.square(cand, dtype=np.float64)) + 1e-18))
    return (cand * (ref_rms / cand_rms)).astype(np.float32)


def match_loudness(x: np.ndarray, target_rms_db: float = -18.0, peak_limit_db: float = -0.5) -> np.ndarray:
    """Normalize audio to `target_rms_db` for fair A/B listening, capped below clipping."""
    cur_db = rms_db(x)
    gain = 10.0 ** ((target_rms_db - cur_db) / 20.0)
    y = x * gain
    peak = float(np.max(np.abs(y))) if len(y) else 0.0
    max_peak = 10.0 ** (peak_limit_db / 20.0)
    if peak > max_peak:
        y = y * (max_peak / peak)
    return y.astype(np.float32)


def align_signals(
    ref: np.ndarray, cand: np.ndarray, max_lag: int = 512
) -> tuple[np.ndarray, np.ndarray, int]:
    """Align `cand` to `ref` via FFT cross-correlation within +/- `max_lag` samples."""
    n = min(len(ref), len(cand))
    if n < 64:
        return ref[:n], cand[:n], 0

    r = ref[:n].astype(np.float64)
    c = cand[:n].astype(np.float64)
    fft_len = 1 << int(np.ceil(np.log2(2 * n)))
    corr = np.fft.irfft(np.fft.rfft(r, fft_len) * np.conj(np.fft.rfft(c, fft_len)), fft_len)

    pos_lags = corr[: max_lag + 1]
    neg_lags = corr[fft_len - max_lag :]
    best_pos_i = int(np.argmax(pos_lags))
    best_neg_i = int(np.argmax(neg_lags))

    if pos_lags[best_pos_i] >= neg_lags[best_neg_i]:
        lag = best_pos_i
    else:
        lag = best_neg_i - max_lag

    if lag > 0:
        r_al = ref[lag:n]
        c_al = cand[: n - lag]
    elif lag < 0:
        shift = -lag
        r_al = ref[: n - shift]
        c_al = cand[shift:n]
    else:
        r_al = ref[:n]
        c_al = cand[:n]

    m = min(len(r_al), len(c_al))
    return r_al[:m], c_al[:m], lag
