using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.Midi;

public enum MidiTriggerKind { Note, ControlChange, ProgramChange }

/// <summary>
/// A control on the MIDI keyboard that fires an app action: a key or pad (a note), a button,
/// knob or pedal (a controller), or a program-change button. The channel is part of its
/// identity, so a pad on channel 10 is a different control from the key with the same number
/// on channel 1. Channel and number are 0-based, as the MIDI message carries them.
/// </summary>
public readonly record struct MidiTrigger(MidiTriggerKind Kind, int Channel, int Number)
{
    /// <summary>The sustain pedal's controller number.</summary>
    public const int SustainPedal = 64;

    /// <summary>A controller counts as pressed from this value up, as the sustain pedal does.</summary>
    public const int PressedThreshold = 64;

    public bool IsValid => Channel is >= 0 and <= 15 && Number is >= 0 and <= 127;

    /// <summary>"C1", "Sustain pedal", "CC 20" or "Program 5", plus " · ch 10" off channel 1.</summary>
    public string DisplayName
    {
        get
        {
            string name = Kind switch
            {
                MidiTriggerKind.Note => MusicNaming.WithOctave(Number),
                MidiTriggerKind.ControlChange => ControllerName(Number),
                _ => $"Program {Number + 1}"
            };
            return Channel == 0 ? name : $"{name} · ch {Channel + 1}";
        }
    }

    /// <summary>
    /// What mapping this control takes away from playing, or null if nothing: a key stops
    /// sounding its note, and the sustain pedal stops sustaining.
    /// </summary>
    public string? SideEffect => Kind switch
    {
        MidiTriggerKind.Note => $"{MusicNaming.WithOctave(Number)} won't sound while it's mapped.",
        MidiTriggerKind.ControlChange when Number == SustainPedal => "The pedal won't sustain while it's mapped.",
        _ => null
    };

    private static string ControllerName(int controller) => controller switch
    {
        1 => "Mod wheel",
        7 => "Volume slider",
        11 => "Expression",
        SustainPedal => "Sustain pedal",
        66 => "Sostenuto pedal",
        67 => "Soft pedal",
        _ => $"CC {controller}"
    };
}
