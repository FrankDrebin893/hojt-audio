namespace PianoMidiVisualizationApp.Services.Playback;

/// <summary>
/// One note of a sequence: when it starts, how long it sounds, which MIDI note and how hard.
/// Times are real time from the start of the sequence, not musical beats, so a recorded take
/// and a generated chord progression can share the same player.
/// </summary>
public record TimedNote(TimeSpan Start, TimeSpan Duration, int Note, int Velocity)
{
    public TimeSpan End => Start + Duration;
}
