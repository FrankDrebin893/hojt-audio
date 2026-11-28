//! Offline rendering and model-override helpers for benchmarking and tuning amp DSP.

use crate::amp::model::{Amp, AmpModel, MAX_STAGES};
use crate::amp::poweramp::PowerAmp;
use crate::amp::preamp::Preamp;
use crate::amp::tonestack::{ToneCurve, ToneStack};
use crate::cab::{design_ir, tail_spectra, Cabinet, MAX_TAIL_SPECTRA_LEN};
use crate::dsp::filters::DcBlocker;
use crate::dsp::oversample::{factor_for, Oversampler, MAX_FACTOR};
use crate::dsp::shaper::output_clip;
use crate::dsp::{curve, db_to_gain};
use crate::effect::CHUNK;
use crate::user_cab;
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// Dial and routing settings for an offline render.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    pub in_gain_db: f32,
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,
    pub cab_off: bool,
    pub ir_path: Option<PathBuf>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            in_gain_db: 0.0,
            gain: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            master: 0.5,
            cab_off: false,
            ir_path: None,
        }
    }
}

/// Finds an amp by its display name (case-insensitive).
pub fn parse_amp_name(name: &str) -> Option<Amp> {
    Amp::ALL.iter().copied().find(|amp| amp.model().name.eq_ignore_ascii_case(name))
}

fn fmt_opt(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{v}"),
        None => "none".to_owned(),
    }
}

fn fmt_slice(values: &[f32]) -> String {
    values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(", ")
}

/// Formats all tunable numeric fields of `AmpModel` as `key = value` lines.
pub fn dump_model(model: &AmpModel) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "tight_hz = {}", model.tight_hz);
    let _ = writeln!(out, "focus = {}", fmt_slice(&model.focus));
    let _ = writeln!(out, "bright_hz = {}", model.bright_hz);
    let _ = writeln!(out, "bright_db = {}", model.bright_db);
    let _ = writeln!(out, "stage_count = {}", model.stage_count);
    let _ = writeln!(out, "fizz_hz = {}", fmt_opt(model.fizz_hz));
    let _ = writeln!(out, "lowcut_hz = {}", fmt_opt(model.lowcut_hz));
    let _ = writeln!(out, "mud_cut = {}", fmt_slice(&model.mud_cut));
    let _ = writeln!(out, "air_shelf = {}", fmt_slice(&model.air_shelf));
    let _ = writeln!(out, "level_db = {}", fmt_slice(&model.level_db));
    let _ = writeln!(out, "makeup_db = {}", fmt_slice(&model.makeup_db));

    for (i, stage) in model.stages.iter().enumerate() {
        let _ = writeln!(out, "stage{i}.coupling_hz = {}", stage.coupling_hz);
        let _ = writeln!(out, "stage{i}.gain_db = {}", fmt_slice(&stage.gain_db));
        let _ = writeln!(out, "stage{i}.headroom = {}", fmt_slice(&stage.headroom));
        let _ = writeln!(out, "stage{i}.bias = {}", stage.bias);
        let _ = writeln!(out, "stage{i}.bias_shift = {}", stage.bias_shift);
        let _ = writeln!(out, "stage{i}.lowpass_hz = {}", stage.lowpass_hz);
    }

    let t = &model.tone;
    let _ = writeln!(out, "tone.bass_hz = {}", t.bass_hz);
    let _ = writeln!(out, "tone.bass_db = {}", t.bass_db);
    let _ = writeln!(out, "tone.mid_hz = {}", t.mid_hz);
    let _ = writeln!(out, "tone.mid_q = {}", t.mid_q);
    let _ = writeln!(out, "tone.mid_db = {}", t.mid_db);
    let _ = writeln!(out, "tone.mid_centre_db = {}", t.mid_centre_db);
    let _ = writeln!(out, "tone.scoop_db = {}", t.scoop_db);
    let _ = writeln!(out, "tone.mid_shift_octaves = {}", t.mid_shift_octaves);
    let _ = writeln!(out, "tone.treble_hz = {}", t.treble_hz);
    let _ = writeln!(out, "tone.treble_db = {}", t.treble_db);
    let _ = writeln!(out, "tone.mid_level_db = {}", t.mid_level_db);

    let p = &model.power;
    let _ = writeln!(out, "power.drive_db = {}", fmt_slice(&p.drive_db));
    let _ = writeln!(out, "power.volume_db = {}", fmt_slice(&p.volume_db));
    let _ = writeln!(out, "power.clean_to = {}", p.clean_to);
    let _ = writeln!(out, "power.sag = {}", p.sag);
    let _ = writeln!(out, "power.sag_attack_ms = {}", p.sag_attack_ms);
    let _ = writeln!(out, "power.sag_release_ms = {}", p.sag_release_ms);
    let _ = writeln!(out, "power.resonance_hz = {}", p.resonance_hz);
    let _ = writeln!(out, "power.resonance_db = {}", p.resonance_db);
    let _ = writeln!(out, "power.presence_hz = {}", p.presence_hz);
    let _ = writeln!(out, "power.presence_db = {}", p.presence_db);

    let c = &model.cab;
    let _ = writeln!(out, "cab.resonance_hz = {}", c.resonance_hz);
    let _ = writeln!(out, "cab.resonance_q = {}", c.resonance_q);
    let _ = writeln!(out, "cab.body = {}", fmt_slice(&c.body));
    let _ = writeln!(out, "cab.dip = {}", fmt_slice(&c.dip));
    let _ = writeln!(out, "cab.bite = {}", fmt_slice(&c.bite));
    let _ = writeln!(out, "cab.rolloff_hz = {}", c.rolloff_hz);
    let _ = writeln!(out, "cab.air_ratio = {}", c.air_ratio);
    let _ = writeln!(out, "cab.breakup_db = {}", c.breakup_db);
    out
}

fn parse_f32(raw: &str, key: &str) -> Result<f32, String> {
    raw.trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid float for {key}: '{raw}'"))
        .and_then(|v| if v.is_finite() { Ok(v) } else { Err(format!("non-finite float for {key}")) })
}

fn parse_opt_f32(raw: &str, key: &str) -> Result<Option<f32>, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        Ok(Some(parse_f32(trimmed, key)?))
    }
}

fn parse_array<const N: usize>(raw: &str, key: &str) -> Result<[f32; N], String> {
    let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
    if parts.len() != N {
        return Err(format!("expected {N} comma-separated floats for {key}, got {}", parts.len()));
    }
    let mut out = [0.0f32; N];
    for (dst, src) in out.iter_mut().zip(parts) {
        *dst = parse_f32(src, key)?;
    }
    Ok(out)
}

/// Applies `key = value` overrides (ignoring blank lines and `#` comments) to `model`.
pub fn apply_overrides(model: &mut AmpModel, text: &str) -> Result<(), String> {
    for (line_no, raw_line) in text.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            return Err(format!("line {}: expected 'key = value', got '{line}'", line_no + 1));
        };
        let key = key.trim();
        let val = val.trim();

        match key {
            "tight_hz" => model.tight_hz = parse_f32(val, key)?,
            "focus" => model.focus = parse_array(val, key)?,
            "bright_hz" => model.bright_hz = parse_f32(val, key)?,
            "bright_db" => model.bright_db = parse_f32(val, key)?,
            "stage_count" => {
                let count = val
                    .parse::<usize>()
                    .map_err(|_| format!("invalid integer for {key}: '{val}'"))?;
                model.stage_count = count.clamp(1, MAX_STAGES);
            }
            "fizz_hz" => model.fizz_hz = parse_opt_f32(val, key)?,
            "lowcut_hz" => model.lowcut_hz = parse_opt_f32(val, key)?,
            "mud_cut" => model.mud_cut = parse_array(val, key)?,
            "air_shelf" => model.air_shelf = parse_array(val, key)?,
            "level_db" => model.level_db = parse_array(val, key)?,
            "makeup_db" => model.makeup_db = parse_array(val, key)?,

            "tone.bass_hz" => model.tone.bass_hz = parse_f32(val, key)?,
            "tone.bass_db" => model.tone.bass_db = parse_f32(val, key)?,
            "tone.mid_hz" => model.tone.mid_hz = parse_f32(val, key)?,
            "tone.mid_q" => model.tone.mid_q = parse_f32(val, key)?,
            "tone.mid_db" => model.tone.mid_db = parse_f32(val, key)?,
            "tone.mid_centre_db" => model.tone.mid_centre_db = parse_f32(val, key)?,
            "tone.scoop_db" => model.tone.scoop_db = parse_f32(val, key)?,
            "tone.mid_shift_octaves" => model.tone.mid_shift_octaves = parse_f32(val, key)?,
            "tone.treble_hz" => model.tone.treble_hz = parse_f32(val, key)?,
            "tone.treble_db" => model.tone.treble_db = parse_f32(val, key)?,
            "tone.mid_level_db" => model.tone.mid_level_db = parse_f32(val, key)?,

            "power.drive_db" => model.power.drive_db = parse_array(val, key)?,
            "power.volume_db" => model.power.volume_db = parse_array(val, key)?,
            "power.clean_to" => model.power.clean_to = parse_f32(val, key)?,
            "power.sag" => model.power.sag = parse_f32(val, key)?,
            "power.sag_attack_ms" => model.power.sag_attack_ms = parse_f32(val, key)?,
            "power.sag_release_ms" => model.power.sag_release_ms = parse_f32(val, key)?,
            "power.resonance_hz" => model.power.resonance_hz = parse_f32(val, key)?,
            "power.resonance_db" => model.power.resonance_db = parse_f32(val, key)?,
            "power.presence_hz" => model.power.presence_hz = parse_f32(val, key)?,
            "power.presence_db" => model.power.presence_db = parse_f32(val, key)?,

            "cab.resonance_hz" => model.cab.resonance_hz = parse_f32(val, key)?,
            "cab.resonance_q" => model.cab.resonance_q = parse_f32(val, key)?,
            "cab.body" => model.cab.body = parse_array(val, key)?,
            "cab.dip" => model.cab.dip = parse_array(val, key)?,
            "cab.bite" => model.cab.bite = parse_array(val, key)?,
            "cab.rolloff_hz" => model.cab.rolloff_hz = parse_f32(val, key)?,
            "cab.air_ratio" => model.cab.air_ratio = parse_f32(val, key)?,
            "cab.breakup_db" => model.cab.breakup_db = parse_f32(val, key)?,

            _ if key.starts_with("stage") => {
                let Some((prefix, field)) = key.split_once('.') else {
                    return Err(format!("unknown key: '{key}'"));
                };
                let idx = prefix
                    .strip_prefix("stage")
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|&i| i < MAX_STAGES)
                    .ok_or_else(|| format!("invalid stage index in '{key}'"))?;
                let stage = &mut model.stages[idx];
                match field {
                    "coupling_hz" => stage.coupling_hz = parse_f32(val, key)?,
                    "gain_db" => stage.gain_db = parse_array(val, key)?,
                    "headroom" => stage.headroom = parse_array(val, key)?,
                    "bias" => stage.bias = parse_f32(val, key)?,
                    "bias_shift" => stage.bias_shift = parse_f32(val, key)?,
                    "lowpass_hz" => stage.lowpass_hz = parse_f32(val, key)?,
                    _ => return Err(format!("unknown stage field: '{key}'")),
                }
            }
            _ => return Err(format!("unknown key: '{key}'")),
        }
    }
    Ok(())
}

/// Renders a mono DI buffer through `model` and `cfg` at `sample_rate`.
pub fn render_buffer(
    di: &[f32],
    sample_rate: f32,
    model: &AmpModel,
    cfg: &RenderConfig,
) -> Result<Vec<f32>, String> {
    let factor = factor_for(sample_rate);
    let oversampled_rate = sample_rate * factor as f32;

    let mut oversampler = Oversampler::new();
    oversampler.set_factor(factor);

    let mut preamp = Preamp::new();
    preamp.configure(model, oversampled_rate);
    preamp.reset();
    preamp.set_gain(model, cfg.gain.clamp(0.0, 1.0), 0);
    preamp.snap();

    let mut tone = ToneStack::new();
    let tone_curve = ToneCurve::new(
        &model.tone,
        cfg.bass.clamp(0.0, 1.0),
        cfg.mid.clamp(0.0, 1.0),
        cfg.treble.clamp(0.0, 1.0),
        oversampled_rate,
    );
    tone.set(&tone_curve, 0);
    tone.reset();

    let mut power = PowerAmp::new();
    power.configure(&model.power, oversampled_rate);
    power.reset();
    power.set_master(
        &model.power,
        cfg.master.clamp(0.0, 1.0),
        curve(&model.makeup_db, cfg.gain.clamp(0.0, 1.0)),
        0,
    );
    power.set_presence(&model.power, cfg.presence.clamp(0.0, 1.0), oversampled_rate, 0);
    power.snap();

    let mut cabinet = Cabinet::new();
    if !cfg.cab_off {
        cabinet.set_sample_rate(sample_rate);
        if let Some(ir_path) = &cfg.ir_path {
            let taps = user_cab::load(ir_path, sample_rate)
                .map_err(|err| format!("failed to load IR '{}': {err:?}", ir_path.display()))?;
            let mut spectra = vec![0.0f32; MAX_TAIL_SPECTRA_LEN];
            let used = tail_spectra(&taps, sample_rate, &mut spectra);
            cabinet.set_taps(&taps, &spectra[..used]);
        } else {
            let ir = design_ir(&model.cab, sample_rate);
            cabinet.set_ir(&ir);
        }
    }

    let mut dc = DcBlocker::new();
    dc.set_sample_rate(sample_rate);

    let in_gain = db_to_gain(cfg.in_gain_db);
    let mut output = Vec::with_capacity(di.len());
    let mut block = [0.0f32; CHUNK];
    let mut up = [0.0f32; CHUNK * MAX_FACTOR];

    for chunk in di.chunks(CHUNK) {
        let n = chunk.len();
        for (dst, &src) in block[..n].iter_mut().zip(chunk) {
            *dst = src * in_gain;
        }
        let up_len = n * factor;
        oversampler.upsample(&block[..n], &mut up[..up_len]);
        crate::amp::process_amp_block(
            model.topology,
            &mut preamp,
            &mut tone,
            &mut power,
            &mut up[..up_len],
        );
        oversampler.downsample(&up[..up_len], &mut block[..n]);

        if !cfg.cab_off {
            cabinet.process(&mut block[..n]);
        }

        for &sample in &block[..n] {
            output.push(output_clip(dc.process(sample)));
        }
    }

    Ok(output)
}

/// Reads an entire mono (or stereo left-channel) WAV file as `f32` samples.
pub fn read_mono_wav(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let file = File::open(path).map_err(|e| format!("open '{}': {e}", path.display()))?;
    let mut reader = hound::WavReader::new(BufReader::new(file))
        .map_err(|e| format!("read wav '{}': {e}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    if channels == 0 {
        return Err(format!("wav '{}' has 0 channels", path.display()));
    }
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => {
            reader.samples::<f32>().map_while(Result::ok).step_by(channels).collect()
        }
        (hound::SampleFormat::Int, bits @ (8 | 16 | 24 | 32)) => {
            let scale = 1.0 / (1_i64 << (bits - 1)) as f64;
            reader
                .samples::<i32>()
                .map_while(Result::ok)
                .step_by(channels)
                .map(|s| (s as f64 * scale) as f32)
                .collect()
        }
        _ => {
            return Err(format!(
                "unsupported wav format {:?} {}-bit in '{}'",
                spec.sample_format,
                spec.bits_per_sample,
                path.display()
            ));
        }
    };
    Ok((samples, spec.sample_rate))
}

/// Writes a mono 32-bit float WAV file.
pub fn write_mono_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create dir '{}': {e}", parent.display()))?;
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("create wav '{}': {e}", path.display()))?;
    for &sample in samples {
        writer.write_sample(sample).map_err(|e| format!("write wav '{}': {e}", path.display()))?;
    }
    writer.finalize().map_err(|e| format!("finalize wav '{}': {e}", path.display()))?;
    Ok(())
}
