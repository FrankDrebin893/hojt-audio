using System.Diagnostics;

namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>
/// Maps wall-clock time to song time: song time advances at <see cref="Speed"/> times real
/// time while running, and never passes <see cref="Barrier"/>. The barrier is how wait mode
/// holds the song at a chord, and how a loop or the song's end is caught exactly rather than
/// a frame late.
/// </summary>
/// <remarks>
/// Position is computed on demand from one (song, wall) anchor pair, so there is no
/// accumulated per-frame drift. Every change of speed or barrier re-anchors first, so time
/// spent held at a barrier is not banked and then released as a jump.
/// </remarks>
public sealed class SongClock
{
    private static readonly long ProcessStart = Stopwatch.GetTimestamp();

    private readonly Func<TimeSpan> _wallClock;
    private TimeSpan _anchorSong;
    private TimeSpan _anchorWall;
    private double _speed = 1.0;

    public SongClock() : this(() => Stopwatch.GetElapsedTime(ProcessStart)) { }

    /// <param name="wallClock">A monotonic time source; injectable so tests can step it.</param>
    public SongClock(Func<TimeSpan> wallClock)
    {
        _wallClock = wallClock;
        _anchorWall = wallClock();
    }

    public bool IsRunning { get; private set; }

    /// <summary>Song seconds per wall-clock second. 0.5 plays at half speed.</summary>
    public double Speed
    {
        get => _speed;
        set
        {
            if (value <= 0) throw new ArgumentOutOfRangeException(nameof(value));
            Reanchor();
            _speed = value;
        }
    }

    /// <summary>A song time the clock stops at until it is moved or cleared.</summary>
    public TimeSpan? Barrier { get; private set; }

    public TimeSpan Position
    {
        get
        {
            var raw = IsRunning
                ? _anchorSong + ((_wallClock() - _anchorWall) * _speed)
                : _anchorSong;

            // A barrier behind the anchor is stale (the clock was sought past it), not a wall.
            return Barrier is { } barrier && raw > barrier && barrier >= _anchorSong ? barrier : raw;
        }
    }

    /// <summary>True while the clock is running but held at its barrier.</summary>
    public bool IsAtBarrier => IsRunning && Barrier is { } barrier && Position >= barrier;

    public void Start()
    {
        if (IsRunning) return;
        _anchorWall = _wallClock();
        IsRunning = true;
    }

    public void Pause()
    {
        if (!IsRunning) return;
        Reanchor();
        IsRunning = false;
    }

    public void Seek(TimeSpan songTime)
    {
        _anchorSong = songTime;
        _anchorWall = _wallClock();
    }

    public void SetBarrier(TimeSpan? barrier)
    {
        Reanchor();
        Barrier = barrier;
    }

    /// <summary>
    /// How long from now, in wall-clock time, until the song reaches <paramref name="songTime"/>
    /// at the current speed, ignoring the barrier. Negative if it is already behind.
    /// </summary>
    public TimeSpan WallTimeUntil(TimeSpan songTime) => (songTime - Position) / _speed;

    private void Reanchor()
    {
        _anchorSong = Position;
        _anchorWall = _wallClock();
    }
}
