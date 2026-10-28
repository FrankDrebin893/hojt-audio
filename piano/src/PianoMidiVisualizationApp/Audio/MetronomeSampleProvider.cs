using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

/// <summary>One click, as scheduled by the audio thread.</summary>
public sealed class MetronomeBeatEventArgs : EventArgs
{
    /// <summary>The click's first frame, counted from the frame the metronome was switched on.</summary>
    public long Frame { get; init; }

    /// <summary>0-based position in the bar; 0 is the downbeat.</summary>
    public int BeatInBar { get; init; }

    public int BeatsPerBar { get; init; }

    /// <summary>True on the downbeat, unless the bar has a single beat and so has no accent.</summary>
    public bool IsAccent { get; init; }

    /// <summary>The tempo governing the interval that starts with this beat.</summary>
    public int Bpm { get; init; }
}

/// <summary>
/// A synthesised metronome click, mixed alongside the synth rather than played as a GM drum
/// note: the loaded soundbank may be a piano-only SFZ with nothing on channel 10.
///
/// Timing is sample-accurate. Beats are scheduled by counting rendered frames, never by a
/// timer, so the tempo cannot drift with UI or thread-scheduling jitter. Beat k of a steady
/// tempo starts on frame ceil(k × 60 × sampleRate / bpm) exactly: the fractional part of each
/// interval is carried forward instead of being rounded away, so fractional periods such as
/// 77 BPM's 34363.6 frames never accumulate error either.
///
/// Settings are written from the UI thread and latched by the audio thread at each beat. A
/// tempo, bar-length or volume change therefore takes effect from the next beat on and never
/// cuts a click short or stretches the interval already in progress.
/// </summary>
public sealed class MetronomeSampleProvider : ISampleProvider
{
    public const int MinBpm = 30;
    public const int MaxBpm = 240;
    public const int MinBeatsPerBar = 1;
    public const int MaxBeatsPerBar = 12;

    private readonly int _channels;
    private readonly long _framesPerMinute;
    private readonly float[] _click;
    private readonly float[] _accentClick;
    private readonly List<MetronomeBeatEventArgs> _beatsThisRead = new();
    private float[] _scratch = Array.Empty<float>();

    // Written by the UI thread, read by the audio thread. Each is a single atomic value.
    private volatile int _bpm = 90;
    private volatile int _beatsPerBar = 4;
    private volatile float _volume = 1f;
    private volatile bool _isEnabled;

    // Audio-thread state only.
    private bool _running;
    private long _frame;
    private long _framesUntilBeat;
    private long _lateness;          // how far the pending beat lands after its ideal time, in frame × bpm units
    private int _latchedBpm;
    private int _beatInBar;
    private float[]? _sounding;
    private int _soundingPosition;
    private float _soundingGain;

    public WaveFormat WaveFormat { get; }

    /// <summary>
    /// Raised on the audio thread, once per click, after the buffer holding it has been
    /// rendered. Handlers must return quickly; marshal anything else to the UI thread.
    /// </summary>
    public event EventHandler<MetronomeBeatEventArgs>? BeatStarted;

    public MetronomeSampleProvider(int sampleRate = 44100, int channels = 2)
    {
        _channels = channels;
        _framesPerMinute = 60L * sampleRate;
        WaveFormat = WaveFormat.CreateIeeeFloatWaveFormat(sampleRate, channels);

        // Higher and louder on the downbeat. A fifth apart, so the pair sounds deliberate.
        _click = SynthesizeClick(sampleRate, frequency: 1000, peak: 0.5f, seconds: 0.025);
        _accentClick = SynthesizeClick(sampleRate, frequency: 1500, peak: 0.8f, seconds: 0.030);
    }

    /// <summary>Clamped to <see cref="MinBpm"/>–<see cref="MaxBpm"/>. Applies from the next beat.</summary>
    public int Bpm
    {
        get => _bpm;
        set => _bpm = Math.Clamp(value, MinBpm, MaxBpm);
    }

    /// <summary>1 means every beat is the same, with no accent. Applies from the next beat.</summary>
    public int BeatsPerBar
    {
        get => _beatsPerBar;
        set => _beatsPerBar = Math.Clamp(value, MinBeatsPerBar, MaxBeatsPerBar);
    }

    /// <summary>The click's own level, 0–1. Applies from the next click.</summary>
    public float Volume
    {
        get => _volume;
        set => _volume = Math.Clamp(value, 0f, 1f);
    }

    /// <summary>
    /// Switching on starts a fresh bar: beat 1 sounds on the first frame of the next buffer.
    /// Switching off stops scheduling beats but lets a click already sounding ring out, since
    /// cutting it mid-waveform would itself pop.
    /// </summary>
    public bool IsEnabled
    {
        get => _isEnabled;
        set => _isEnabled = value;
    }

    public int Read(float[] buffer, int offset, int count)
    {
        // Rendered into a private buffer and copied out at the end, never accumulated into the
        // caller's: NAudio's ASIO buffer can be overwritten mid-render by the native layer.
        if (_scratch.Length < count)
            _scratch = new float[count];

        bool enabled = _isEnabled;
        if (enabled && !_running)
        {
            _running = true;
            _frame = 0;
            _framesUntilBeat = 0;
            _lateness = 0;
            _latchedBpm = 0;
            _beatInBar = -1;
        }
        else if (!enabled)
        {
            _running = false;
        }

        int frames = count / _channels;
        int sample = 0;
        for (int f = 0; f < frames; f++)
        {
            if (_running)
            {
                if (_framesUntilBeat == 0)
                    StartBeat();
                _framesUntilBeat--;
                _frame++;
            }

            float value = 0f;
            if (_sounding != null)
            {
                value = _sounding[_soundingPosition++] * _soundingGain;
                if (_soundingPosition == _sounding.Length)
                    _sounding = null;
            }

            for (int c = 0; c < _channels; c++)
                _scratch[sample++] = value;
        }

        // A partial frame, which a well-behaved caller never asks for.
        for (; sample < count; sample++)
            _scratch[sample] = 0f;

        // Element by element: Array.Copy throws on NAudio's ASIO buffer.
        for (int i = 0; i < count; i++)
            buffer[offset + i] = _scratch[i];

        if (_beatsThisRead.Count > 0)
        {
            foreach (var beat in _beatsThisRead)
                BeatStarted?.Invoke(this, beat);
            _beatsThisRead.Clear();
        }

        return count;
    }

    private void StartBeat()
    {
        int bpm = _bpm;
        int beatsPerBar = _beatsPerBar;

        // The lateness carried over is measured in frame × old-bpm units. Rescale it so the
        // sub-frame remainder survives a tempo change rather than being dropped.
        if (_latchedBpm != 0 && bpm != _latchedBpm)
            _lateness = _lateness * bpm / _latchedBpm;
        _latchedBpm = bpm;

        _beatInBar = _beatInBar + 1 >= beatsPerBar ? 0 : _beatInBar + 1;
        bool accent = beatsPerBar > 1 && _beatInBar == 0;

        _sounding = accent ? _accentClick : _click;
        _soundingPosition = 0;
        _soundingGain = _volume;

        // One beat is framesPerMinute / bpm frames. Measured from this beat's ideal (possibly
        // fractional) onset, the next ideal onset is framesPerMinute - lateness units away;
        // round up to whole frames and carry what rounding added.
        long untilNextIdeal = _framesPerMinute - _lateness;
        _framesUntilBeat = (untilNextIdeal + bpm - 1) / bpm;
        _lateness = _framesUntilBeat * bpm - untilNextIdeal;

        _beatsThisRead.Add(new MetronomeBeatEventArgs
        {
            Frame = _frame,
            BeatInBar = _beatInBar,
            BeatsPerBar = beatsPerBar,
            IsAccent = accent,
            Bpm = bpm
        });
    }

    /// <summary>
    /// A decaying sine: a fast attack so it reads as a click rather than a beep, an
    /// exponential decay, and a short taper so the last sample is exactly zero.
    /// </summary>
    private static float[] SynthesizeClick(int sampleRate, double frequency, float peak, double seconds)
    {
        int length = (int)Math.Round(sampleRate * seconds);
        int attack = Math.Max(1, sampleRate / 4000);      // 0.25 ms
        int taper = length / 5;
        double timeConstant = seconds / 5;                // about -43 dB by the end

        var click = new float[length];
        for (int i = 0; i < length; i++)
        {
            double t = (double)i / sampleRate;
            double envelope = Math.Exp(-t / timeConstant);
            if (i < attack)
                envelope *= (double)i / attack;
            if (i >= length - taper)
                envelope *= (double)(length - 1 - i) / taper;

            click[i] = (float)(peak * envelope * Math.Sin(2 * Math.PI * frequency * t));
        }
        return click;
    }
}
