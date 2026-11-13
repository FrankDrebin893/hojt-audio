//! Renders a WAV through the pitch shifter, for measuring and listening:
//!
//!   cargo run -p suite_common --release --example pitch_render -- in.wav out.wav <semitones> [latency ms] [rate factor]
//!
//! The input is taken as one channel. With a rate factor (4 is the Guitar Amp at 44.1 and
//! 48 kHz) the input is stepped up by it first and the output stepped down again, as the
//! pedal in front of the amp runs. The output is 32-bit float, so nothing clips.

use suite_common::dsp::pitch::PitchShifter;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: pitch_render in.wav out.wav <semitones> [latency ms] [rate factor]");
        std::process::exit(2);
    }
    let semitones: i32 = args[3].parse().expect("semitones must be a whole number");
    let latency_ms: Option<f32> = args.get(4).map(|value| value.parse().expect("latency in ms"));
    let factor: usize = args.get(5).map_or(1, |value| value.parse().expect("rate factor"));

    let mut reader = hound::WavReader::open(&args[1]).expect("could not open the input");
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|sample| sample.unwrap() as f32 * scale).collect()
        }
    };
    let input: Vec<f32> = samples.chunks(channels).map(|frame| frame[0]).collect();

    let mut shifter = PitchShifter::new();
    shifter.set_sample_rate(spec.sample_rate as f32 * factor as f32);
    shifter.set_semitones(semitones);
    if let Some(latency_ms) = latency_ms {
        shifter.set_latency_ms(latency_ms);
    }

    let out_spec = hound::WavSpec {
        channels: 1,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer =
        hound::WavWriter::create(&args[2], out_spec).expect("could not make the output");
    let started = std::time::Instant::now();
    for &sample in &input {
        // Stepped up by holding the sample and down by the mean: crude, and the same for
        // every engine it is used to compare
        let mut sum = 0.0;
        for _ in 0..factor {
            sum += shifter.process_sample(sample);
        }
        writer.write_sample(sum / factor as f32).unwrap();
    }
    writer.finalize().unwrap();
    let seconds = input.len() as f32 / spec.sample_rate as f32;
    eprintln!("{:.1} s of audio in {:.2} s", seconds, started.elapsed().as_secs_f32());
}
