//! Nothing allocates on the audio thread (Code Conventions in CLAUDE.md)

use crate::chain::AmpChain;
use crate::presets::PRESETS;
use crate::GuitarAmpPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    host::assert_process_allocates_nothing("Guitar Amp", &mut GuitarAmpPlugin::default(), 200, &[]);
}

/// The defaults leave most pedals off: every factory preset, which between them turn every
/// pedal and every amp on, through the chain itself, in stereo and in mono
#[test]
fn test_every_preset_allocates_nothing() {
    let mut chain = AmpChain::new();
    chain.set_sample_rate(host::SAMPLE_RATE);
    let mut left = vec![0.0f32; host::BLOCKS[0]];
    let mut right = vec![0.0f32; host::BLOCKS[0]];
    for preset in &PRESETS {
        let settings = preset.settings();
        host::assert_allocates_nothing(&format!("The preset {}", preset.name), || {
            for block in 0..100 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The second amp: off, on, switched on and off in the middle of a block, and its amp
/// switched while it plays (and the first amp's, beside it)
#[test]
fn test_the_second_amp_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::second::SecondSettings;
    use crate::Amp;
    let mut chain = AmpChain::new();
    chain.set_sample_rate(host::SAMPLE_RATE);
    let mut left = vec![0.0f32; host::BLOCKS[0]];
    let mut right = vec![0.0f32; host::BLOCKS[0]];
    let second = |on: bool, amp: Amp| AmpSettings {
        amp: Amp::Torden,
        second: SecondSettings { on, amp, ..SecondSettings::default() },
        blend: 0.4,
        ..AmpSettings::default()
    };
    // The same with a built-in cabinet for the second amp, switched while it plays
    let second_through = |on: bool, speaker: usize| AmpSettings {
        amp: Amp::Torden,
        second: SecondSettings {
            on,
            speaker: Some(speaker % crate::amp::model::SPEAKERS.len()),
            ..SecondSettings::default()
        },
        blend: 0.4,
        ..AmpSettings::default()
    };
    host::assert_allocates_nothing("The second amp", || {
        for block in 0..400 {
            let len = host::BLOCKS[block % host::BLOCKS.len()];
            for i in 0..len {
                left[i] = (i as f32 * 0.0144).sin() * 0.5;
                right[i] = left[i];
            }
            // Off for a stretch, on, switched on and off again every few blocks, and its
            // amp (and the first one's) switched while it plays
            let settings = match block / 25 % 4 {
                0 => second(false, Amp::Klar),
                1 => second(true, Amp::ALL[block / 5 % Amp::ALL.len()]),
                2 if block % 2 == 0 => second_through(true, block / 3),
                2 => second(block % 3 != 0, Amp::Klar),
                _ => AmpSettings {
                    amp: Amp::ALL[block % Amp::ALL.len()],
                    ..second(true, Amp::ALL[(block + 2) % Amp::ALL.len()])
                },
            };
            chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
            chain.process(&settings, &mut left[..len], None);
        }
    });
}

/// The pitch shifter pedal: off, on at each shift, switched on and off in the middle of a block
/// and its shift and tracking turned while it plays, in front of the amp, at the rates that
/// oversample 4 and 2 times
#[test]
fn test_the_pitch_shifter_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::pitch::PitchSettings;
    for sample_rate in [44_100.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("The pitch shifter", || {
            for block in 0..600 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                // Off for a stretch, on at one shift after the other, then switched on and off
                // every few blocks, with the shift and the tracking turning
                let pitch = match block / 50 % 4 {
                    0 => PitchSettings::default(),
                    1 => PitchSettings {
                        on: true,
                        semitones: (block as i32 / 5) % 25 - 12,
                        ..PitchSettings::default()
                    },
                    2 => PitchSettings {
                        on: block % 3 != 0,
                        semitones: -5,
                        blend: 0.5,
                        latency_ms: 2.0 + (block % 49) as f32,
                    },
                    _ => PitchSettings { on: true, semitones: 7, ..PitchSettings::default() },
                };
                let settings = AmpSettings { pitch, ..AmpSettings::default() };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The metronome: off, on in each sound and time signature, following a host that plays and
/// stops, with the tempo turned while it clicks, at the rates a host runs at
#[test]
fn test_the_metronome_allocates_nothing() {
    use crate::metronome::{Metronome, MetronomeSettings, Sound, TimeSignature};
    for sample_rate in [44_100.0, 96_000.0] {
        let mut metronome = Metronome::new();
        metronome.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        let mut heard = false;
        host::assert_allocates_nothing("The metronome", || {
            for block in 0..600 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                left.fill(0.0);
                right.fill(0.0);
                let settings = MetronomeSettings {
                    on: block / 20 % 5 != 0,
                    bpm: 40.0 + (block % 261) as f32,
                    sync: block / 30 % 2 == 1,
                    time_signature: TimeSignature::ALL[block / 7 % 4],
                    sound: Sound::ALL[block / 11 % 3],
                    gain: 0.5,
                    host_tempo: (block % 9 != 0).then_some(60.0 + (block % 100) as f64),
                    host_playing: block % 13 != 0,
                    host_beats: (block % 5 != 0).then_some(block as f64 * 0.37),
                };
                metronome.process(&settings, &mut left[..len], Some(&mut right[..len]));
                metronome.process(&settings, &mut left[..len], None);
                heard |= left[..len].iter().any(|sample| *sample != 0.0);
            }
        });
        assert!(heard, "the test did not play the metronome");
    }
}

/// The plugin's own `process` with the metronome on, as a host calls it
#[test]
fn test_process_with_the_metronome_on_allocates_nothing() {
    use nih_plug::prelude::BoolParam;
    let mut plugin = GuitarAmpPlugin {
        params: std::sync::Arc::new(crate::GuitarAmpParams {
            metronome_on: BoolParam::new("Metronome", true),
            ..crate::GuitarAmpParams::default()
        }),
        ..GuitarAmpPlugin::default()
    };
    host::assert_process_allocates_nothing("Guitar Amp with the metronome", &mut plugin, 200, &[]);
}

/// The click on its own output: `process` is given the "Metronome" pair by the test host,
/// writes the click there and the amp alone to the main output, and allocates nothing
#[test]
fn test_process_with_the_click_on_its_own_output_allocates_nothing() {
    use crate::metronome::ClickOut;
    use nih_plug::prelude::{BoolParam, EnumParam, Plugin};
    let mut plugin = GuitarAmpPlugin {
        params: std::sync::Arc::new(crate::GuitarAmpParams {
            metronome_on: BoolParam::new("Metronome", true),
            metronome_out: EnumParam::new("Metronome Output", ClickOut::Separate),
            ..crate::GuitarAmpParams::default()
        }),
        ..GuitarAmpPlugin::default()
    };
    assert_eq!(GuitarAmpPlugin::AUDIO_IO_LAYOUTS[0].aux_output_ports.len(), 1);
    host::assert_process_allocates_nothing(
        "Guitar Amp with the click apart",
        &mut plugin,
        200,
        &[],
    );
}

/// The EQ pedal: off, on with every band moved, a dial turned while it plays and switched on
/// and off in the middle of a block, in front of the amp (at the rates that oversample 4 and
/// 2 times) and behind the cabinet
#[test]
fn test_the_eq_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::eq::EqSettings;
    use crate::order::{ChainOrder, Pedal, Zone};
    for sample_rate in [44_100.0, 192_000.0] {
        for zone in [Zone::Front, Zone::Back] {
            let mut chain = AmpChain::new();
            chain.set_sample_rate(sample_rate);
            let order = ChainOrder::default().moved(Pedal::Eq, zone, 0);
            let mut left = vec![0.0f32; host::BLOCKS[0]];
            let mut right = vec![0.0f32; host::BLOCKS[0]];
            host::assert_allocates_nothing("The EQ", || {
                for block in 0..300 {
                    let len = host::BLOCKS[block % host::BLOCKS.len()];
                    for i in 0..len {
                        left[i] = (i as f32 * 0.0144).sin() * 0.5;
                        right[i] = left[i];
                    }
                    let dial = (block % 40) as f32 / 40.0;
                    let eq = match block / 50 % 3 {
                        0 => EqSettings::default(),
                        1 => EqSettings {
                            on: true,
                            low: dial,
                            mid: 1.0 - dial,
                            freq: dial,
                            high: dial,
                        },
                        _ => EqSettings {
                            on: block % 2 == 0,
                            low: 0.9,
                            mid: 0.1,
                            freq: dial,
                            high: 0.0,
                        },
                    };
                    let settings =
                        AmpSettings { eq, layout: order.into(), ..AmpSettings::default() };
                    chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                    chain.process(&settings, &mut left[..len], None);
                }
            });
        }
    }
}

/// The wah pedal: off, on in Pedal mode with the rocker swept and Bite turned, on in Auto mode
/// with Sens turned, the mode switched while it plays, and switched on and off in the middle of
/// a block, in front of the amp at the rates that oversample 4 and 2 times
#[test]
fn test_the_wah_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::wah::{WahMode, WahSettings};
    for sample_rate in [44_100.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("The wah", || {
            for block in 0..600 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                let dial = (block % 40) as f32 / 40.0;
                let wah = match block / 50 % 4 {
                    0 => WahSettings::default(),
                    1 => WahSettings {
                        on: true,
                        pedal: dial,
                        bite: 1.0 - dial,
                        ..WahSettings::default()
                    },
                    2 => WahSettings {
                        on: true,
                        mode: WahMode::Auto,
                        pedal: 0.1,
                        sens: dial,
                        ..WahSettings::default()
                    },
                    _ => WahSettings {
                        on: block % 3 != 0,
                        mode: if block % 2 == 0 { WahMode::Pedal } else { WahMode::Auto },
                        pedal: dial,
                        bite: dial,
                        sens: 1.0 - dial,
                    },
                };
                let settings = AmpSettings { wah, ..AmpSettings::default() };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The delay and the reverb in front of the amp (#268), on the oversampled rate, and moved from
/// one side of the amp to the other while they play, at the rates that oversample 4 and 2 times
#[test]
fn test_the_delay_and_the_reverb_allocate_nothing_in_front_of_the_amp() {
    use crate::chain::AmpSettings;
    use crate::delay::DelaySettings;
    use crate::order::{ChainOrder, Pedal, Zone};
    use crate::reverb::ReverbSettings;
    for sample_rate in [44_100.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("The effects in front", || {
            for block in 0..600 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                let dial = (block % 40) as f32 / 40.0;
                // In front for the first half of each stretch of a hundred blocks, behind for
                // the rest
                let zone = if block % 100 < 50 { Zone::Front } else { Zone::Back };
                let order = ChainOrder::default().moved(Pedal::Delay, zone, 0).moved(
                    Pedal::Reverb,
                    zone,
                    1,
                );
                let settings = AmpSettings {
                    delay: DelaySettings {
                        on: block % 7 != 0,
                        time_ms: 20.0 + 900.0 * dial,
                        feedback: dial,
                        mix: 1.0 - dial,
                    },
                    reverb: ReverbSettings { on: block % 5 != 0, ..ReverbSettings::default() },
                    layout: order.into(),
                    ..AmpSettings::default()
                };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The drive pedal, the wah and the pitch shifter on both sides of the amp (#268), moved from
/// one side to the other while they play, with their switches and dials moving, at the rates
/// that oversample 4 and 2 times
#[test]
fn test_the_drive_the_wah_and_the_pitch_shifter_allocate_nothing_on_either_side_of_the_amp() {
    use crate::chain::AmpSettings;
    use crate::order::{ChainOrder, Pedal, Zone};
    use crate::pitch::PitchSettings;
    use crate::wah::{WahMode, WahSettings};
    for sample_rate in [44_100.0, 96_000.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("Drive, wah and pitch on both sides", || {
            for block in 0..900 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                let dial = (block % 40) as f32 / 40.0;
                // All three in front, all three behind, and the three apart, in turn
                let zones = match block / 100 % 3 {
                    0 => [Zone::Front; 3],
                    1 => [Zone::Back; 3],
                    _ => [Zone::Back, Zone::Front, Zone::Back],
                };
                let order = ChainOrder::default()
                    .moved(Pedal::Drive, zones[0], 0)
                    .moved(Pedal::Wah, zones[1], 0)
                    .moved(Pedal::Pitch, zones[2], 0);
                let settings = AmpSettings {
                    drive_on: block % 7 != 0,
                    drive_gain: dial,
                    drive_tone: 1.0 - dial,
                    drive_level: dial,
                    wah: WahSettings {
                        on: block % 5 != 0,
                        mode: if block % 3 == 0 { WahMode::Auto } else { WahMode::Pedal },
                        pedal: dial,
                        bite: 1.0 - dial,
                        sens: dial,
                    },
                    pitch: PitchSettings {
                        on: block % 9 != 0,
                        semitones: (block % 25) as i32 - 12,
                        blend: dial,
                        latency_ms: 2.0 + 48.0 * dial,
                    },
                    layout: order.into(),
                    ..AmpSettings::default()
                };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The chain model, as the audio thread meets it (a host may restore a saved state from there,
/// and every block reads the chain with the dials): the chain read and written while blocks are
/// taken out, put in and moved, its twelve-pedal view worked out, and the memory of the effects
/// taken out
#[test]
fn test_the_chain_model_allocates_nothing() {
    use crate::order::{ChainOrder, Pedal, Zone};
    use crate::slots::{Chain, EffectKind, SlotValues, StoredChain, KNOBS};
    let stored = StoredChain::new();
    let default = ChainOrder::default();
    host::assert_allocates_nothing("The chain model", || {
        for n in 0..3000 {
            let pedal = Pedal::ALL[n % Pedal::ALL.len()];
            let zone = if n % 2 == 0 { Zone::Front } else { Zone::Back };
            stored.put_order(default.moved(pedal, zone, n % 7));
            let mut chain = stored.get();
            let kind = EffectKind::ALL[n % EffectKind::ALL.len()];
            if let Some(block) = chain.blocks().get(n % 5).copied() {
                chain.remove(block.slot);
                stored.memory().remember(block.kind, SlotValues { on: true, knobs: [0.5; KNOBS] });
            }
            let _ = chain.add(kind, zone, n % 4);
            let _ = chain.add_amp();
            let _ = chain.remove_amp(1);
            stored.put(chain);
            assert!(stored.order().front().len() + stored.order().back().len() == Pedal::ALL.len());
            let _ = stored.memory().recall(kind);
            let _ = Chain::from_order(&default).order();
        }
    });
}

/// What a block of `kind` is told at dial position `dial` (0.0 to 1.0), switched on or off
fn block_at(kind: crate::slots::EffectKind, dial: f32, on: bool) -> crate::extras::BlockSettings {
    use crate::comp::CompSettings;
    use crate::delay::DelaySettings;
    use crate::eq::EqSettings;
    use crate::extras::BlockSettings;
    use crate::fuzz::FuzzSettings;
    use crate::mods::chorus::ChorusSettings;
    use crate::mods::flanger::FlangerSettings;
    use crate::mods::phaser::PhaserSettings;
    use crate::mods::tremolo::TremoloSettings;
    use crate::pitch::PitchSettings;
    use crate::reverb::ReverbSettings;
    use crate::slots::EffectKind;
    use crate::wah::{WahMode, WahSettings};
    match kind {
        EffectKind::Gate => BlockSettings::Gate {
            on,
            threshold_db: -80.0 + 60.0 * dial,
            release_ms: 20.0 + 480.0 * dial,
        },
        EffectKind::Drive => {
            BlockSettings::Drive { on, drive: dial, tone: 1.0 - dial, level: dial }
        }
        EffectKind::Comp => {
            BlockSettings::Comp(CompSettings { on, sustain: dial, level: 1.0 - dial })
        }
        EffectKind::Fuzz => BlockSettings::Fuzz(FuzzSettings { on, fuzz: dial, tone: 1.0 - dial }),
        EffectKind::Pitch => BlockSettings::Pitch(PitchSettings {
            on,
            semitones: (dial * 24.0) as i32 - 12,
            blend: dial,
            latency_ms: 2.0 + 48.0 * dial,
        }),
        EffectKind::Wah => BlockSettings::Wah(WahSettings {
            on,
            mode: if dial > 0.5 { WahMode::Auto } else { WahMode::Pedal },
            pedal: dial,
            bite: 1.0 - dial,
            sens: dial,
        }),
        EffectKind::Eq => BlockSettings::Eq(EqSettings {
            on,
            low: dial,
            mid: 1.0 - dial,
            freq: dial,
            high: 1.0 - dial,
        }),
        EffectKind::Chorus => {
            BlockSettings::Chorus(ChorusSettings { on, rate_hz: 0.1 + 5.0 * dial, depth: dial })
        }
        EffectKind::Flanger => {
            BlockSettings::Flanger(FlangerSettings { on, rate_hz: 0.1 + 4.0 * dial, depth: dial })
        }
        EffectKind::Phaser => {
            BlockSettings::Phaser(PhaserSettings { on, rate_hz: 0.1 + 7.0 * dial, depth: dial })
        }
        EffectKind::Tremolo => {
            BlockSettings::Tremolo(TremoloSettings { on, rate_hz: 0.5 + 11.0 * dial, depth: dial })
        }
        EffectKind::Delay => BlockSettings::Delay(DelaySettings {
            on,
            time_ms: 20.0 + 900.0 * dial,
            feedback: 0.9 * dial,
            mix: 1.0 - dial,
        }),
        EffectKind::Reverb => {
            BlockSettings::Reverb(ReverbSettings { on, decay_s: 0.3 + 5.0 * dial, mix: dial })
        }
    }
}

/// Every effect as a block that plays from its slot (#268), alone and as a pair of the same
/// kind, in front of the amp and behind the cabinet, put in and taken out and moved across the
/// amp while it plays, its switch and dials moving, at the rates that oversample 4 and 2 times.
/// The last line holds all sixteen blocks at once
#[test]
fn test_effects_on_slots_allocate_nothing() {
    use crate::chain::AmpSettings;
    use crate::extras::BlockSettings;
    use crate::layout::Layout;
    use crate::slots::{Chain, EffectKind, MAX_SLOTS};
    let line = |ids: &[&str]| Layout::of(&Chain::from_ids(ids.iter().copied()));
    // What stands where, for each stretch of blocks: the layout and the kinds on slots 13 to 16
    let mut stretches: Vec<(Layout, [Option<EffectKind>; 4])> = Vec::new();
    for kind in EffectKind::ALL {
        let id = kind.id();
        let (a, b) = (format!("{id}@13"), format!("{id}@14"));
        let lines: [Vec<&str>; 4] = [
            vec!["amps=1", &a, "amp"],
            vec!["amps=1", "amp", &a],
            vec!["amps=1", &a, "amp", &b],
            vec!["amps=1", &b, "amp", &a],
        ];
        for ids in lines {
            let layout = line(&ids);
            if layout.voices().is_empty() {
                continue;
            }
            let mut kinds = [None; 4];
            kinds[0] = Some(kind);
            kinds[1] = Some(kind);
            stretches.push((layout, kinds));
        }
    }
    // Everything at once: the twelve pedals and four more
    stretches.push((
        line(&[
            "amps=1",
            "gate@13",
            "pitch@10",
            "wah@12",
            "fuzz@9",
            "comp@1",
            "drive@2",
            "phaser@4",
            "amp",
            "chorus@3",
            "flanger@5",
            "tremolo@6",
            "eq@11",
            "delay@7",
            "reverb@8",
            "delay@14",
            "reverb@15",
            "wah@16",
        ]),
        [
            Some(EffectKind::Gate),
            Some(EffectKind::Delay),
            Some(EffectKind::Reverb),
            Some(EffectKind::Wah),
        ],
    ));
    for sample_rate in [44_100.0, 96_000.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("Effects on slots", || {
            for block in 0..stretches.len() * 24 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                let dial = (block % 40) as f32 / 40.0;
                let (layout, kinds) = &stretches[block / 24];
                let mut slots = [BlockSettings::None; MAX_SLOTS];
                for (n, kind) in kinds.iter().enumerate() {
                    if let Some(kind) = *kind {
                        slots[12 + n] = block_at(kind, dial, block % (7 + n) != 0);
                    }
                }
                let settings = AmpSettings {
                    delay: crate::delay::DelaySettings { on: block % 5 != 0, ..Default::default() },
                    layout: *layout,
                    slots,
                    ..AmpSettings::default()
                };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}
