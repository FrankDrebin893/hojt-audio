namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// A tone shaping laid over whichever soundbank is loaded. It is not a sample set: no piano
/// sound is bundled, so a voicing only changes how the user's own soundbank is voiced.
/// </summary>
public enum PianoVoicing
{
    /// <summary>The soundbank as it comes, untouched.</summary>
    Natural,

    /// <summary>
    /// A bright, tight, forward piano that stays clear in a band mix: less low-mid mud, more
    /// attack and air, and a gentle compressor that evens out the playing. The character of a
    /// stage piano's grand, not of an acoustic one.
    /// </summary>
    StageGrand,
}

public static class PianoVoicingInfo
{
    /// <summary>The names the settings panel lists, in order.</summary>
    public static readonly IReadOnlyList<PianoVoicing> All = new[] { PianoVoicing.Natural, PianoVoicing.StageGrand };

    public static string DisplayName(PianoVoicing voicing) => voicing switch
    {
        PianoVoicing.StageGrand => "Stage grand",
        _ => "Natural",
    };

    /// <summary>
    /// Reads a saved name. Anything that is not the name of a defined voicing, including a
    /// number or an empty value, is Natural, so a hand-edited file never throws.
    /// </summary>
    public static PianoVoicing Parse(string? saved) =>
        Enum.TryParse<PianoVoicing>(saved, out var v) && Enum.IsDefined(v) && !int.TryParse(saved, out _)
            ? v
            : PianoVoicing.Natural;
}
