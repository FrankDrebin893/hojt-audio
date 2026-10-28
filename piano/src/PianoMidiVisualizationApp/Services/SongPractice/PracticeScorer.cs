namespace PianoMidiVisualizationApp.Services.SongPractice;

public enum PracticeMode
{
    /// <summary>The song stops at each chord until you hold all of it.</summary>
    Wait,

    /// <summary>The song keeps going; notes are scored as hit or missed on the fly.</summary>
    PlayAlong
}

/// <summary>How a played note was judged.</summary>
public enum NoteVerdict { Correct, Wrong }

/// <summary>
/// Judges live input against the "you play" chords. Pure logic with no clock of its own: the
/// caller passes the song position in, which keeps it deterministic and testable.
/// </summary>
/// <remarks>
/// Wait mode: a chord is satisfied when every one of its notes is held <em>and</em> was struck
/// since the previous chord was satisfied. The second half is what makes a repeated note have
/// to be played again rather than being satisfied by a key still held from last time.
/// </remarks>
public sealed class PracticeScorer
{
    /// <summary>How far ahead of its time a wait-mode chord can be played and still count.</summary>
    public static readonly TimeSpan EarlyWindow = TimeSpan.FromMilliseconds(400);

    /// <summary>Play-along tolerance either side of a note's start.</summary>
    public static readonly TimeSpan HitWindow = TimeSpan.FromMilliseconds(200);

    private readonly IReadOnlyList<PracticeChord> _chords;
    private readonly Func<int, int> _fold;

    /// <summary>Wait mode: the first chord not yet satisfied.</summary>
    private int _nextChord;

    /// <summary>Play-along: every chord note, flattened in time order, with a hit flag each.</summary>
    private readonly List<Target> _targets = new();
    private int _nextTarget;

    private readonly HashSet<int> _held = new();

    /// <summary>Keys struck since they last counted toward a chord.</summary>
    private readonly HashSet<int> _fresh = new();

    public PracticeMode Mode { get; }
    public int Correct { get; private set; }
    public int Missed { get; private set; }
    public int Wrong { get; private set; }

    /// <param name="fold">Maps a written note onto the drawn keyboard; a folded note may be played at either pitch.</param>
    public PracticeScorer(IReadOnlyList<PracticeChord> chords, PracticeMode mode, Func<int, int> fold)
    {
        _chords = chords;
        _fold = fold;
        Mode = mode;

        foreach (var chord in chords)
            foreach (int note in chord.Notes)
                _targets.Add(new Target(chord.Time, note));
    }

    /// <summary>
    /// The chord wait mode is holding for, or will hold for next. Null once every chord is
    /// done, and always null in play-along mode.
    /// </summary>
    public PracticeChord? PendingChord =>
        Mode == PracticeMode.Wait && _nextChord < _chords.Count ? _chords[_nextChord] : null;

    /// <summary>
    /// Re-aims at <paramref name="position"/> after a seek, restart or loop: chords from there on
    /// become playable again. Stats are kept; call <see cref="ResetStats"/> to clear them.
    /// </summary>
    public void Rewind(TimeSpan position)
    {
        _nextChord = FirstChordAtOrAfter(position);
        _nextTarget = 0;
        for (int i = 0; i < _targets.Count; i++)
        {
            bool stillAhead = _targets[i].Time + HitWindow >= position;
            if (stillAhead) _targets[i].Hit = false;
            else _nextTarget = i + 1;
        }

        // Anything held across the jump has to be struck again to count.
        _fresh.Clear();
    }

    public void ResetStats() => Correct = Missed = Wrong = 0;

    public NoteVerdict NoteOn(int note, TimeSpan position)
    {
        _held.Add(note);
        _fresh.Add(note);

        if (Mode == PracticeMode.Wait)
        {
            bool expected = IsExpectedInWaitMode(note, position);
            Advance(position);
            if (expected) return NoteVerdict.Correct;
        }
        else if (TryHitTarget(note, position))
        {
            Correct++;
            return NoteVerdict.Correct;
        }

        Wrong++;
        return NoteVerdict.Wrong;
    }

    public void NoteOff(int note)
    {
        _held.Remove(note);
        _fresh.Remove(note);
    }

    /// <summary>
    /// Call every frame. Wait mode: satisfies the pending chord if it is now in reach and held.
    /// Play-along: counts notes whose window has closed unplayed as missed.
    /// </summary>
    public void Update(TimeSpan position)
    {
        if (Mode == PracticeMode.Wait)
        {
            Advance(position);
            return;
        }

        while (_nextTarget < _targets.Count && _targets[_nextTarget].Time + HitWindow < position)
        {
            if (!_targets[_nextTarget].Hit) Missed++;
            _nextTarget++;
        }
    }

    /// <summary>Satisfies chords in order for as long as the held keys allow.</summary>
    private void Advance(TimeSpan position)
    {
        while (_nextChord < _chords.Count)
        {
            var chord = _chords[_nextChord];
            if (chord.Time - position > EarlyWindow) return;

            var used = new List<int>(chord.Notes.Count);
            foreach (int note in chord.Notes)
            {
                int? match = FreshHeldMatch(note);
                if (match is null) return;
                used.Add(match.Value);
            }

            foreach (int key in used) _fresh.Remove(key);
            Correct += chord.Notes.Count;
            _nextChord++;
        }
    }

    private int? FreshHeldMatch(int written)
    {
        if (_held.Contains(written) && _fresh.Contains(written)) return written;

        int folded = _fold(written);
        if (folded != written && _held.Contains(folded) && _fresh.Contains(folded)) return folded;

        return null;
    }

    private bool Matches(int written, int played) => played == written || played == _fold(written);

    /// <summary>
    /// A key is fine in wait mode if it belongs to the pending chord or one just after it, so
    /// playing slightly ahead is never punished. Anything else is a wrong note.
    /// </summary>
    private bool IsExpectedInWaitMode(int note, TimeSpan position)
    {
        if (_nextChord >= _chords.Count) return false;

        var horizon = TimeSpan.FromTicks(Math.Max(_chords[_nextChord].Time.Ticks, position.Ticks)) + EarlyWindow;
        for (int i = _nextChord; i < _chords.Count && _chords[i].Time <= horizon; i++)
        {
            if (_chords[i].Notes.Any(n => Matches(n, note))) return true;
        }
        return false;
    }

    /// <summary>Marks the earliest unplayed matching note whose window is open.</summary>
    private bool TryHitTarget(int note, TimeSpan position)
    {
        for (int i = _nextTarget; i < _targets.Count; i++)
        {
            var target = _targets[i];
            if (target.Time - HitWindow > position) break;
            if (target.Hit || !Matches(target.Note, note)) continue;
            if (position > target.Time + HitWindow) continue;

            target.Hit = true;
            return true;
        }
        return false;
    }

    private int FirstChordAtOrAfter(TimeSpan position)
    {
        int lo = 0, hi = _chords.Count;
        while (lo < hi)
        {
            int mid = (lo + hi) / 2;
            if (_chords[mid].Time < position) lo = mid + 1;
            else hi = mid;
        }
        return lo;
    }

    private sealed class Target(TimeSpan time, int note)
    {
        public TimeSpan Time { get; } = time;
        public int Note { get; } = note;
        public bool Hit { get; set; }
    }
}
