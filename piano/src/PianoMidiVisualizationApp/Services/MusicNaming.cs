namespace PianoMidiVisualizationApp.Services;

/// <summary>Single source of truth for turning MIDI note numbers into names.</summary>
public static class MusicNaming
{
    private static readonly string[] PitchClassNames =
        { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" };

    /// <summary>Pitch class only, e.g. 60 -> "C".</summary>
    public static string PitchClass(int midiNote) => PitchClassNames[PitchClassOf(midiNote)];

    /// <summary>Name with octave, e.g. 60 -> "C4".</summary>
    public static string WithOctave(int midiNote) =>
        $"{PitchClass(midiNote)}{(midiNote / 12) - 1}";

    /// <summary>Pitch class index 0-11, C = 0.</summary>
    public static int PitchClassOf(int midiNote) => ((midiNote % 12) + 12) % 12;
}
