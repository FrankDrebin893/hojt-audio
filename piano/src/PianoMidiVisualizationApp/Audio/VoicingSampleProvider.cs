using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// Applies a <see cref="PianoVoicing"/> to the synth's stereo output: a few EQ bands, a gentle
/// stereo-linked compressor and a soft ceiling. Everything is allocated up front, so Read
/// never allocates, and the voicing can be changed from the UI thread while audio runs.
/// </summary>
public sealed class VoicingSampleProvider : ISampleProvider
{
    private readonly ISampleProvider _source;
    private readonly Biquad[] _left;
    private readonly Biquad[] _right;
    private readonly float _attackCoeff;
    private readonly float _releaseCoeff;
    private float _envelope;       // compressor level, linear
    private volatile int _voicing; // PianoVoicing
    private int _appliedVoicing;   // what the filter state was last reset for

    // Compressor, tuned by ear for a piano: slow enough to leave the attack alone.
    private const float ThresholdDb = -16f;
    private const float Ratio = 2.5f;
    private const float MakeupDb = 2.5f;
    private const float Ceiling = 0.9f;

    public WaveFormat WaveFormat => _source.WaveFormat;

    public VoicingSampleProvider(ISampleProvider source)
    {
        if (source.WaveFormat.Encoding != WaveFormatEncoding.IeeeFloat || source.WaveFormat.Channels != 2)
            throw new ArgumentException($"Expected stereo float, got {source.WaveFormat}.", nameof(source));

        _source = source;
        float fs = source.WaveFormat.SampleRate;

        _left = Chain(fs);
        _right = Chain(fs);
        _attackCoeff = TimeCoeff(0.008f, fs);
        _releaseCoeff = TimeCoeff(0.180f, fs);
    }

    public PianoVoicing Voicing
    {
        get => (PianoVoicing)_voicing;
        set => _voicing = (int)value;
    }

    private static Biquad[] Chain(float fs) => new[]
    {
        Biquad.HighPass(fs, 55f, 0.707f),          // no rumble under the lowest strings
        Biquad.Peak(fs, 320f, 0.8f, -2.5f),        // clears the low-mid mud
        Biquad.Peak(fs, 3200f, 0.9f, 2.5f),        // the forward attack
        Biquad.HighShelf(fs, 8000f, 0.707f, 3.0f), // air
    };

    private static float TimeCoeff(float seconds, float fs) => MathF.Exp(-1f / (seconds * fs));

    public int Read(float[] buffer, int offset, int count)
    {
        int read = _source.Read(buffer, offset, count);

        var voicing = _voicing;
        if (voicing != _appliedVoicing)
        {
            // Stale filter and compressor state would click when a voicing comes back in.
            foreach (var b in _left) b.Reset();
            foreach (var b in _right) b.Reset();
            _envelope = 0f;
            _appliedVoicing = voicing;
        }

        if (voicing != (int)PianoVoicing.StageGrand)
            return read;

        float makeup = MathF.Pow(10f, MakeupDb / 20f);
        for (int i = 0; i + 1 < read; i += 2)
        {
            float l = buffer[offset + i];
            float r = buffer[offset + i + 1];
            foreach (var b in _left) l = b.Process(l);
            foreach (var b in _right) r = b.Process(r);

            // One gain for both channels, so the stereo image holds still.
            float peak = MathF.Max(MathF.Abs(l), MathF.Abs(r));
            float coeff = peak > _envelope ? _attackCoeff : _releaseCoeff;
            _envelope = peak + coeff * (_envelope - peak);
            float levelDb = 20f * MathF.Log10(MathF.Max(_envelope, 1e-6f));
            float over = levelDb - ThresholdDb;
            float gainDb = over > 0f ? -over * (1f - 1f / Ratio) : 0f;
            float gain = MathF.Pow(10f, gainDb / 20f) * makeup;

            buffer[offset + i] = SoftCeiling(l * gain);
            buffer[offset + i + 1] = SoftCeiling(r * gain);
        }
        return read;
    }

    /// <summary>Transparent below the ceiling, rounds off above it instead of clipping.</summary>
    internal static float SoftCeiling(float x)
    {
        float a = MathF.Abs(x);
        if (a <= Ceiling) return x;
        float shaped = Ceiling + (1f - Ceiling) * MathF.Tanh((a - Ceiling) / (1f - Ceiling));
        return MathF.CopySign(shaped, x);
    }

    /// <summary>One RBJ-cookbook biquad section, direct form 1.</summary>
    internal sealed class Biquad
    {
        private readonly float _b0, _b1, _b2, _a1, _a2;
        private float _x1, _x2, _y1, _y2;

        private Biquad(float b0, float b1, float b2, float a0, float a1, float a2)
        {
            _b0 = b0 / a0; _b1 = b1 / a0; _b2 = b2 / a0;
            _a1 = a1 / a0; _a2 = a2 / a0;
        }

        public void Reset() => _x1 = _x2 = _y1 = _y2 = 0f;

        public float Process(float x)
        {
            float y = _b0 * x + _b1 * _x1 + _b2 * _x2 - _a1 * _y1 - _a2 * _y2;
            _x2 = _x1; _x1 = x;
            _y2 = _y1; _y1 = y;
            return y;
        }

        public static Biquad HighPass(float fs, float f0, float q)
        {
            float w = 2f * MathF.PI * f0 / fs, cos = MathF.Cos(w), alpha = MathF.Sin(w) / (2f * q);
            return new Biquad((1 + cos) / 2, -(1 + cos), (1 + cos) / 2, 1 + alpha, -2 * cos, 1 - alpha);
        }

        public static Biquad Peak(float fs, float f0, float q, float gainDb)
        {
            float a = MathF.Pow(10f, gainDb / 40f);
            float w = 2f * MathF.PI * f0 / fs, cos = MathF.Cos(w), alpha = MathF.Sin(w) / (2f * q);
            return new Biquad(1 + alpha * a, -2 * cos, 1 - alpha * a, 1 + alpha / a, -2 * cos, 1 - alpha / a);
        }

        public static Biquad HighShelf(float fs, float f0, float q, float gainDb)
        {
            float a = MathF.Pow(10f, gainDb / 40f);
            float w = 2f * MathF.PI * f0 / fs, cos = MathF.Cos(w), alpha = MathF.Sin(w) / (2f * q);
            float s = 2f * MathF.Sqrt(a) * alpha;
            return new Biquad(
                a * ((a + 1) + (a - 1) * cos + s),
                -2 * a * ((a - 1) + (a + 1) * cos),
                a * ((a + 1) + (a - 1) * cos - s),
                (a + 1) - (a - 1) * cos + s,
                2 * ((a - 1) - (a + 1) * cos),
                (a + 1) - (a - 1) * cos - s);
        }
    }
}
