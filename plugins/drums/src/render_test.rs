//! Rendering the whole plugin the way a host plays it, for the tests of Human

use crate::kit::Kit;
use crate::{DrumSynthParams, DrumSynthPlugin};
use nih_plug::prelude::*;
use std::sync::Arc;

pub const SAMPLE_RATE: f32 = 48_000.0;

/// Events at absolute samples, in order: what a part is before a host cuts it into blocks
pub type Part = Vec<(u64, NoteEvent<()>)>;

pub fn on(note: u8, velocity: f32) -> NoteEvent<()> {
    NoteEvent::NoteOn { timing: 0, voice_id: None, channel: 0, note, velocity }
}

pub fn off(note: u8) -> NoteEvent<()> {
    NoteEvent::NoteOff { timing: 0, voice_id: None, channel: 0, note, velocity: 0.0 }
}

pub fn pedal(value: f32) -> NoteEvent<()> {
    NoteEvent::MidiCC { timing: 0, channel: 0, cc: 4, value }
}

/// A bar of rock at 120 bpm drawn on the grid, `bars` times: crash with the kick on the first
/// beat, kick on 1 and 3 (and the "and" of 3), snare on 2 and 4, closed hats on the eighths
pub fn rock_beat(bars: u64) -> Part {
    let eighth = (SAMPLE_RATE * 0.25) as u64;
    let mut part = Part::new();
    for bar in 0..bars {
        for step in 0..8u64 {
            let at = (bar * 8 + step) * eighth;
            if step == 0 && bar % 4 == 0 {
                part.push((at, on(49, 0.9)));
                part.push((at + 2 * eighth, off(49)));
            }
            if matches!(step, 0 | 4 | 5) {
                part.push((at, on(36, 0.9)));
            }
            if matches!(step, 2 | 6) {
                part.push((at, on(38, 0.85)));
            }
            part.push((at, on(42, if step % 2 == 0 { 0.7 } else { 0.5 })));
        }
    }
    part
}

/// The plugin as a host starts it, on `kit`, with Human at `human` (0.0 to 1.0) and Choke as given
pub fn plugin(kit: Kit, human: f32, choke: bool) -> DrumSynthPlugin {
    let params = DrumSynthParams {
        kit: EnumParam::new("Kit", kit),
        choke: BoolParam::new("Choke", choke),
        human: crate::percent_param("Human", human),
        ..DrumSynthParams::default()
    };
    let params = Arc::new(params);
    // A host starts the smoothers at the parameters' values; the test host cannot
    params.kick.smoothed.reset(params.kick.value());
    params.snare.smoothed.reset(params.snare.value());
    params.toms.smoothed.reset(params.toms.value());
    params.hihat.smoothed.reset(params.hihat.value());
    params.cymbals.smoothed.reset(params.cymbals.value());
    params.room.smoothed.reset(params.room.value());
    params.gain.smoothed.reset(params.gain.value());
    let mut plugin = DrumSynthPlugin { params, ..DrumSynthPlugin::default() };
    let layout = DrumSynthPlugin::AUDIO_IO_LAYOUTS[0];
    let config = BufferConfig {
        sample_rate: SAMPLE_RATE,
        min_buffer_size: None,
        max_buffer_size: 512,
        process_mode: ProcessMode::Realtime,
    };
    assert!(plugin.initialize(&layout, &config, &mut Host));
    plugin.reset();
    plugin
}

/// Plays `part` through `plugin` for `samples` samples in blocks of `block`: the left channel,
/// or both with `render_stereo`
pub fn render(plugin: &mut DrumSynthPlugin, part: &Part, samples: usize, block: usize) -> Vec<f32> {
    render_stereo(plugin, part, samples, block).into_iter().map(|(left, _)| left).collect()
}

/// `render`, both channels
pub fn render_stereo(
    plugin: &mut DrumSynthPlugin,
    part: &Part,
    samples: usize,
    block: usize,
) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(samples);
    let mut next = 0;
    let mut start = 0usize;
    while start < samples {
        let len = block.min(samples - start);
        let mut channels = [vec![0.0f32; len], vec![0.0f32; len]];
        let mut buffer = Buffer::default();
        // SAFETY: the slices are `channels`', which outlives `buffer`, and `len` long
        unsafe {
            buffer.set_slices(len, |slices| {
                slices.clear();
                slices.extend(channels.iter_mut().map(|channel| &mut channel[..]));
            });
        }
        let mut events = Vec::new();
        while let Some((at, event)) = part.get(next) {
            if *at >= (start + len) as u64 {
                break;
            }
            events.push(timed(*event, (*at - start as u64) as u32));
            next += 1;
        }
        let mut aux = AuxiliaryBuffers { inputs: &mut [], outputs: &mut [] };
        let mut context = Block { events, next: 0 };
        plugin.process(&mut buffer, &mut aux, &mut context);
        drop(buffer);
        out.extend(channels[0].iter().cloned().zip(channels[1].iter().cloned()));
        start += len;
    }
    out
}

/// `event` at `timing` in its block (the events of a part are made with timing 0)
fn timed(event: NoteEvent<()>, timing: u32) -> NoteEvent<()> {
    match event {
        NoteEvent::NoteOn { voice_id, channel, note, velocity, .. } => {
            NoteEvent::NoteOn { timing, voice_id, channel, note, velocity }
        }
        NoteEvent::NoteOff { voice_id, channel, note, velocity, .. } => {
            NoteEvent::NoteOff { timing, voice_id, channel, note, velocity }
        }
        NoteEvent::MidiCC { channel, cc, value, .. } => {
            NoteEvent::MidiCC { timing, channel, cc, value }
        }
        other => other,
    }
}

/// FNV-1a over the bits of the samples: two renders are the same to the bit when this is
pub fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().fold(0xcbf2_9ce4_8422_2325, |hash, sample| {
        (hash ^ sample.to_bits() as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

struct Host;

impl InitContext<DrumSynthPlugin> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn execute(&self, _task: ()) {}
    fn set_latency_samples(&self, _samples: u32) {}
    fn set_current_voice_capacity(&self, _capacity: u32) {}
}

struct Block {
    events: Vec<NoteEvent<()>>,
    next: usize,
}

impl ProcessContext<DrumSynthPlugin> for Block {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn execute_background(&self, _task: ()) {}
    fn execute_gui(&self, _task: ()) {}
    fn transport(&self) -> &Transport {
        unimplemented!("the test host has no transport")
    }
    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        let event = self.events.get(self.next).copied();
        self.next += 1;
        event
    }
    fn send_event(&mut self, _event: NoteEvent<()>) {}
    fn set_latency_samples(&self, _samples: u32) {}
    fn set_current_voice_capacity(&self, _capacity: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part with every kind of event: hits, a pedal that moves and a hat that follows it, a
    /// cymbal let go, all of it in the first second
    fn busy_part() -> Part {
        let mut part: Part = rock_beat(1).into_iter().filter(|(at, _)| *at < 40_000).collect();
        part.push((30_000, pedal(0.2)));
        part.push((30_001, on(46, 0.8)));
        part.push((36_000, pedal(1.0)));
        part.push((36_001, on(46, 0.8)));
        part.push((39_000, on(51, 0.6)));
        part.push((39_000, off(51)));
        part.sort_by_key(|(at, _)| *at);
        part
    }

    /// The plugin as it was before Human, as a host plays it: every event goes to the kit at the
    /// sample it arrives on (`set_time` there), with choke on and the parameters as `plugin`
    /// leaves them. The left channel
    fn reference(kit: Kit, part: &Part, samples: usize) -> Vec<f32> {
        use crate::drum_kit::{DrumKit, Stems};
        let params = plugin(kit, 0.0, true).params.clone();
        let mut drums = DrumKit::new();
        drums.set_sample_rate(SAMPLE_RATE);
        drums.reset();
        drums.set_kit(params.kit.value());
        drums.set_tune(params.tune.value());
        drums.set_damping(params.damping.value());
        let mut next = 0;
        let mut out = Vec::with_capacity(samples);
        for now in 0..samples as u64 {
            while let Some((at, event)) = part.get(next) {
                if *at > now {
                    break;
                }
                drums.set_time(now);
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } if *velocity <= 0.0 => {
                        drums.note_off(*note)
                    }
                    NoteEvent::NoteOn { note, velocity, .. } => drums.note_on(*note, *velocity),
                    NoteEvent::NoteOff { note, .. } => drums.note_off(*note),
                    NoteEvent::MidiCC { cc: 4, value, .. } => drums.set_hat_openness(1.0 - value),
                    _ => {}
                }
                next += 1;
            }
            let mix = &mut drums.mix;
            mix.kick = params.kick.value();
            mix.snare = params.snare.value();
            mix.toms = params.toms.value();
            mix.hihat = params.hihat.value();
            mix.cymbals = params.cymbals.value();
            mix.room = params.room.value();
            let (left, _) = drums.process_stems(&mut Stems::default());
            out.push(left * params.gain.value());
        }
        out
    }

    /// Human at 0 is the plugin as it was, to the bit, on every kit: the same part through the
    /// plugin and through a kit driven directly the way the plugin did before Human. Both run on
    /// this machine, so no platform's maths comes into it
    #[test]
    fn test_human_at_0_renders_what_the_build_before_did_to_the_bit() {
        for kit in Kit::ALL {
            let mut part = rock_beat(1);
            part.push((30_000, pedal(0.2)));
            part.push((60_000, pedal(1.0)));
            part.push((60_001, on(46, 0.8)));
            part.push((70_000, off(51)));
            part.sort_by_key(|(at, _)| *at);
            let before = reference(kit, &part, 120_000);
            for block in [512, 77] {
                let now = render(&mut plugin(kit, 0.0, true), &part, 120_000, block);
                assert_eq!(fingerprint(&now), fingerprint(&before), "{kit:?}, blocks of {block}");
            }
        }
    }

    #[test]
    fn test_the_same_part_renders_the_same_twice() {
        let part = busy_part();
        let a = render(&mut plugin(Kit::Rock, 1.0, true), &part, 48_000, 512);
        let b = render(&mut plugin(Kit::Rock, 1.0, true), &part, 48_000, 512);
        assert_eq!(fingerprint(&a), fingerprint(&b), "two plugins");
        let plain = render(&mut plugin(Kit::Rock, 0.0, true), &part, 48_000, 512);
        assert_ne!(fingerprint(&a), fingerprint(&plain), "Human at 100 % changed nothing");
    }

    #[test]
    fn test_a_block_boundary_in_the_middle_of_a_delay_changes_nothing() {
        let part = busy_part();
        let whole = render(&mut plugin(Kit::Rock, 1.0, true), &part, 48_000, 512);
        for block in [1, 64, 333] {
            let cut = render(&mut plugin(Kit::Rock, 1.0, true), &part, 48_000, block);
            assert_eq!(fingerprint(&whole), fingerprint(&cut), "blocks of {block}");
        }
    }

    /// A hit is delayed and never early: the first sound of a single kick is at its own sample
    /// or up to 8 ms after it. Silent pedal moves before it use up draws, so each plugin
    /// draws a different delay for the kick
    #[test]
    fn test_a_hit_is_never_early_and_at_most_8_ms_late() {
        let max = (crate::human::MAX_DELAY_S * SAMPLE_RATE) as usize;
        let at = 1000u64;
        let mut late = 0;
        for draws_before in 0..40u64 {
            let mut part: Part = (0..draws_before).map(|i| (i * 10, pedal(0.0))).collect();
            part.push((at, on(36, 0.9)));
            let first_sound = |human: f32| {
                let out = render(&mut plugin(Kit::Rock, human, false), &part, 4000, 512);
                out.iter().position(|sample| *sample != 0.0).expect("the kick made no sound")
            };
            let plain = first_sound(0.0);
            let humanised = first_sound(1.0);
            assert!(humanised >= plain, "a hit came early (after {draws_before} draws)");
            assert!(humanised - plain <= max, "a hit was {} late", humanised - plain);
            late += (humanised > plain) as usize;
        }
        assert!(late > 20, "only {late} of 40 hits were late");
    }

    #[test]
    fn test_human_at_100_percent_and_a_note_off_still_damps_the_cymbal() {
        // A crash let go after a second: with Choke on it is damped, with the delay as without
        let part: Part = vec![(0, on(49, 0.9)), (48_000, off(49))];
        let rings = |human: f32| {
            let out = render(&mut plugin(Kit::Rock, human, true), &part, 96_000, 512);
            crate::test_util::rms(&out[60_000..90_000])
        };
        let held = |human: f32| {
            let part: Part = vec![(0, on(49, 0.9))];
            let out = render(&mut plugin(Kit::Rock, human, true), &part, 96_000, 512);
            crate::test_util::rms(&out[60_000..90_000])
        };
        for human in [0.0, 1.0] {
            assert!(rings(human) < held(human) * 0.3, "the off did not damp at Human {human}");
        }
    }

    /// The four-bar rock beat drawn on the grid, through every kit with Human at 0 and at 100 %,
    /// as WAV files to listen to. DRUMS_LISTEN_DIR names where they go (default target/renders):
    ///   cargo test -p drums --release render_human_wavs -- --ignored
    #[test]
    #[ignore]
    fn render_human_wavs() {
        let dir =
            std::env::var("DRUMS_LISTEN_DIR").map(std::path::PathBuf::from).unwrap_or_else(|_| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/renders")
            });
        std::fs::create_dir_all(&dir).unwrap();
        // Four bars of eight eighths at 120 bpm, and a second for the last crash to ring
        let samples = 4 * 8 * 12_000 + 48_000;
        let mut part = rock_beat(4);
        // The crash is let go on the first beat of the last bar
        part.push((3 * 96_000 + 96_000 / 2, off(49)));
        part.sort_by_key(|(at, _)| *at);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        for kit in Kit::ALL {
            for (human, label) in [(0.0, "human0"), (1.0, "human100")] {
                let out = render_stereo(&mut plugin(kit, human, false), &part, samples, 512);
                let name = format!("drums_{:?}_rock_beat_{label}.wav", kit).to_lowercase();
                let mut writer = hound::WavWriter::create(dir.join(name), spec).unwrap();
                for (left, right) in out {
                    for sample in [left, right] {
                        writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
                    }
                }
                writer.finalize().unwrap();
            }
        }
    }
}
