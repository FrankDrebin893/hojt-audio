"""Rust-in-the-loop perceptual optimizer and diagnostic tool for amp DSP."""

from __future__ import annotations

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import numpy as np

OPT_DIR = Path(__file__).resolve().parent
if str(OPT_DIR) not in sys.path:
    sys.path.insert(0, str(OPT_DIR))

from audio import align_signals, match_loudness, read_mono_wav, write_mono_wav
from cmaes import CMAES
from loss import evaluate_pair

REPO_ROOT = OPT_DIR.parent.parent
EXE_NAME = "amp_bench.exe" if os.name == "nt" else "amp_bench"


def find_or_build_bench() -> Path:
    for profile in ("optimised", "release"):
        cand = REPO_ROOT / "target" / profile / EXE_NAME
        if cand.exists():
            return cand
    print("Building amp_bench (--profile optimised)...")
    subprocess.run(
        ["cargo", "build", "-p", "amp", "--bin", "amp_bench", "--profile", "optimised"],
        cwd=REPO_ROOT,
        check=True,
    )
    cand = REPO_ROOT / "target" / "optimised" / EXE_NAME
    if not cand.exists():
        raise FileNotFoundError(f"Expected binary not found after build: {cand}")
    return cand


def run_amp_bench(
    exe: Path,
    amp_name: str,
    di_path: Path,
    out_path: Path,
    *,
    ir_path: Path | None = None,
    cab_off: bool = False,
    overrides_path: Path | None = None,
    in_gain_db: float = 0.0,
    gain: float = 0.5,
    bass: float = 0.5,
    mid: float = 0.5,
    treble: float = 0.5,
    presence: float = 0.5,
    master: float = 0.5,
) -> None:
    cmd = [
        str(exe),
        "--amp",
        amp_name,
        "--di",
        str(di_path),
        "--out",
        str(out_path),
        "--in-gain-db",
        f"{in_gain_db:.4f}",
        "--gain",
        f"{gain:.4f}",
        "--bass",
        f"{bass:.4f}",
        "--mid",
        f"{mid:.4f}",
        "--treble",
        f"{treble:.4f}",
        "--presence",
        f"{presence:.4f}",
        "--master",
        f"{master:.4f}",
    ]
    if cab_off:
        cmd.append("--cab-off")
    elif ir_path is not None:
        cmd.extend(["--ir", str(ir_path)])
    if overrides_path is not None:
        cmd.extend(["--overrides", str(overrides_path)])
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)


def dump_model(exe: Path, amp_name: str) -> dict[str, str]:
    res = subprocess.run(
        [str(exe), "--amp", amp_name, "--dump-model"],
        check=True,
        capture_output=True,
        text=True,
    )
    parsed: dict[str, str] = {}
    for line in res.stdout.splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            parsed[k.strip()] = v.strip()
    return parsed


def default_param_spec(
    model_kv: dict[str, str], mode: str = "head", voice: str = "default"
) -> list[dict]:
    """Build a practical default set of continuous parameters from a dumped AmpModel."""
    stage_count = int(model_kv.get("stage_count", "2"))
    specs: list[dict] = []

    def add(key: str, idx: int | None, val: float, lo: float, hi: float) -> None:
        lo_c = min(lo, val)
        hi_c = max(hi, val)
        specs.append({"key": key, "index": idx, "init": val, "bounds": [lo_c, hi_c]})

    if mode in ("head", "all"):
        tight_bounds = (
            (100.0, 220.0) if voice == "crunch" else (40.0, 140.0) if voice == "clean" else (25.0, 260.0)
        )
        add("tight_hz", None, float(model_kv["tight_hz"]), *tight_bounds)
        focus = [float(x) for x in model_kv["focus"].split(",")]
        add("focus", 0, focus[0], 300.0, 2200.0)
        add("focus", 1, focus[1], 0.4, 2.0)
        add("focus", 2, focus[2], -6.0, 12.0)
        add("bright_db", None, float(model_kv["bright_db"]), 0.0, 14.0)

        mud = [float(x) for x in model_kv["mud_cut"].split(",")]
        add("mud_cut", 0, mud[0], 180.0, 500.0)
        add("mud_cut", 1, mud[1], 0.5, 2.5)
        mud_db_bounds = (-4.5, 2.0) if voice == "crunch" else (-10.0, 4.0)
        add("mud_cut", 2, mud[2], *mud_db_bounds)

        air = [float(x) for x in model_kv["air_shelf"].split(",")]
        add("air_shelf", 0, air[0], 3000.0, 9000.0)
        add("air_shelf", 1, air[1], -6.0, 14.0)

        if model_kv.get("fizz_hz", "none").lower() != "none":
            add("fizz_hz", None, float(model_kv["fizz_hz"]), 3500.0, 16000.0)
        if model_kv.get("lowcut_hz", "none").lower() != "none":
            add("lowcut_hz", None, float(model_kv["lowcut_hz"]), 40.0, 220.0)

        for i in range(stage_count):
            p = f"stage{i}"
            add(f"{p}.coupling_hz", None, float(model_kv[f"{p}.coupling_hz"]), 20.0, 250.0)
            gains = [float(x) for x in model_kv[f"{p}.gain_db"].split(",")]
            for gi, gval in enumerate(gains):
                add(f"{p}.gain_db", gi, gval, gval - 12.0, gval + 12.0)
            hr = [float(x) for x in model_kv[f"{p}.headroom"].split(",")]
            add(f"{p}.headroom", 0, hr[0], max(0.2, hr[0] * 0.4), hr[0] * 3.0)
            add(f"{p}.headroom", 1, hr[1], max(0.2, hr[1] * 0.4), hr[1] * 3.0)
            add(f"{p}.bias_shift", None, float(model_kv[f"{p}.bias_shift"]), 0.0, 0.8)
            lp = float(model_kv[f"{p}.lowpass_hz"])
            add(f"{p}.lowpass_hz", None, lp, 2500.0, 18000.0)

        add("tone.bass_hz", None, float(model_kv["tone.bass_hz"]), 60.0, 220.0)
        add("tone.mid_hz", None, float(model_kv["tone.mid_hz"]), 300.0, 1100.0)
        mid_c_bounds = (-1.0, 6.0) if voice == "crunch" else (-10.0, 6.0)
        add("tone.mid_centre_db", None, float(model_kv["tone.mid_centre_db"]), *mid_c_bounds)
        add("tone.treble_hz", None, float(model_kv["tone.treble_hz"]), 1800.0, 5500.0)
        pwr_drive = [float(x) for x in model_kv["power.drive_db"].split(",")]
        add("power.drive_db", 1, pwr_drive[1], -12.0, 8.0)
        add("power.sag", None, float(model_kv["power.sag"]), 0.02, 0.45)
        add("power.resonance_hz", None, float(model_kv["power.resonance_hz"]), 60.0, 160.0)
        res_bounds = (0.0, 3.5) if voice == "crunch" else (0.0, 8.0)
        add("power.resonance_db", None, float(model_kv["power.resonance_db"]), *res_bounds)
        add("power.presence_db", None, float(model_kv["power.presence_db"]), 0.0, 10.0)

    if mode in ("cab", "all"):
        add("cab.resonance_hz", None, float(model_kv["cab.resonance_hz"]), 55.0, 160.0)
        add("cab.resonance_q", None, float(model_kv["cab.resonance_q"]), 0.6, 2.0)
        body = [float(x) for x in model_kv["cab.body"].split(",")]
        add("cab.body", 0, body[0], 100.0, 450.0)
        add("cab.body", 1, body[1], 0.5, 1.8)
        body_db_bounds = (-2.0, 3.5) if voice == "crunch" else (-4.0, 8.0)
        add("cab.body", 2, body[2], *body_db_bounds)
        dip = [float(x) for x in model_kv["cab.dip"].split(",")]
        add("cab.dip", 0, dip[0], 300.0, 1600.0)
        add("cab.dip", 1, dip[1], 0.5, 2.0)
        dip_db_bounds = (
            (-4.0, 1.0) if voice == "crunch" else (-3.5, 0.0) if voice == "clean" else (-8.0, 2.0)
        )
        add("cab.dip", 2, dip[2], *dip_db_bounds)
        bite = [float(x) for x in model_kv["cab.bite"].split(",")]
        add("cab.bite", 0, bite[0], 1400.0, 5000.0)
        add("cab.bite", 1, bite[1], 0.5, 1.8)
        bite_db_bounds = (
            (1.0, 8.0) if voice == "crunch" else (0.0, 5.0) if voice == "clean" else (-2.0, 12.0)
        )
        add("cab.bite", 2, bite[2], *bite_db_bounds)
        rolloff_bounds = (7500.0, 14000.0) if voice == "clean" else (3200.0, 12000.0)
        add("cab.rolloff_hz", None, float(model_kv["cab.rolloff_hz"]), *rolloff_bounds)
        add("cab.air_ratio", None, float(model_kv["cab.air_ratio"]), 1.0, 2.0)
        breakup_bounds = (0.0, 0.2) if voice == "clean" else (0.0, 4.0)
        add("cab.breakup_db", None, float(model_kv["cab.breakup_db"]), *breakup_bounds)

    return specs


def vector_to_overrides_text(
    x: np.ndarray,
    specs: list[dict],
    base_kv: dict[str, str],
    base_prefix: str = "",
) -> str:
    arrays: dict[str, list[float]] = {}
    scalars: dict[str, float] = {}
    for val, spec in zip(x, specs):
        key = spec["key"]
        idx = spec.get("index")
        if idx is None:
            scalars[key] = float(val)
        else:
            if key not in arrays:
                arrays[key] = [float(v) for v in base_kv[key].split(",")]
            arrays[key][idx] = float(val)

    lines = ["# Generated by tools/amp_opt/optimize.py"]
    if base_prefix:
        lines.append(base_prefix.rstrip())
    for key, val in sorted(scalars.items()):
        lines.append(f"{key} = {val:.6f}")
    for key, vals in sorted(arrays.items()):
        lines.append(f"{key} = " + ", ".join(f"{v:.6f}" for v in vals))
    return "\n".join(lines) + "\n"


def format_metrics_table(metrics: dict) -> str:
    lines = [
        f"- **Total Loss**: `{metrics['total']:.4f}`",
        f"- **MR-STFT**: `{metrics['mr_stft']:.4f}`",
        f"- **Band Tilt**: `{metrics['band_tilt']:.4f}`",
        f"- **Envelope L1**: `{metrics['envelope_db']:.2f} dB`",
        f"- **Crest Gap (cand - ref)**: `{metrics['crest_gap_db']:+.2f} dB`",
        f"- **Alignment Lag**: `{metrics['lag_samples']} samples`",
        "",
        "| Band | Delta (cand - ref) |",
        "| :--- | ---: |",
    ]
    for band, delta in metrics["bands_db"].items():
        lines.append(f"| {band} | {delta:+.2f} dB |")
    return "\n".join(lines)


def cmd_diagnose(args: argparse.Namespace) -> None:
    exe = find_or_build_bench()
    di_path = Path(args.di)
    ref_path = Path(args.ref)
    ir_path = Path(args.ir) if args.ir else None
    ov_path = Path(args.overrides) if args.overrides else None

    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = REPO_ROOT / "bench" / "runs" / f"{ts}_diagnose_{args.amp.lower()}"
    out_dir.mkdir(parents=True, exist_ok=True)
    raw_cand = out_dir / "_raw_candidate.wav"

    run_amp_bench(
        exe,
        args.amp,
        di_path,
        raw_cand,
        ir_path=ir_path,
        cab_off=args.cab_off,
        overrides_path=ov_path,
        in_gain_db=args.in_gain_db,
        gain=args.gain,
        bass=args.bass,
        mid=args.mid,
        treble=args.treble,
        presence=args.presence,
        master=args.master,
    )

    ref_audio, ref_sr = read_mono_wav(ref_path)
    cand_audio, cand_sr = read_mono_wav(raw_cand)
    if ref_sr != cand_sr:
        raise ValueError(f"Sample rate mismatch: ref={ref_sr} Hz, cand={cand_sr} Hz")

    metrics = evaluate_pair(ref_audio, cand_audio, ref_sr)
    r_al, c_al, _ = align_signals(ref_audio, cand_audio)
    write_mono_wav(out_dir / "01_reference.wav", match_loudness(r_al), ref_sr)
    write_mono_wav(out_dir / "02_baseline.wav", match_loudness(c_al), ref_sr)
    raw_cand.unlink(missing_ok=True)

    report = f"# Diagnostic Report: {args.amp}\n\n" + format_metrics_table(metrics) + "\n"
    (out_dir / "report.md").write_text(report, encoding="utf-8")
    print(report)
    print(f"Saved A/B bundle to: {out_dir}")


def cmd_run(args: argparse.Namespace) -> None:
    exe = find_or_build_bench()
    cases = json.loads(Path(args.cases).read_text(encoding="utf-8"))
    if not cases:
        raise ValueError("cases JSON must contain at least one case")

    base_kv = dump_model(exe, args.amp)
    base_prefix = ""
    base_ov_path = Path(args.base_overrides) if args.base_overrides else None
    if base_ov_path is not None:
        base_prefix = base_ov_path.read_text(encoding="utf-8")

    if args.params:
        specs = json.loads(Path(args.params).read_text(encoding="utf-8"))
    else:
        specs = default_param_spec(base_kv, mode=args.mode, voice=args.voice)

    x0 = np.array([float(s["init"]) for s in specs], dtype=np.float64)
    bounds = [(float(s["bounds"][0]), float(s["bounds"][1])) for s in specs]

    loaded_refs: list[tuple[np.ndarray, int]] = [read_mono_wav(c["ref"]) for c in cases]

    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = REPO_ROOT / "bench" / "runs" / f"{ts}_opt_{args.amp.lower()}"
    out_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        tmp_ov = tmp_dir / "overrides.txt"
        tmp_wav = tmp_dir / "cand.wav"

        def eval_overrides(ov_file: Path | None) -> tuple[float, list[dict]]:
            per_case: list[dict] = []
            losses: list[float] = []
            for idx, (case, (ref_audio, ref_sr)) in enumerate(zip(cases, loaded_refs)):
                ir_p = Path(case["ir"]) if case.get("ir") else None
                run_amp_bench(
                    exe,
                    args.amp,
                    Path(case["di"]),
                    tmp_wav,
                    ir_path=ir_p,
                    cab_off=bool(case.get("cab_off", False)),
                    overrides_path=ov_file,
                    in_gain_db=float(case.get("in_gain_db", 0.0)),
                    gain=float(case.get("gain", 0.5)),
                    bass=float(case.get("bass", 0.5)),
                    mid=float(case.get("mid", 0.5)),
                    treble=float(case.get("treble", 0.5)),
                    presence=float(case.get("presence", 0.5)),
                    master=float(case.get("master", 0.5)),
                )
                cand_audio, cand_sr = read_mono_wav(tmp_wav)
                if cand_sr != ref_sr:
                    raise ValueError(f"Case {idx} sample rate mismatch: {cand_sr} != {ref_sr}")
                m = evaluate_pair(ref_audio, cand_audio, ref_sr)
                per_case.append(m)
                losses.append(m["total"])
            return float(np.mean(losses)), per_case

        base_loss, base_cases = eval_overrides(base_ov_path)
        print(f"Baseline mean loss ({len(cases)} case(s)): {base_loss:.4f}")

        opt = CMAES(x0=x0, bounds=bounds, sigma0=args.sigma, seed=args.seed)
        best_x = x0.copy()
        best_loss = base_loss
        best_cases = base_cases
        evals_done = 0

        while evals_done < args.evals:
            xs = opt.ask()
            fs: list[float] = []
            for x in xs:
                tmp_ov.write_text(
                    vector_to_overrides_text(x, specs, base_kv, base_prefix), encoding="utf-8"
                )
                loss, case_metrics = eval_overrides(tmp_ov)
                fs.append(loss)
                evals_done += 1
                if loss < best_loss:
                    best_loss = loss
                    best_x = x.copy()
                    best_cases = case_metrics
            opt.tell(xs, fs)
            print(
                f"gen={opt.gen:02d} evals={evals_done:4d}/{args.evals} "
                f"gen_best={min(fs):.4f} overall_best={best_loss:.4f} sigma={opt.sigma:.4f}"
            )

        best_ov_path = out_dir / "best_overrides.txt"
        best_ov_path.write_text(
            vector_to_overrides_text(best_x, specs, base_kv, base_prefix), encoding="utf-8"
        )

        report_parts = [
            f"# Optimization Report: {args.amp}",
            "",
            f"- **Evaluations**: {evals_done}",
            f"- **Baseline Mean Loss**: `{base_loss:.4f}`",
            f"- **Optimized Mean Loss**: `{best_loss:.4f}`",
            "",
        ]
        for idx, (case, (ref_audio, ref_sr), bm, am) in enumerate(
            zip(cases, loaded_refs, base_cases, best_cases)
        ):
            cname = case.get("name", f"case_{idx}")
            cdir = out_dir / cname
            cdir.mkdir(parents=True, exist_ok=True)
            ir_p = Path(case["ir"]) if case.get("ir") else None
            kwargs = dict(
                ir_path=ir_p,
                cab_off=bool(case.get("cab_off", False)),
                in_gain_db=float(case.get("in_gain_db", 0.0)),
                gain=float(case.get("gain", 0.5)),
                bass=float(case.get("bass", 0.5)),
                mid=float(case.get("mid", 0.5)),
                treble=float(case.get("treble", 0.5)),
                presence=float(case.get("presence", 0.5)),
                master=float(case.get("master", 0.5)),
            )
            before_raw = tmp_dir / f"before_{idx}.wav"
            after_raw = tmp_dir / f"after_{idx}.wav"
            run_amp_bench(exe, args.amp, Path(case["di"]), before_raw, overrides_path=base_ov_path, **kwargs)
            run_amp_bench(exe, args.amp, Path(case["di"]), after_raw, overrides_path=best_ov_path, **kwargs)

            b_audio, _ = read_mono_wav(before_raw)
            a_audio, _ = read_mono_wav(after_raw)
            r_al, b_al, _ = align_signals(ref_audio, b_audio)
            _, a_al, _ = align_signals(ref_audio, a_audio)
            write_mono_wav(cdir / "01_reference.wav", match_loudness(r_al), ref_sr)
            write_mono_wav(cdir / "02_before.wav", match_loudness(b_al), ref_sr)
            write_mono_wav(cdir / "03_after.wav", match_loudness(a_al), ref_sr)

            report_parts.extend(
                [
                    f"## Case: {cname}",
                    "### Before",
                    format_metrics_table(bm),
                    "",
                    "### After",
                    format_metrics_table(am),
                    "",
                ]
            )

        (out_dir / "report.md").write_text("\n".join(report_parts), encoding="utf-8")
        print(f"Optimization complete. Bundle saved to: {out_dir}")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Amp DSP perceptual diagnostic & CMA-ES optimizer")
    sub = p.add_subparsers(dest="cmd", required=True)

    diag = sub.add_parser("diagnose", help="Compare one DI/ref pair against current amp DSP")
    diag.add_argument("--amp", required=True)
    diag.add_argument("--di", required=True)
    diag.add_argument("--ref", required=True)
    diag.add_argument("--ir")
    diag.add_argument("--cab-off", action="store_true")
    diag.add_argument("--overrides")
    diag.add_argument("--in-gain-db", type=float, default=0.0)
    diag.add_argument("--gain", type=float, default=0.5)
    diag.add_argument("--bass", type=float, default=0.5)
    diag.add_argument("--mid", type=float, default=0.5)
    diag.add_argument("--treble", type=float, default=0.5)
    diag.add_argument("--presence", type=float, default=0.5)
    diag.add_argument("--master", type=float, default=0.5)

    run = sub.add_parser("run", help="Run CMA-ES optimization across a JSON list of cases")
    run.add_argument("--amp", required=True)
    run.add_argument("--cases", required=True, help="Path to JSON array of case dicts")
    run.add_argument("--mode", choices=["head", "cab", "all"], default="head")
    run.add_argument("--voice", choices=["default", "clean", "crunch"], default="default")
    run.add_argument("--base-overrides", help="Optional fixed base overrides file")
    run.add_argument("--params", help="Optional JSON param spec list")
    run.add_argument("--evals", type=int, default=120)
    run.add_argument("--sigma", type=float, default=0.2)
    run.add_argument("--seed", type=int, default=42)
    return p


def main() -> None:
    args = build_parser().parse_args()
    if args.cmd == "diagnose":
        cmd_diagnose(args)
    elif args.cmd == "run":
        cmd_run(args)


if __name__ == "__main__":
    main()
