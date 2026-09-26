using System.Diagnostics;
using System.Runtime.InteropServices;

namespace PianoMidiVisualizationApp.Services.Playback;

public sealed class PlaybackPositionEventArgs(TimeSpan position) : EventArgs
{
    /// <summary>Where the playhead is, in sequence time (not scaled by the speed factor).</summary>
    public TimeSpan Position { get; } = position;
}

/// <summary>
/// Plays a list of <see cref="TimedNote"/>s in real time on a dedicated timing thread, with
/// Play/Stop, looping over [<see cref="LoopStart"/>, <see cref="LoopEnd"/>) and a speed factor.
///
/// <para>Shared by the recorder and by every other feature that makes the app play notes by
/// itself. Wire it to <c>MainViewModel.PlayNoteOn</c>/<c>PlayNoteOff</c>, which sound, light
/// and analyse a note without recording it:
/// <code>var player = new NoteSequencePlayer(vm.PlayNoteOn, vm.PlayNoteOff);</code></para>
///
/// <para>Nothing it starts is ever left hanging: every note still sounding gets its note-off on
/// <see cref="Stop"/>, on each loop wrap, on <see cref="Load"/> and on <see cref="Dispose"/>,
/// so the note-on and note-off callbacks always pair up.</para>
///
/// <para>Threading: every member may be called from any thread. The note callbacks run on the
/// timing thread — or on the caller's thread for the note-offs that Stop, Load and Dispose send
/// — while the player's lock is held, so that a Stop can never race a note-on into a hung
/// note. They must therefore be quick, must not call back into the player, and must never
/// block on the UI thread (<c>Dispatcher.BeginInvoke</c>, never <c>Invoke</c>), or a Stop from
/// the UI thread would deadlock against them. <see cref="PositionChanged"/> and
/// <see cref="PlaybackStopped"/> are raised outside the lock, on the timing thread except where
/// noted, so a UI subscriber has to marshal.</para>
/// </summary>
public sealed class NoteSequencePlayer : IDisposable
{
    /// <summary>A loop range shorter than this falls back to looping the whole sequence.</summary>
    public static readonly TimeSpan MinimumLoopLength = TimeSpan.FromMilliseconds(50);

    public const double MinimumSpeed = 0.1;
    public const double MaximumSpeed = 4.0;

    // Wait-then-spin: the thread waits on a timer until this close to an event, then busy-spins
    // the rest. Measured under ~95% CPU load, a high-resolution timer wait lands within ~0.5 ms,
    // so 2 ms of spin absorbs it with room to spare.
    private const double SpinThresholdMs = 2.0;

    // The thread never waits longer than this, so position updates and a changed loop range
    // or speed are picked up promptly even across a long rest in the music.
    private const double MaxWaitMs = 10;

    // An overshoot past the loop end longer than this is a stall (a debugger break, a suspend)
    // or a loop range moved behind the playhead — not timing jitter — so the wrap restarts the
    // loop from now instead of fast-forwarding through the loops it "missed".
    private static readonly TimeSpan MaxWrapCatchUp = TimeSpan.FromMilliseconds(100);

    private static readonly long PositionIntervalTicks = Stopwatch.Frequency / 30;

    private const int NotSounding = -1;

    private readonly Action<int, int> _noteOn;
    private readonly Action<int> _noteOff;
    private readonly object _sync = new();

    private IReadOnlyList<TimedNote> _notes = [];
    private ScheduledEvent[] _events = [];
    private TimeSpan _length;
    private TimeSpan? _loopStart;
    private TimeSpan? _loopEnd;
    private bool _isLooping;
    private double _speed = 1.0;

    private bool _isPlaying;
    private bool _disposed;

    /// <summary>
    /// Bumped by every Play and Stop. The timing thread checks it under the lock before each
    /// step, so a superseded thread winds down on its own and never needs joining.
    /// </summary>
    private int _generation;

    /// <summary>Index of the next event to dispatch.</summary>
    private int _cursor;

    // The playhead is extrapolated from a single anchor rather than accumulated tick by tick,
    // so timing jitter never builds up into drift: position = anchor + elapsed × speed.
    private long _anchorTimestamp;
    private TimeSpan _anchorPosition;

    /// <summary>
    /// Which note instance (index into the loaded notes) is sounding on each key, or
    /// <see cref="NotSounding"/>. Tracking the instance rather than a flag means an earlier
    /// note's late note-off can never cut short a later note on the same key.
    /// </summary>
    private readonly int[] _sounding = new int[128];

    /// <summary>Raised about 30 times a second while playing, on the timing thread.</summary>
    public event EventHandler<PlaybackPositionEventArgs>? PositionChanged;

    /// <summary>
    /// Raised once playback stops, whether it reached the end or was stopped, loaded over or
    /// disposed. It comes from the timing thread at the end, otherwise from the caller's thread.
    /// Not raised when <see cref="Play()"/> restarts a sequence that is already playing.
    /// </summary>
    public event EventHandler? PlaybackStopped;

    /// <param name="noteOn">Called with (note, velocity) to start a note.</param>
    /// <param name="noteOff">Called with the note to release.</param>
    public NoteSequencePlayer(Action<int, int> noteOn, Action<int> noteOff)
    {
        _noteOn = noteOn ?? throw new ArgumentNullException(nameof(noteOn));
        _noteOff = noteOff ?? throw new ArgumentNullException(nameof(noteOff));
        Array.Fill(_sounding, NotSounding);
    }

    /// <summary>The loaded notes, as passed to <see cref="Load"/> minus any out of MIDI range.</summary>
    public IReadOnlyList<TimedNote> Notes
    {
        get { lock (_sync) return _notes; }
    }

    /// <summary>How long the sequence runs: the explicit length given to Load, else its last note-off.</summary>
    public TimeSpan Length
    {
        get { lock (_sync) return _length; }
    }

    public bool IsPlaying
    {
        get { lock (_sync) return _isPlaying; }
    }

    /// <summary>The playhead in sequence time. Zero whenever playback is stopped.</summary>
    public TimeSpan Position
    {
        get
        {
            lock (_sync)
                return _isPlaying ? PositionAt(Stopwatch.GetTimestamp()) : TimeSpan.Zero;
        }
    }

    /// <summary>
    /// Whether playback wraps from <see cref="LoopEnd"/> back to <see cref="LoopStart"/> or
    /// stops at <see cref="Length"/>. Takes effect immediately, including mid-playback.
    /// </summary>
    public bool IsLooping
    {
        get { lock (_sync) return _isLooping; }
        set { lock (_sync) _isLooping = value; }
    }

    /// <summary>
    /// Playback rate: 0.5 plays at half speed. Clamped to [<see cref="MinimumSpeed"/>,
    /// <see cref="MaximumSpeed"/>]. Changing it mid-playback keeps the playhead where it is.
    /// </summary>
    public double Speed
    {
        get { lock (_sync) return _speed; }
        set
        {
            if (double.IsNaN(value)) throw new ArgumentOutOfRangeException(nameof(value));
            value = Math.Clamp(value, MinimumSpeed, MaximumSpeed);

            lock (_sync)
            {
                if (_isPlaying)
                {
                    long now = Stopwatch.GetTimestamp();
                    _anchorPosition = PositionAt(now);
                    _anchorTimestamp = now;
                }
                _speed = value;
            }
        }
    }

    /// <summary>Where a loop starts: the range set with <see cref="SetLoopRange"/>, else zero.</summary>
    public TimeSpan LoopStart
    {
        get { lock (_sync) return GetLoopRangeLocked().Start; }
    }

    /// <summary>Where a loop wraps (exclusive): the range set with <see cref="SetLoopRange"/>, else <see cref="Length"/>.</summary>
    public TimeSpan LoopEnd
    {
        get { lock (_sync) return GetLoopRangeLocked().End; }
    }

    /// <summary>
    /// Loops over [start, end) instead of the whole sequence. Only notes starting inside the
    /// range play; one still sounding at <paramref name="end"/> is released at the wrap. The
    /// range is clamped to the sequence, and one shorter than <see cref="MinimumLoopLength"/>
    /// falls back to the whole sequence. Takes effect immediately; if the playhead is already
    /// past <paramref name="end"/> it jumps straight to <paramref name="start"/>.
    /// </summary>
    public void SetLoopRange(TimeSpan start, TimeSpan end)
    {
        if (end <= start)
            throw new ArgumentOutOfRangeException(nameof(end), "The loop must end after it starts.");

        lock (_sync)
        {
            _loopStart = start;
            _loopEnd = end;
        }
    }

    /// <summary>Loops the whole sequence again.</summary>
    public void ClearLoopRange()
    {
        lock (_sync)
        {
            _loopStart = null;
            _loopEnd = null;
        }
    }

    /// <summary>
    /// Replaces the sequence, stopping any playback first and clearing the loop range (the
    /// looping flag and speed are kept). Notes outside MIDI range 0-127 are dropped.
    /// </summary>
    /// <param name="length">
    /// How long the sequence runs, e.g. a recording's full length including a trailing rest.
    /// Defaults to the last note-off.
    /// </param>
    public void Load(IEnumerable<TimedNote> notes, TimeSpan? length = null)
    {
        ArgumentNullException.ThrowIfNull(notes);

        bool wasPlaying;
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            wasPlaying = StopLocked();

            var valid = notes
                .Where(n => n.Note is >= 0 and <= 127
                         && n.Start >= TimeSpan.Zero && n.Duration >= TimeSpan.Zero)
                .ToArray();

            var events = new ScheduledEvent[valid.Length * 2];
            for (int i = 0; i < valid.Length; i++)
            {
                var n = valid[i];
                events[2 * i] = new ScheduledEvent(n.Start, n.Note, Math.Clamp(n.Velocity, 1, 127), i, true);
                events[2 * i + 1] = new ScheduledEvent(n.End, n.Note, 0, i, false);
            }
            Array.Sort(events);

            _notes = valid;
            _events = events;
            _length = length is { } l && l > TimeSpan.Zero
                ? l
                : valid.Length > 0 ? valid.Max(n => n.End) : TimeSpan.Zero;
            _loopStart = null;
            _loopEnd = null;
        }

        if (wasPlaying) RaisePlaybackStopped();
    }

    /// <summary>
    /// Starts from the top: the loop start when looping, else zero. Restarts if already playing.
    /// </summary>
    public void Play() => Play(null);

    /// <summary>
    /// Starts from <paramref name="from"/> (the loop start, or zero, when null). Notes already
    /// under way at that point are not started part-way through. Restarts if already playing.
    /// Does nothing when the sequence is empty.
    /// </summary>
    public void Play(TimeSpan? from)
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            StopLocked();

            if (_length <= TimeSpan.Zero) return;

            var start = from ?? (_isLooping ? GetLoopRangeLocked().Start : TimeSpan.Zero);
            if (start < TimeSpan.Zero) start = TimeSpan.Zero;

            _anchorPosition = start;
            _anchorTimestamp = Stopwatch.GetTimestamp();
            _cursor = FirstEventAtOrAfter(start);
            _isPlaying = true;

            int generation = ++_generation;
            var thread = new Thread(() => Run(generation))
            {
                IsBackground = true,
                Priority = ThreadPriority.Highest,
                Name = "NoteSequencePlayer"
            };
            thread.Start();
        }
    }

    /// <summary>Stops playback and releases every sounding note. Safe to call when stopped.</summary>
    public void Stop()
    {
        bool wasPlaying;
        lock (_sync)
            wasPlaying = StopLocked();

        if (wasPlaying) RaisePlaybackStopped();
    }

    public void Dispose()
    {
        bool wasPlaying;
        lock (_sync)
        {
            if (_disposed) return;
            wasPlaying = StopLocked();
            _disposed = true;
        }

        if (wasPlaying) RaisePlaybackStopped();
    }

    // ------------------------------------------------------------------ timing thread

    private void Run(int generation)
    {
        using (var timing = new TimingThreadScope())
        {
            long lastPositionReport = 0;

            while (true)
            {
                long now = Stopwatch.GetTimestamp();
                TimeSpan position;
                double waitMs = 0;
                bool ended = false;

                lock (_sync)
                {
                    if (generation != _generation) return;

                    position = PositionAt(now);
                    bool looping = TryGetLoopLocked(out var loopStart, out var loopEnd) && _isLooping;
                    var end = looping ? loopEnd : _length;

                    while (position >= end)
                    {
                        // Everything due before the end still plays; the loop is [start, end), so
                        // an event exactly at a loop end belongs to the next pass instead.
                        if (!DispatchUntilLocked(end, inclusive: !looping)) return;

                        if (!looping)
                        {
                            StopLocked();
                            ended = true;
                            break;
                        }

                        ReleaseAllLocked();
                        if (generation != _generation) return;

                        // Re-anchor at the exact instant the loop end was reached rather than at
                        // "now", so the loop length stays exact however many times it wraps.
                        long endTimestamp = _anchorTimestamp + ToStopwatchTicks((end - _anchorPosition) / _speed);
                        _anchorTimestamp = Stopwatch.GetElapsedTime(endTimestamp, now) > MaxWrapCatchUp
                            ? now
                            : endTimestamp;
                        _anchorPosition = loopStart;
                        _cursor = FirstEventAtOrAfter(loopStart);
                        position = PositionAt(now);
                    }

                    if (!ended)
                    {
                        if (!DispatchUntilLocked(position, inclusive: true)) return;

                        var next = _cursor < _events.Length && _events[_cursor].Time < end
                            ? _events[_cursor].Time
                            : end;
                        waitMs = (next - position).TotalMilliseconds / _speed;
                    }
                }

                if (ended)
                {
                    RaisePlaybackStopped();
                    return;
                }

                if (now - lastPositionReport >= PositionIntervalTicks)
                {
                    lastPositionReport = now;
                    RaisePositionChanged(position);
                }

                if (waitMs > SpinThresholdMs)
                {
                    timing.Wait(Math.Min(waitMs - SpinThresholdMs, MaxWaitMs));
                }
                else if (waitMs > 0)
                {
                    // A pure busy-spin, outside the lock. Not Thread.Yield: that hands the core
                    // to whatever else is ready, even at lower priority, and under load cost up
                    // to ~5 ms of lateness. A Stop meanwhile is caught on the next pass.
                    long due = now + (long)(waitMs * Stopwatch.Frequency / 1000);
                    while (Stopwatch.GetTimestamp() < due)
                        Thread.SpinWait(20);
                }
            }
        }
    }

    /// <summary>
    /// Sends every event before <paramref name="limit"/> (or at it, if inclusive). Returns false
    /// if a callback stopped or restarted the player, in which case the caller must bail out.
    /// </summary>
    private bool DispatchUntilLocked(TimeSpan limit, bool inclusive)
    {
        int generation = _generation;

        while (_cursor < _events.Length)
        {
            var ev = _events[_cursor];
            if (inclusive ? ev.Time > limit : ev.Time >= limit) break;
            _cursor++;

            if (ev.IsOn)
            {
                // A second note on a key that is still sounding re-strikes it.
                if (_sounding[ev.Note] != NotSounding)
                    Invoke(_noteOff, ev.Note);
                _sounding[ev.Note] = ev.Instance;
                Invoke(_noteOn, ev.Note, ev.Velocity);
            }
            else if (_sounding[ev.Note] == ev.Instance)
            {
                _sounding[ev.Note] = NotSounding;
                Invoke(_noteOff, ev.Note);
            }

            if (generation != _generation) return false;
        }

        return true;
    }

    /// <summary>Returns whether it was playing.</summary>
    private bool StopLocked()
    {
        _generation++;
        bool wasPlaying = _isPlaying;
        _isPlaying = false;
        _anchorPosition = TimeSpan.Zero;
        ReleaseAllLocked();
        return wasPlaying;
    }

    private void ReleaseAllLocked()
    {
        for (int note = 0; note < _sounding.Length; note++)
        {
            if (_sounding[note] == NotSounding) continue;
            _sounding[note] = NotSounding;
            Invoke(_noteOff, note);
        }
    }

    private TimeSpan PositionAt(long timestamp) =>
        _anchorPosition + Stopwatch.GetElapsedTime(_anchorTimestamp, timestamp) * _speed;

    private (TimeSpan Start, TimeSpan End) GetLoopRangeLocked()
    {
        TryGetLoopLocked(out var start, out var end);
        return (start, end);
    }

    /// <summary>The effective loop range. False if even the whole sequence is too short to loop.</summary>
    private bool TryGetLoopLocked(out TimeSpan start, out TimeSpan end)
    {
        start = Clamp(_loopStart ?? TimeSpan.Zero, _length);
        end = Clamp(_loopEnd ?? _length, _length);

        if (end - start < MinimumLoopLength)
        {
            start = TimeSpan.Zero;
            end = _length;
        }

        return end - start >= MinimumLoopLength;
    }

    private static TimeSpan Clamp(TimeSpan value, TimeSpan max) =>
        value < TimeSpan.Zero ? TimeSpan.Zero : value > max ? max : value;

    /// <summary>The first event at or after <paramref name="time"/>, by binary search.</summary>
    private int FirstEventAtOrAfter(TimeSpan time)
    {
        int lo = 0, hi = _events.Length;
        while (lo < hi)
        {
            int mid = (lo + hi) >>> 1;
            if (_events[mid].Time < time) lo = mid + 1;
            else hi = mid;
        }
        return lo;
    }

    private static long ToStopwatchTicks(TimeSpan span) =>
        (long)Math.Round(span.TotalSeconds * Stopwatch.Frequency);

    // A throwing callback must neither kill the process (this is a bare thread) nor abandon the
    // rest of a release loop and strand notes, so failures are logged and skipped.
    private static void Invoke(Action<int> callback, int note)
    {
        try { callback(note); }
        catch (Exception ex) { Debug.WriteLine($"NoteSequencePlayer note-off callback failed: {ex}"); }
    }

    private static void Invoke(Action<int, int> callback, int note, int velocity)
    {
        try { callback(note, velocity); }
        catch (Exception ex) { Debug.WriteLine($"NoteSequencePlayer note-on callback failed: {ex}"); }
    }

    private void RaisePositionChanged(TimeSpan position)
    {
        try { PositionChanged?.Invoke(this, new PlaybackPositionEventArgs(position)); }
        catch (Exception ex) { Debug.WriteLine($"NoteSequencePlayer PositionChanged handler failed: {ex}"); }
    }

    private void RaisePlaybackStopped()
    {
        try { PlaybackStopped?.Invoke(this, EventArgs.Empty); }
        catch (Exception ex) { Debug.WriteLine($"NoteSequencePlayer PlaybackStopped handler failed: {ex}"); }
    }

    /// <summary>
    /// One note-on or note-off. Sorted by time, then note-ons first, so that a zero-length
    /// note still starts before it stops and a re-struck key's new note-on is not cancelled by
    /// the old note's note-off at the same instant (instance tracking discards that note-off).
    /// </summary>
    private readonly record struct ScheduledEvent(TimeSpan Time, int Note, int Velocity, int Instance, bool IsOn)
        : IComparable<ScheduledEvent>
    {
        public int CompareTo(ScheduledEvent other)
        {
            int byTime = Time.CompareTo(other.Time);
            if (byTime != 0) return byTime;
            if (IsOn != other.IsOn) return IsOn ? -1 : 1;
            return Instance.CompareTo(other.Instance);
        }
    }

    /// <summary>
    /// What makes the timing thread punctual, set up and torn down on the thread itself.
    ///
    /// <para>Measured on this machine at ~95% CPU load, waiting for targets 5-40 ms ahead:
    /// Sleep with timeBeginPeriod(1) landed up to 19 ms late — Windows 11 ignores a process's
    /// timer-resolution request when it judges it invisible or inaudible, and an ASIO-only app
    /// is inaudible to it — and even an honoured high-resolution timer was up to 7 ms late
    /// without MMCSS, because the thread was not scheduled in time. With both, the worst of
    /// ~460 waits was 0.03 ms late.</para>
    /// </summary>
    private sealed class TimingThreadScope : IDisposable
    {
        private readonly IntPtr _timer;
        private readonly IntPtr _mmcss;
        private readonly bool _raisedTimerResolution;

        public TimingThreadScope()
        {
            // Registers the thread with the Multimedia Class Scheduler, which schedules it in the
            // real-time band like an audio thread. Best effort: Highest priority is the fallback.
            uint taskIndex = 0;
            _mmcss = NativeMethods.AvSetMmThreadCharacteristics("Pro Audio", ref taskIndex);

            // A high-resolution waitable timer (Windows 10 1803+) is precise on its own and
            // needs no process-wide timer-resolution change.
            _timer = NativeMethods.CreateWaitableTimerEx(
                IntPtr.Zero, null, NativeMethods.CreateWaitableTimerHighResolution, NativeMethods.TimerAllAccess);

            if (_timer == IntPtr.Zero)
            {
                NativeMethods.TimeBeginPeriod(1);
                _raisedTimerResolution = true;
            }
        }

        public void Wait(double milliseconds)
        {
            if (_timer != IntPtr.Zero)
            {
                long dueTime = -(long)(milliseconds * 10_000);   // negative: relative, in 100 ns units
                if (NativeMethods.SetWaitableTimer(_timer, ref dueTime, 0, IntPtr.Zero, IntPtr.Zero, false))
                {
                    NativeMethods.WaitForSingleObject(_timer, NativeMethods.Infinite);
                    return;
                }
            }

            Thread.Sleep(Math.Max(1, (int)milliseconds));
        }

        public void Dispose()
        {
            if (_timer != IntPtr.Zero) NativeMethods.CloseHandle(_timer);
            if (_raisedTimerResolution) NativeMethods.TimeEndPeriod(1);
            if (_mmcss != IntPtr.Zero) NativeMethods.AvRevertMmThreadCharacteristics(_mmcss);
        }
    }

    private static class NativeMethods
    {
        public const uint CreateWaitableTimerHighResolution = 0x2;
        public const uint TimerAllAccess = 0x1F0003;
        public const uint Infinite = 0xFFFFFFFF;

        [DllImport("kernel32.dll", EntryPoint = "CreateWaitableTimerExW", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateWaitableTimerEx(IntPtr attributes, string? name, uint flags, uint access);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetWaitableTimer(IntPtr timer, ref long dueTime, int period,
                                                   IntPtr completionRoutine, IntPtr argument,
                                                   [MarshalAs(UnmanagedType.Bool)] bool resume);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool CloseHandle(IntPtr handle);

        [DllImport("avrt.dll", EntryPoint = "AvSetMmThreadCharacteristicsW", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr AvSetMmThreadCharacteristics(string taskName, ref uint taskIndex);

        [DllImport("avrt.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool AvRevertMmThreadCharacteristics(IntPtr handle);

        [DllImport("winmm.dll", EntryPoint = "timeBeginPeriod")]
        public static extern uint TimeBeginPeriod(uint milliseconds);

        [DllImport("winmm.dll", EntryPoint = "timeEndPeriod")]
        public static extern uint TimeEndPeriod(uint milliseconds);
    }
}
