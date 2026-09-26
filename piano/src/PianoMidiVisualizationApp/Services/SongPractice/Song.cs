namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>What happens to a part's notes during practice.</summary>
public enum PartRole
{
    /// <summary>Falls toward the keys for you to play; never sounded by the app.</summary>
    YouPlay,

    /// <summary>Played by the app as accompaniment.</summary>
    AutoPlay,

    /// <summary>Neither drawn nor played.</summary>
    Mute
}

/// <summary>
/// One strand of a song: a MIDI track, one channel of a multi-channel track, or one hand of
/// a piano track that was split at middle C.
/// </summary>
/// <param name="ColorIndex">0 = right hand, 1 = left hand, 2+ = any other part.</param>
public sealed record SongPart(
    int Index,
    string Name,
    string Instrument,
    int NoteCount,
    bool IsDrums,
    bool IsPiano,
    PartRole DefaultRole,
    int ColorIndex);

/// <summary>A note in absolute song time at 100% speed, already resolved through the tempo map.</summary>
public readonly record struct SongNote(int Part, int Note, int Velocity, TimeSpan Start, TimeSpan Duration)
{
    public TimeSpan End => Start + Duration;
}

/// <summary>A loaded song: its parts, every note in start order, and where its bars fall.</summary>
public sealed class Song
{
    public string Title { get; }
    public IReadOnlyList<SongPart> Parts { get; }

    /// <summary>Sorted by start time, then pitch.</summary>
    public IReadOnlyList<SongNote> Notes { get; }

    /// <summary>
    /// Start time of every bar, plus the end of the last bar as a final boundary, so bar
    /// <c>n</c> (1-based) spans <c>BarStarts[n-1]</c> to <c>BarStarts[n]</c>.
    /// </summary>
    public IReadOnlyList<TimeSpan> BarStarts { get; }

    /// <summary>When the last note ends.</summary>
    public TimeSpan Duration { get; }

    /// <summary>The tempo at the start of the song, for display only; timing uses the full tempo map.</summary>
    public double InitialBpm { get; }

    public int BarCount => BarStarts.Count - 1;

    public Song(string title, IReadOnlyList<SongPart> parts, IReadOnlyList<SongNote> notes,
                IReadOnlyList<TimeSpan> barStarts, double initialBpm)
    {
        if (barStarts.Count < 2)
            throw new ArgumentException("A song needs at least one bar.", nameof(barStarts));

        Title = title;
        Parts = parts;
        Notes = notes;
        BarStarts = barStarts;
        InitialBpm = initialBpm;
        Duration = notes.Count == 0 ? TimeSpan.Zero : notes.Max(n => n.End);
    }

    /// <summary>The 1-based bar containing <paramref name="time"/>, clamped to the song.</summary>
    public int BarAt(TimeSpan time)
    {
        if (time <= BarStarts[0]) return 1;

        // Binary search for the last bar start at or before the time.
        int lo = 0, hi = BarStarts.Count - 2;
        while (lo < hi)
        {
            int mid = (lo + hi + 1) / 2;
            if (BarStarts[mid] <= time) lo = mid;
            else hi = mid - 1;
        }
        return lo + 1;
    }

    /// <summary>
    /// Start time of a 1-based bar. <c>BarCount + 1</c> gives the end of the last bar, which
    /// is what an A–B loop ending on the final bar needs.
    /// </summary>
    public TimeSpan BarStart(int bar) => BarStarts[Math.Clamp(bar, 1, BarStarts.Count) - 1];
}
