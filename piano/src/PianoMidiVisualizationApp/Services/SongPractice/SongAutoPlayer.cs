using PianoMidiVisualizationApp.Services.Playback;

namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>
/// Sounds the auto-play parts of a song, one segment at a time, on
/// <see cref="NoteSequencePlayer"/>s.
/// </summary>
/// <remarks>
/// <para>Wait mode holds the song at each of your chords, so the accompaniment can only be
/// handed out up to the next place the song might stop, and extended when it moves on. A
/// player cannot be extended while it plays (Load stops it and cuts every sounding note), so
/// each segment gets its own player from a small pool. Earlier segments keep playing out
/// their note-offs undisturbed while the next one starts, which is what keeps a held
/// accompaniment note from being cut short whenever you play a chord slightly early.</para>
///
/// <para>Every segment is loaded relative to the moment it is scheduled, from the song clock's
/// position at that instant, so all players share the song clock's timeline.</para>
///
/// <para>Call on the UI thread. The note callbacks run on the players' timing threads.</para>
/// </remarks>
public sealed class SongAutoPlayer : IDisposable
{
    /// <summary>
    /// Same-key notes are trimmed to end this long before the next one starts. Two segments'
    /// players run on separate threads, so a note-off and a note-on due at the same instant
    /// could otherwise arrive in either order and cut the new note dead.
    /// </summary>
    private static readonly TimeSpan SameKeyGap = TimeSpan.FromMilliseconds(5);

    private static readonly TimeSpan MinimumDuration = TimeSpan.FromMilliseconds(10);

    private readonly Action<int, int> _noteOn;
    private readonly Action<int> _noteOff;
    private readonly List<NoteSequencePlayer> _players = new();
    private SongNote[] _notes = [];
    private double _speed = 1.0;

    /// <param name="noteOn">The host's app-note path, e.g. <c>MainViewModel.PlayNoteOn</c>.</param>
    public SongAutoPlayer(Action<int, int> noteOn, Action<int> noteOff)
    {
        _noteOn = noteOn;
        _noteOff = noteOff;
    }

    /// <summary>The notes to play from now on, in any order. Stops whatever is playing.</summary>
    public void SetNotes(IEnumerable<SongNote> notes)
    {
        StopAll();
        _notes = TrimSameKeyOverlaps(notes);
    }

    /// <summary>Song seconds per real second; applied to every segment still playing.</summary>
    public double Speed
    {
        get => _speed;
        set
        {
            _speed = value;
            foreach (var player in _players)
            {
                if (player.IsPlaying) player.Speed = value;
            }
        }
    }

    /// <summary>
    /// Plays every note starting in [<paramref name="from"/>, <paramref name="until"/>), timed
    /// so that song time <paramref name="position"/> is now.
    /// </summary>
    public void Schedule(TimeSpan from, TimeSpan until, TimeSpan position)
    {
        if (until <= from) return;

        int first = FirstStartingAtOrAfter(from);
        var segment = new List<TimedNote>();
        for (int i = first; i < _notes.Length && _notes[i].Start < until; i++)
        {
            var n = _notes[i];
            var start = n.Start - position;
            segment.Add(new TimedNote(start < TimeSpan.Zero ? TimeSpan.Zero : start, n.Duration, n.Note, n.Velocity));
        }
        if (segment.Count == 0) return;

        var player = _players.FirstOrDefault(p => !p.IsPlaying);
        if (player == null)
        {
            player = new NoteSequencePlayer(_noteOn, _noteOff);
            _players.Add(player);
        }

        player.Load(segment);
        player.IsLooping = false;
        player.Speed = _speed;
        player.Play(TimeSpan.Zero);
    }

    /// <summary>Stops every segment and releases every note it was sounding.</summary>
    public void StopAll()
    {
        foreach (var player in _players)
            player.Stop();
    }

    public void Dispose()
    {
        foreach (var player in _players)
            player.Dispose();
        _players.Clear();
    }

    private int FirstStartingAtOrAfter(TimeSpan time)
    {
        int lo = 0, hi = _notes.Length;
        while (lo < hi)
        {
            int mid = (lo + hi) >>> 1;
            if (_notes[mid].Start < time) lo = mid + 1;
            else hi = mid;
        }
        return lo;
    }

    /// <summary>
    /// Sorts by start and shortens any note that would still be sounding when the same key is
    /// struck again, by any part. A piano cannot sound one key twice anyway.
    /// </summary>
    private static SongNote[] TrimSameKeyOverlaps(IEnumerable<SongNote> notes)
    {
        var sorted = notes.OrderBy(n => n.Start).ThenBy(n => n.Note).ToArray();
        var nextStartOnKey = new TimeSpan?[128];

        for (int i = sorted.Length - 1; i >= 0; i--)
        {
            var n = sorted[i];
            if (n.Note is < 0 or > 127) continue;

            if (nextStartOnKey[n.Note] is { } next && n.End > next - SameKeyGap)
            {
                var duration = next - SameKeyGap - n.Start;
                sorted[i] = n with { Duration = duration < MinimumDuration ? MinimumDuration : duration };
            }
            nextStartOnKey[n.Note] = n.Start;
        }

        return sorted;
    }
}
