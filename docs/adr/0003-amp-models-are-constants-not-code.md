# Amp models differ by constants and archetype topology

Every amp model in the Guitar Amp is one `AmpModel` of constants in `plugins/amp/src/amp/model.rs`. The DSP never branches on which individual amp is picked (`match amp`); `plugins/amp/tests/rules.rs` holds this. Instead, `AmpModel.topology` selects one of a small fixed set of circuit archetypes (`AmpTopology`: `CleanAcoustic`, `VintageCrunch`, `ModernHighGain`), and both lanes of the stack dispatch on that topology.

The written record does not say why constants-only was originally chosen over amp-specific code paths; what it gives is one place to tune a sound, and a second lane that costs only what the first does. Allowing `AmpTopology` preserves that shared, zero-allocation structure while letting clean, crunch, and modern high-gain amps use physically appropriate signal flow (such as pre- vs. post-gain tone stacks and steeper input tight filters).

## Consequences

A sound that constants and existing archetypes cannot reach needs either a new constant in the shared stages or a new `AmpTopology` variant, never a per-amp special case.
