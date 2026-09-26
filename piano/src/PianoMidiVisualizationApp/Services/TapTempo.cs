namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// Turns taps into a tempo: the average interval across the last few taps. A pause longer
/// than the slowest supported beat starts a fresh count, so an old tempo never leaks in.
/// </summary>
public sealed class TapTempo
{
    /// <summary>Four intervals: enough to smooth out one sloppy tap, few enough to follow a change.</summary>
    public const int MaxTaps = 5;

    /// <summary>Comfortably past 2 s, the beat at the 30 BPM minimum, so tapping that slowly still counts.</summary>
    public static readonly TimeSpan ResetAfter = TimeSpan.FromSeconds(2.5);

    private readonly List<TimeSpan> _taps = new();

    /// <summary>
    /// Records a tap at <paramref name="now"/> on any monotonic clock. Returns the tempo in
    /// BPM, unclamped, or null until there are two taps to measure between.
    /// </summary>
    public int? Tap(TimeSpan now)
    {
        if (_taps.Count > 0 && (now - _taps[^1] > ResetAfter || now <= _taps[^1]))
            _taps.Clear();

        _taps.Add(now);
        if (_taps.Count > MaxTaps)
            _taps.RemoveAt(0);

        if (_taps.Count < 2)
            return null;

        double secondsPerBeat = (_taps[^1] - _taps[0]).TotalSeconds / (_taps.Count - 1);
        return (int)Math.Round(60 / secondsPerBeat);
    }
}
