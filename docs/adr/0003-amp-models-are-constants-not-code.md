# Amp models differ only by constants

Every amp model in the Guitar Amp is one `AmpModel` of constants in `plugins/amp/src/amp/model.rs`. The DSP never branches on which amp is picked (`match amp`); `plugins/amp/tests/rules.rs` holds this. Both lanes of the stack run the same stages on their own `AmpModel`, and a new amp is a new set of constants plus its look.

The written record does not say why this was chosen over amp-specific code paths; what it gives is one place to tune a sound, and a second lane that costs only what the first does. If the reason was something more, add it here.

## Consequences

A sound that constants cannot reach needs a new constant in the shared stages, not a special case.
