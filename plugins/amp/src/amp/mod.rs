pub mod model;
pub mod poweramp;
pub mod preamp;
pub mod tonestack;

use model::AmpTopology;
use poweramp::PowerAmp;
use preamp::Preamp;
use tonestack::ToneStack;

/// Runs one oversampled block through the amp's stages in the order its topology asks for.
pub fn process_amp_block(
    topology: AmpTopology,
    preamp: &mut Preamp,
    tone: &mut ToneStack,
    power: &mut PowerAmp,
    block: &mut [f32],
) {
    match topology {
        AmpTopology::CleanAcoustic => {
            tone.process(block);
            preamp.process(block);
            power.process(block);
        }
        AmpTopology::VintageCrunch | AmpTopology::ModernHighGain => {
            preamp.process(block);
            tone.process(block);
            power.process(block);
        }
    }
}
