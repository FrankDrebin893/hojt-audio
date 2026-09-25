using System.Diagnostics;
using PianoMidiVisualizationApp.Services.Playback;

namespace PianoMidiVisualizationApp.Services.Recording;

/// <summary>
/// Captures live note-ons and note-offs into <see cref="TimedNote"/>s.
///
/// <para>Recording starts at the first note, not when <see cref="Arm"/> is called, so a take
/// never opens with the silence of reaching from the mouse to the keys. It ends at the last
/// note-off: a take runs exactly from the first key struck to the last key released, which
/// is also a natural loop length.</para>
///
/// <para>Timestamps are <see cref="Stopwatch.GetTimestamp"/> values taken by the caller on the
/// MIDI callback thread, before anything is marshalled to the UI. Every member is thread-safe.</para>
/// </summary>
public sealed class NoteRecorder
{
    private readonly object _sync = new();
    private readonly List<TimedNote> _notes = new();

    /// <summary>Per key: the timestamp and velocity of the note being held, if any.</summary>
    private readonly (long Timestamp, int Velocity)?[] _open = new (long, int)?[128];

    private bool _isArmed;
    private long? _firstNoteTimestamp;

    /// <summary>Between <see cref="Arm"/> and <see cref="Stop"/>, whether or not a note has come yet.</summary>
    public bool IsArmed
    {
        get { lock (_sync) return _isArmed; }
    }

    /// <summary>Whether the first note has arrived, i.e. the take's clock is running.</summary>
    public bool HasStarted
    {
        get { lock (_sync) return _firstNoteTimestamp.HasValue; }
    }

    /// <summary>Time since the first note, or zero while still waiting for it.</summary>
    public TimeSpan Elapsed
    {
        get
        {
            lock (_sync)
                return _firstNoteTimestamp is { } first
                    ? Stopwatch.GetElapsedTime(first)
                    : TimeSpan.Zero;
        }
    }

    /// <summary>Waits for the first note. Discards anything left from an unfinished take.</summary>
    public void Arm()
    {
        lock (_sync)
        {
            ResetLocked();
            _isArmed = true;
        }
    }

    public void NoteOn(int note, int velocity, long timestamp)
    {
        if (note is < 0 or > 127) return;

        lock (_sync)
        {
            if (!_isArmed) return;
            _firstNoteTimestamp ??= timestamp;

            // A second note-on without a note-off in between (some controllers send one)
            // ends the first note here rather than leaving it open-ended.
            CloseLocked(note, timestamp);
            _open[note] = (timestamp, velocity);
        }
    }

    public void NoteOff(int note, long timestamp)
    {
        if (note is < 0 or > 127) return;

        lock (_sync)
        {
            // A note-off for a key held down before recording began has no note-on to pair with.
            if (_isArmed) CloseLocked(note, timestamp);
        }
    }

    /// <summary>
    /// Ends the take, closing any keys still held at <paramref name="timestamp"/>. Returns the
    /// notes in start order, with times relative to the first note; empty if none arrived.
    /// </summary>
    public IReadOnlyList<TimedNote> Stop(long timestamp)
    {
        lock (_sync)
        {
            for (int note = 0; note < _open.Length; note++)
                CloseLocked(note, timestamp);

            var take = _notes
                .OrderBy(n => n.Start)
                .ThenBy(n => n.Note)
                .ToList();

            ResetLocked();
            return take;
        }
    }

    /// <summary>Stops without producing a take.</summary>
    public void Cancel()
    {
        lock (_sync) ResetLocked();
    }

    private void CloseLocked(int note, long timestamp)
    {
        if (_open[note] is not { } open || _firstNoteTimestamp is not { } first) return;
        _open[note] = null;

        var start = Stopwatch.GetElapsedTime(first, open.Timestamp);
        var end = Stopwatch.GetElapsedTime(first, Math.Max(timestamp, open.Timestamp));
        _notes.Add(new TimedNote(start, end - start, note, open.Velocity));
    }

    private void ResetLocked()
    {
        _isArmed = false;
        _firstNoteTimestamp = null;
        _notes.Clear();
        Array.Clear(_open);
    }
}
