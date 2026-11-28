use amp::bench::{
    apply_overrides, dump_model, parse_amp_name, read_mono_wav, render_buffer, write_mono_wav,
    RenderConfig,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn print_usage() {
    eprintln!(
        "Usage: amp_bench --amp <name> [--dump-model | --di <in.wav> --out <out.wav> [options]]\n\
         Options:\n\
         \x20 --ir <cab.wav>          Use external cabinet IR\n\
         \x20 --cab-off               Bypass cabinet section\n\
         \x20 --overrides <file.txt>  Apply key=value AmpModel overrides\n\
         \x20 --in-gain-db <f32>      Input trim in dB (default 0.0)\n\
         \x20 --gain <f32>            Gain dial 0..1 (default 0.5)\n\
         \x20 --bass <f32>            Bass dial 0..1 (default 0.5)\n\
         \x20 --mid <f32>             Mid dial 0..1 (default 0.5)\n\
         \x20 --treble <f32>          Treble dial 0..1 (default 0.5)\n\
         \x20 --presence <f32>        Presence dial 0..1 (default 0.5)\n\
         \x20 --master <f32>          Master dial 0..1 (default 0.5)"
    );
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_f32_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<f32, String> {
    let raw = next_arg(args, flag)?;
    raw.parse::<f32>().map_err(|_| format!("invalid float for {flag}: '{raw}'"))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut amp_name: Option<String> = None;
    let mut dump_only = false;
    let mut di_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut overrides_path: Option<PathBuf> = None;
    let mut cfg = RenderConfig::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            "--amp" => amp_name = Some(next_arg(&mut args, "--amp")?),
            "--dump-model" => dump_only = true,
            "--di" => di_path = Some(PathBuf::from(next_arg(&mut args, "--di")?)),
            "--out" => out_path = Some(PathBuf::from(next_arg(&mut args, "--out")?)),
            "--ir" => cfg.ir_path = Some(PathBuf::from(next_arg(&mut args, "--ir")?)),
            "--cab-off" => cfg.cab_off = true,
            "--overrides" => {
                overrides_path = Some(PathBuf::from(next_arg(&mut args, "--overrides")?));
            }
            "--in-gain-db" => cfg.in_gain_db = parse_f32_arg(&mut args, "--in-gain-db")?,
            "--gain" => cfg.gain = parse_f32_arg(&mut args, "--gain")?,
            "--bass" => cfg.bass = parse_f32_arg(&mut args, "--bass")?,
            "--mid" => cfg.mid = parse_f32_arg(&mut args, "--mid")?,
            "--treble" => cfg.treble = parse_f32_arg(&mut args, "--treble")?,
            "--presence" => cfg.presence = parse_f32_arg(&mut args, "--presence")?,
            "--master" => cfg.master = parse_f32_arg(&mut args, "--master")?,
            other => return Err(format!("unknown argument: '{other}'")),
        }
    }

    let amp_name = amp_name.ok_or_else(|| "missing --amp <name>".to_owned())?;
    let amp = parse_amp_name(&amp_name)
        .ok_or_else(|| format!("unknown amp '{amp_name}'"))?;
    let mut model = *amp.model();

    if let Some(path) = &overrides_path {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read overrides '{}': {e}", path.display()))?;
        apply_overrides(&mut model, &text)?;
    }

    if dump_only {
        print!("{}", dump_model(&model));
        return Ok(());
    }

    let di_path = di_path.ok_or_else(|| "missing --di <path.wav>".to_owned())?;
    let out_path = out_path.ok_or_else(|| "missing --out <path.wav>".to_owned())?;

    let (di, sample_rate) = read_mono_wav(&di_path)?;
    let rendered = render_buffer(&di, sample_rate as f32, &model, &cfg)?;
    write_mono_wav(&out_path, &rendered, sample_rate)?;
    Ok(())
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("amp_bench error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
