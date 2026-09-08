namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// What the readout shows for the currently held notes. <see cref="Notes"/> and
/// <see cref="Intervals"/> are index-aligned token-for-token and column-padded, so
/// rendering them in a monospace face lines each interval up under its note.
/// </summary>
/// <remarks>
/// A record struct so value equality suppresses redundant PropertyChanged when the
/// analysis is unchanged, with no allocation per MIDI event.
/// </remarks>
public readonly record struct ChordAnalysis(string Name, string Notes, string Intervals)
{
    public static readonly ChordAnalysis Empty = new("", "", "");

    public bool IsEmpty => string.IsNullOrEmpty(Name);
}
