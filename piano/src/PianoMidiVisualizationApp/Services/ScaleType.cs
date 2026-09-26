namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// The scales a key can be built on. Persisted by name in <c>AppSettings.KeyScale</c>, so
/// members may be added but never renamed.
/// </summary>
public enum ScaleType
{
    Major,
    NaturalMinor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues
}

public static class ScaleTypeExtensions
{
    // Semitones above the tonic, ascending. Melodic minor is the ascending ("jazz") form.
    private static readonly int[] MajorSteps           = { 0, 2, 4, 5, 7, 9, 11 };
    private static readonly int[] NaturalMinorSteps    = { 0, 2, 3, 5, 7, 8, 10 };
    private static readonly int[] HarmonicMinorSteps   = { 0, 2, 3, 5, 7, 8, 11 };
    private static readonly int[] MelodicMinorSteps    = { 0, 2, 3, 5, 7, 9, 11 };
    private static readonly int[] DorianSteps          = { 0, 2, 3, 5, 7, 9, 10 };
    private static readonly int[] PhrygianSteps        = { 0, 1, 3, 5, 7, 8, 10 };
    private static readonly int[] LydianSteps          = { 0, 2, 4, 6, 7, 9, 11 };
    private static readonly int[] MixolydianSteps      = { 0, 2, 4, 5, 7, 9, 10 };
    private static readonly int[] LocrianSteps         = { 0, 1, 3, 5, 6, 8, 10 };
    private static readonly int[] MajorPentatonicSteps = { 0, 2, 4, 7, 9 };
    private static readonly int[] MinorPentatonicSteps = { 0, 3, 5, 7, 10 };
    private static readonly int[] BluesSteps           = { 0, 3, 5, 6, 7, 10 };

    /// <summary>Semitones above the tonic of each scale note, ascending from 0.</summary>
    public static IReadOnlyList<int> Steps(this ScaleType scale) => scale switch
    {
        ScaleType.Major => MajorSteps,
        ScaleType.NaturalMinor => NaturalMinorSteps,
        ScaleType.HarmonicMinor => HarmonicMinorSteps,
        ScaleType.MelodicMinor => MelodicMinorSteps,
        ScaleType.Dorian => DorianSteps,
        ScaleType.Phrygian => PhrygianSteps,
        ScaleType.Lydian => LydianSteps,
        ScaleType.Mixolydian => MixolydianSteps,
        ScaleType.Locrian => LocrianSteps,
        ScaleType.MajorPentatonic => MajorPentatonicSteps,
        ScaleType.MinorPentatonic => MinorPentatonicSteps,
        ScaleType.Blues => BluesSteps,
        _ => MajorSteps
    };

    /// <summary>
    /// Semitones from the tonic up to the major key that shares this scale's key signature:
    /// modes sit on their parent major, and every minor form borrows the natural minor's
    /// relative major. Harmonic and melodic minor's raised notes are accidentals, not part
    /// of the signature, exactly as they are written in sheet music.
    /// </summary>
    public static int SignatureMajorOffset(this ScaleType scale) => scale switch
    {
        ScaleType.Major => 0,
        ScaleType.NaturalMinor or ScaleType.HarmonicMinor or ScaleType.MelodicMinor => 3,
        ScaleType.Dorian => 10,       // D Dorian -> C
        ScaleType.Phrygian => 8,      // E Phrygian -> C
        ScaleType.Lydian => 7,        // F Lydian -> C
        ScaleType.Mixolydian => 5,    // G Mixolydian -> C
        ScaleType.Locrian => 1,       // B Locrian -> C
        ScaleType.MajorPentatonic => 0,
        ScaleType.MinorPentatonic or ScaleType.Blues => 3,
        _ => 0
    };

    /// <summary>How the scale reads in the picker, e.g. "Harmonic Minor".</summary>
    public static string DisplayName(this ScaleType scale) => scale switch
    {
        ScaleType.NaturalMinor => "Natural Minor",
        ScaleType.HarmonicMinor => "Harmonic Minor",
        ScaleType.MelodicMinor => "Melodic Minor",
        ScaleType.MajorPentatonic => "Major Pentatonic",
        ScaleType.MinorPentatonic => "Minor Pentatonic",
        _ => scale.ToString()
    };
}
