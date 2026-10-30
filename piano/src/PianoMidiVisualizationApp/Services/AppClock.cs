using System.Diagnostics;

namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// The clocks the view models read: <see cref="GetTimestamp"/> for note timing (recording,
/// playback, learned controls) and <see cref="Now"/> for what a player reads (the MIDI log's
/// times, a take's time of day). The app runs on <see cref="Real"/>; the screenshot harness
/// passes one it steps by hand, so its renders come out the same on every run.
/// </summary>
public class AppClock
{
    /// <summary>The wall clock and <see cref="Stopwatch"/>.</summary>
    public static AppClock Real { get; } = new();

    /// <summary>A <see cref="Stopwatch.GetTimestamp"/> value, in <see cref="Stopwatch.Frequency"/> ticks.</summary>
    public virtual long GetTimestamp() => Stopwatch.GetTimestamp();

    /// <summary>The local time of day.</summary>
    public virtual DateTime Now => DateTime.Now;
}
