namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// A key centre: a tonic pitch class plus the scale built on it. Knows which pitch classes
/// belong to it, which degree each one is, and whether it is conventionally spelled with flats.
/// </summary>
/// <remarks>
/// Spelling here is a per-key sharp-or-flat *preference* applied to a flat 12-name table,
/// not true letter-sequence spelling. F# major therefore renders its 4th as "B" and never
/// as "E#", and Gb major is spelled with flats throughout. That is correct for every note
/// name this app actually displays, and it avoids a double-accidental engine — so this is
/// deliberate, not an oversight waiting to be fixed.
/// </remarks>
public readonly record struct MusicKey(int TonicPitchClass, ScaleType Scale)
{
    /// <summary>Major keys conventionally written with flats: F, Bb, Eb, Ab, Db, Gb.</summary>
    private static readonly int[] FlatMajorTonics = { 5, 10, 3, 8, 1, 6 };

    /// <summary>Tonic reduced to 0-11, so callers can't poison the lookups with a stray value.</summary>
    public int Tonic => Wrap(TonicPitchClass);

    /// <summary>Semitones above the tonic of each scale note, ascending from 0.</summary>
    public IReadOnlyList<int> Steps => Scale.Steps();

    /// <summary>
    /// Seven notes, one per letter — the scales Roman numerals and diatonic chords are
    /// defined on. False for the pentatonics and blues.
    /// </summary>
    public bool IsHeptatonic => Steps.Count == 7;

    /// <summary>12-bit set of the pitch classes in this key, bit 0 = C.</summary>
    public int PitchClassMask
    {
        get
        {
            int mask = 0;
            foreach (int step in Steps)
                mask |= 1 << ((Tonic + step) % 12);
            return mask;
        }
    }

    public bool Contains(int pitchClass) => (PitchClassMask & (1 << Wrap(pitchClass))) != 0;

    public bool IsTonic(int pitchClass) => Wrap(pitchClass) == Tonic;

    /// <summary>
    /// The 1-based scale degree of a pitch class (tonic = 1), or null when it is not in the
    /// scale. Counts this scale's own notes, so the pentatonics run 1-5 and blues 1-6.
    /// </summary>
    public int? DegreeOf(int pitchClass)
    {
        int interval = Wrap(pitchClass - Tonic);
        var steps = Steps;
        for (int i = 0; i < steps.Count; i++)
        {
            if (steps[i] == interval)
                return i + 1;
        }
        return null;
    }

    /// <summary>
    /// The major key with the same key signature: C for D Dorian and for A minor, Bb for
    /// G harmonic minor, G for E blues.
    /// </summary>
    public int SignatureMajorTonic => SignatureMajorTonicFor(Tonic, Scale);

    public bool UsesFlats => UsesFlatsFor(Tonic, Scale);

    /// <summary>How this key's own tonic is spelled, e.g. "Bb" for Bb minor.</summary>
    public string TonicName => MusicNaming.PitchClassName(Tonic, UsesFlats);

    /// <summary>
    /// "D Dorian", "A Harmonic Minor", "E Blues". Natural minor is just "A Minor", as
    /// musicians name the key; the picker spells it out only because harmonic and melodic
    /// minor sit beside it.
    /// </summary>
    public string DisplayName =>
        $"{TonicName} {(Scale == ScaleType.NaturalMinor ? "Minor" : Scale.DisplayName())}";

    public static int SignatureMajorTonicFor(int tonicPitchClass, ScaleType scale) =>
        Wrap(tonicPitchClass + scale.SignatureMajorOffset());

    /// <summary>Flats exactly when the key signature's major is a flat key.</summary>
    /// <remarks>
    /// Pitch class 6 is both F# and Gb major, and Gb wins, as it does for the major key itself.
    /// The one exception is B Lydian: under Gb its tonic would be Cb, which the flat name table
    /// can only print as "B", leaving a sharp-looking tonic among flat-named notes. Its parent
    /// is F# major, so it reads with sharps.
    /// </remarks>
    public static bool UsesFlatsFor(int tonicPitchClass, ScaleType scale)
    {
        int signature = SignatureMajorTonicFor(tonicPitchClass, scale);
        if (signature == 6 && Wrap(tonicPitchClass) == 11)
            return false;
        return Array.IndexOf(FlatMajorTonics, signature) >= 0;
    }

    /// <summary>
    /// How a root reads when it is the tonic of a key on the given scale — Eb minor rather
    /// than D# minor. Used to label the root dropdown, which re-spells when the scale changes.
    /// </summary>
    public static string RootName(int pitchClass, ScaleType scale) =>
        MusicNaming.PitchClassName(pitchClass, UsesFlatsFor(pitchClass, scale));

    private static int Wrap(int pitchClass) => ((pitchClass % 12) + 12) % 12;
}
