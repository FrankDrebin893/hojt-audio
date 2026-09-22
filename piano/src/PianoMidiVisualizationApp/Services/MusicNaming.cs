using System.Text.RegularExpressions;

namespace PianoMidiVisualizationApp.Services;

/// <summary>Single source of truth for turning MIDI note numbers into names.</summary>
public static partial class MusicNaming
{
    private static readonly string[] PitchClassNames =
        { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" };

    private static readonly string[] FlatPitchClassNames =
        { "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B" };

    /// <summary>Pitch class only, e.g. 60 -> "C".</summary>
    public static string PitchClass(int midiNote) => PitchClassNames[PitchClassOf(midiNote)];

    /// <summary>Pitch class only, spelled with flats when the active key calls for it.</summary>
    public static string PitchClass(int midiNote, bool useFlats) =>
        PitchClassName(PitchClassOf(midiNote), useFlats);

    /// <summary>Names a pitch class index directly (0-11), rather than a MIDI note.</summary>
    public static string PitchClassName(int pitchClass, bool useFlats)
    {
        int pc = ((pitchClass % 12) + 12) % 12;
        return useFlats ? FlatPitchClassNames[pc] : PitchClassNames[pc];
    }

    /// <summary>Name with octave, e.g. 60 -> "C4".</summary>
    public static string WithOctave(int midiNote) =>
        $"{PitchClass(midiNote)}{(midiNote / 12) - 1}";

    /// <summary>Name with octave, spelled with flats when the active key calls for it.</summary>
    public static string WithOctave(int midiNote, bool useFlats) =>
        $"{PitchClass(midiNote, useFlats)}{(midiNote / 12) - 1}";

    /// <summary>Pitch class index 0-11, C = 0.</summary>
    public static int PitchClassOf(int midiNote) => ((midiNote % 12) + 12) % 12;

    /// <summary>
    /// Re-spells the note letters in a chord name to match the active key, e.g.
    /// "D#m7/A#" -> "Ebm7/Bb". Only the root and the slash bass are letters; everything
    /// between them is quality text ("m7", "maj9") and is left untouched.
    /// </summary>
    public static string Respell(string chordName, bool useFlats)
    {
        if (string.IsNullOrEmpty(chordName)) return chordName;

        return ChordRootRegex().Replace(chordName, match =>
        {
            int? pc = PitchClassOfName(match.Groups["letter"].Value, match.Groups["accidental"].Value);
            return pc is { } p
                ? match.Groups["lead"].Value + PitchClassName(p, useFlats)
                : match.Value;
        });
    }

    private static int? PitchClassOfName(string letter, string accidental)
    {
        int pc = letter switch
        {
            "C" => 0, "D" => 2, "E" => 4, "F" => 5, "G" => 7, "A" => 9, "B" => 11, _ => -1
        };
        if (pc < 0) return null;

        if (accidental == "#") pc++;
        else if (accidental == "b") pc--;

        return ((pc % 12) + 12) % 12;
    }

    /// <summary>
    /// A note letter at the very start of the name or straight after a slash — the only two
    /// places a chord name carries a root. Matching anywhere else would mangle quality text.
    /// </summary>
    [GeneratedRegex(@"(?<lead>^|/)(?<letter>[A-G])(?<accidental>[#b]?)")]
    private static partial Regex ChordRootRegex();
}
