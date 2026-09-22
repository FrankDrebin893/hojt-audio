namespace PianoMidiVisualizationApp.Services;

public enum KeyQuality { Major, Minor }

/// <summary>
/// A key signature: a tonic pitch class plus major or natural minor. Knows which pitch
/// classes belong to it and whether it is conventionally spelled with flats.
/// </summary>
/// <remarks>
/// Spelling here is a per-key sharp-or-flat *preference* applied to a flat 12-name table,
/// not true letter-sequence spelling. F# major therefore renders its 4th as "B" and never
/// as "E#", and Gb major is spelled with flats throughout. That is correct for every note
/// name this app actually displays, and it avoids a double-accidental engine — so this is
/// deliberate, not an oversight waiting to be fixed.
/// </remarks>
public readonly record struct MusicKey(int TonicPitchClass, KeyQuality Quality)
{
    private static readonly int[] MajorSteps = { 0, 2, 4, 5, 7, 9, 11 };
    private static readonly int[] MinorSteps = { 0, 2, 3, 5, 7, 8, 10 };

    /// <summary>Major keys conventionally written with flats: F, Bb, Eb, Ab, Db, Gb.</summary>
    private static readonly int[] FlatMajorTonics = { 5, 10, 3, 8, 1, 6 };

    /// <summary>Minor keys conventionally written with flats: D, G, C, F, Bb, Eb.</summary>
    private static readonly int[] FlatMinorTonics = { 2, 7, 0, 5, 10, 3 };

    /// <summary>Tonic reduced to 0-11, so callers can't poison the lookups with a stray value.</summary>
    public int Tonic => ((TonicPitchClass % 12) + 12) % 12;

    /// <summary>12-bit set of the pitch classes in this key, bit 0 = C.</summary>
    public int PitchClassMask
    {
        get
        {
            int mask = 0;
            foreach (int step in Quality == KeyQuality.Major ? MajorSteps : MinorSteps)
                mask |= 1 << ((Tonic + step) % 12);
            return mask;
        }
    }

    public bool Contains(int pitchClass) => (PitchClassMask & (1 << (((pitchClass % 12) + 12) % 12))) != 0;

    public bool IsTonic(int pitchClass) => (((pitchClass % 12) + 12) % 12) == Tonic;

    public bool UsesFlats => UsesFlatsFor(Tonic, Quality);

    /// <summary>How this key's own tonic is spelled, e.g. "Bb" for Bb minor.</summary>
    public string TonicName => MusicNaming.PitchClassName(Tonic, UsesFlats);

    public string DisplayName => $"{TonicName} {(Quality == KeyQuality.Major ? "Major" : "Minor")}";

    public static bool UsesFlatsFor(int tonicPitchClass, KeyQuality quality)
    {
        int pc = ((tonicPitchClass % 12) + 12) % 12;
        var flats = quality == KeyQuality.Major ? FlatMajorTonics : FlatMinorTonics;
        return Array.IndexOf(flats, pc) >= 0;
    }

    /// <summary>
    /// How a root reads when it is the tonic of a key of the given quality — Eb minor rather
    /// than D# minor. Used to label the root dropdown, which re-spells when quality changes.
    /// </summary>
    public static string RootName(int pitchClass, KeyQuality quality) =>
        MusicNaming.PitchClassName(pitchClass, UsesFlatsFor(pitchClass, quality));
}
