namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// What the readout shows for the currently held notes. <see cref="Notes"/> and
/// <see cref="Intervals"/> are index-aligned token-for-token and column-padded, so
/// rendering them in a monospace face lines each interval up under its note.
/// </summary>
/// <param name="RootPitchClass">
/// The pitch class (C = 0) that <see cref="Intervals"/> are measured from. Set for every
/// non-empty analysis: the chord root, the note itself for a single note, or the bass for
/// a note list that isn't a recognised chord. Null only for <see cref="Empty"/>.
/// </param>
/// <param name="Voicing">
/// Inversion and voicing, e.g. "1st inversion · close". Empty when there's nothing
/// certain to say (single notes, dyads, unnamed note lists).
/// </param>
/// <remarks>
/// A record struct so value equality suppresses redundant PropertyChanged when the
/// analysis is unchanged, with no allocation per MIDI event.
/// </remarks>
public readonly record struct ChordAnalysis(
    string Name, string Notes, string Intervals, int? RootPitchClass, string Voicing)
{
    public static readonly ChordAnalysis Empty = new("", "", "", null, "");

    /// <summary>
    /// The bass's role behind <see cref="Voicing"/>, for figured bass. <see cref="ChordInversion.None"/>
    /// whenever <see cref="Voicing"/> doesn't name an inversion.
    /// </summary>
    public ChordInversion Inversion { get; init; }

    public bool IsEmpty => string.IsNullOrEmpty(Name);
}
