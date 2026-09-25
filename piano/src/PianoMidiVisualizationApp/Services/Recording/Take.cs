using PianoMidiVisualizationApp.Services.Playback;

namespace PianoMidiVisualizationApp.Services.Recording;

/// <summary>One finished recording. Immutable, so the player and the exporter can share it.</summary>
public sealed class Take
{
    public Take(int number, IReadOnlyList<TimedNote> notes, DateTime createdAt)
    {
        Number = number;
        Notes = notes;
        CreatedAt = createdAt;
        Length = notes.Count > 0 ? notes.Max(n => n.End) : TimeSpan.Zero;
        LowestNote = notes.Count > 0 ? notes.Min(n => n.Note) : 60;
        HighestNote = notes.Count > 0 ? notes.Max(n => n.Note) : 60;
    }

    /// <summary>Counts up for the session, so names stay unique after older takes are dropped.</summary>
    public int Number { get; }

    public string Name => $"Take {Number}";

    public IReadOnlyList<TimedNote> Notes { get; }

    /// <summary>From the first note-on to the last note-off.</summary>
    public TimeSpan Length { get; }

    public DateTime CreatedAt { get; }

    public int LowestNote { get; }
    public int HighestNote { get; }

    public string LengthText => FormatTime(Length);

    public string CreatedText => CreatedAt.ToString("HH:mm");

    public string Summary => $"{Notes.Count} note{(Notes.Count == 1 ? "" : "s")}";

    /// <summary>m:ss.t — tenths are enough to find a loop point by ear.</summary>
    public static string FormatTime(TimeSpan time)
    {
        if (time < TimeSpan.Zero) time = TimeSpan.Zero;
        return $"{(int)time.TotalMinutes}:{time.Seconds:00}.{time.Milliseconds / 100}";
    }
}
