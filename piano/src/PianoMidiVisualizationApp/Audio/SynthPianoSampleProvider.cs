using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// A piano made of arithmetic: every note is two slightly detuned strings, each a stack of
/// partials that are a little sharp of the harmonic series (a real string is stiff) and die
/// away at their own rate, with a hammer-position comb on their levels and a short noise
/// thump at the attack. Harder playing is louder and brighter. No samples, so it is ours.
///
/// Everything is allocated up front: <see cref="Read"/> and the note calls never allocate.
/// </summary>
public sealed class SynthPianoSampleProvider : INotePlayer
{
    private const int MaxVoices = 32;
    private const int MaxPartials = 28;
    private const float VoiceLevel = 0.22f;
    private const float Ceiling = 0.9f;

    private enum VoiceState { Free, Held, Sustained, Releasing }

    private sealed class Voice
    {
        public VoiceState State;
        public int Channel;
        public int Note;
        public long Age;

        public int Count;   // oscillators in use (two per partial)
        public readonly float[] Re = new float[MaxPartials * 2];
        public readonly float[] Im = new float[MaxPartials * 2];
        public readonly float[] Cos = new float[MaxPartials * 2];
        public readonly float[] Sin = new float[MaxPartials * 2];
        public readonly float[] Decay = new float[MaxPartials * 2];

        public float GainL, GainR;
        public float Attack, AttackStep;   // 0 to 1 over a couple of milliseconds
        public float ReleaseGain, ReleaseCoeff;

        public float NoiseLevel, NoiseDecay, NoiseLowpass, NoiseState;
        public uint Rng;
    }

    private readonly Voice[] _voices = new Voice[MaxVoices];
    private readonly bool[] _pedal = new bool[16];
    private readonly object _lock = new();
    private readonly float _sampleRate;
    private long _clock;

    public WaveFormat WaveFormat { get; }

    public SynthPianoSampleProvider(int sampleRate = 44100)
    {
        _sampleRate = sampleRate;
        WaveFormat = WaveFormat.CreateIeeeFloatWaveFormat(sampleRate, 2);
        for (int i = 0; i < MaxVoices; i++)
            _voices[i] = new Voice();
    }

    public void NoteOn(int channel, int note, int velocity)
    {
        if (velocity <= 0)
        {
            NoteOff(channel, note);
            return;
        }

        lock (_lock)
        {
            // A key pressed again lets the string it was still ringing go.
            foreach (var old in _voices)
            {
                if (old.Channel == channel && old.Note == note
                    && (old.State == VoiceState.Held || old.State == VoiceState.Sustained))
                    BeginRelease(old);
            }

            Start(TakeVoice(), channel, note, velocity);
        }
    }

    public void NoteOff(int channel, int note)
    {
        lock (_lock)
        {
            Voice? target = null;
            foreach (var v in _voices)
            {
                if (v.State == VoiceState.Held && v.Channel == channel && v.Note == note
                    && (target is null || v.Age > target.Age))
                    target = v;
            }
            if (target is null) return;

            if (channel >= 0 && channel < _pedal.Length && _pedal[channel])
                target.State = VoiceState.Sustained;
            else
                BeginRelease(target);
        }
    }

    public void SetSustainPedal(int channel, bool isDown)
    {
        if (channel < 0 || channel >= _pedal.Length) return;
        lock (_lock)
        {
            _pedal[channel] = isDown;
            if (isDown) return;
            foreach (var v in _voices)
            {
                if (v.State == VoiceState.Sustained && v.Channel == channel)
                    BeginRelease(v);
            }
        }
    }

    public int Read(float[] buffer, int offset, int count)
    {
        lock (_lock)
        {
            Array.Clear(buffer, offset, count);
            int frames = count / 2;
            foreach (var v in _voices)
            {
                if (v.State != VoiceState.Free)
                    Render(v, buffer, offset, frames);
            }

            for (int i = 0; i < frames * 2; i++)
                buffer[offset + i] = SoftCeiling(buffer[offset + i]);
        }
        return count;
    }

    // ---------------------------------------------------------------- voices

    private Voice TakeVoice()
    {
        Voice? best = null;
        foreach (var v in _voices)
        {
            if (v.State == VoiceState.Free) return v;
        }
        // None free: the oldest one that is already dying, else the oldest of all.
        foreach (var v in _voices)
        {
            if (v.State == VoiceState.Releasing && (best is null || v.Age < best.Age))
                best = v;
        }
        if (best is not null) return best;
        foreach (var v in _voices)
        {
            if (best is null || v.Age < best.Age)
                best = v;
        }
        return best!;
    }

    private void BeginRelease(Voice v)
    {
        v.State = VoiceState.Releasing;
        // The damper falls faster on the short bass-light treble strings than on the heavy bass ones.
        float tau = Math.Clamp(0.30f * MathF.Pow(2f, -(v.Note - 48) / 24f), 0.06f, 0.45f);
        v.ReleaseCoeff = MathF.Exp(-1f / (tau * _sampleRate));
    }

    private void Start(Voice v, int channel, int note, int velocity)
    {
        v.State = VoiceState.Held;
        v.Channel = channel;
        v.Note = note;
        v.Age = ++_clock;
        v.Attack = 0f;
        v.AttackStep = 1f / (0.002f * _sampleRate);
        v.ReleaseGain = 1f;
        v.ReleaseCoeff = 1f;
        v.Rng = 0x9E3779B9u ^ (uint)(note * 7919 + velocity * 104729 + (int)_clock);
        if (v.Rng == 0) v.Rng = 1;

        float vel = Math.Clamp(velocity, 1, 127) / 127f;
        float f0 = 440f * MathF.Pow(2f, (note - 69) / 12f);

        // Stiffness: how much sharper each partial is than a pure multiple. Larger at the extremes.
        float stiffness = Math.Clamp(6e-5f * MathF.Pow(2f, (note - 60) / 14f), 2e-5f, 3e-3f);
        // Low strings ring for many seconds, high ones for a fraction of one.
        float tau1 = Math.Clamp(5.5f * MathF.Pow(2f, -(note - 48) / 20f), 0.5f, 14f);
        float brightness = 0.55f + 0.43f * vel;
        float level = VoiceLevel * (0.04f + 0.96f * MathF.Pow(vel, 1.5f));
        // The two strings of a note are never quite in tune: they beat slowly against each other.
        float detune = 0.0002f + 0.00025f * ((note * 0.618f) % 1f);

        // First pass: the levels, so they can be scaled to a known peak.
        float sum = 0f;
        int partials = 0;
        Span<float> amp = stackalloc float[MaxPartials];
        for (int n = 1; n <= MaxPartials; n++)
        {
            float fn = n * f0 * MathF.Sqrt(1f + stiffness * n * n);
            if (fn > 0.45f * _sampleRate) break;
            float hammer = 0.4f + 0.6f * MathF.Abs(MathF.Sin(MathF.PI * n * 0.12f));
            float a = hammer * MathF.Pow(brightness, n - 1) / MathF.Pow(n, 0.9f);
            amp[partials++] = a;
            sum += a;
        }
        if (partials == 0) { v.State = VoiceState.Free; return; }
        float scale = level / sum;

        int k = 0;
        for (int n = 1; n <= partials; n++)
        {
            float fn = n * f0 * MathF.Sqrt(1f + stiffness * n * n);
            float tau = MathF.Max(tau1 / (1f + 0.45f * (n - 1)), 0.08f);
            float decay = MathF.Exp(-1f / (tau * _sampleRate));
            for (int s = 0; s < 2; s++)
            {
                float f = fn * (s == 0 ? 1f - detune : 1f + detune);
                float w = 2f * MathF.PI * f / _sampleRate;
                v.Cos[k] = MathF.Cos(w);
                v.Sin[k] = MathF.Sin(w);
                v.Decay[k] = decay;
                v.Re[k] = amp[n - 1] * scale * 0.5f;
                v.Im[k] = 0f;
                k++;
            }
        }
        v.Count = k;

        // Low notes sit a little left, high notes a little right, as at the keyboard.
        float pan = Math.Clamp((note - 60) / 48f * 0.35f, -0.5f, 0.5f);
        float angle = (pan + 1f) * MathF.PI / 4f;
        v.GainL = MathF.Cos(angle) * 1.41421356f;
        v.GainR = MathF.Sin(angle) * 1.41421356f;

        // The hammer's thump: a few milliseconds of dull noise, harder with harder playing.
        v.NoiseLevel = 0.5f * level * vel;
        v.NoiseDecay = MathF.Exp(-1f / (0.006f * _sampleRate));
        v.NoiseLowpass = 0.25f;
        v.NoiseState = 0f;
    }

    private void Render(Voice v, float[] buffer, int offset, int frames)
    {
        int count = v.Count;
        var re = v.Re; var im = v.Im; var cos = v.Cos; var sin = v.Sin; var decay = v.Decay;

        for (int i = 0; i < frames; i++)
        {
            float s = 0f;
            for (int k = 0; k < count; k++)
            {
                float r = re[k], q = im[k];
                float nr = (r * cos[k] - q * sin[k]) * decay[k];
                float nq = (r * sin[k] + q * cos[k]) * decay[k];
                re[k] = nr;
                im[k] = nq;
                s += nq;
            }

            if (v.NoiseLevel > 1e-6f)
            {
                v.Rng ^= v.Rng << 13; v.Rng ^= v.Rng >> 17; v.Rng ^= v.Rng << 5;
                float noise = (v.Rng / (float)uint.MaxValue) * 2f - 1f;
                v.NoiseState += v.NoiseLowpass * (noise - v.NoiseState);
                s += v.NoiseState * v.NoiseLevel;
                v.NoiseLevel *= v.NoiseDecay;
            }

            if (v.Attack < 1f)
                v.Attack = MathF.Min(1f, v.Attack + v.AttackStep);
            s *= v.Attack;

            if (v.State == VoiceState.Releasing)
            {
                v.ReleaseGain *= v.ReleaseCoeff;
                s *= v.ReleaseGain;
            }

            buffer[offset + 2 * i] += s * v.GainL;
            buffer[offset + 2 * i + 1] += s * v.GainR;
        }

        // A voice that has gone inaudible is free for the next note.
        if (v.State == VoiceState.Releasing && v.ReleaseGain < 1e-4f)
            v.State = VoiceState.Free;
        else if (re[0] * re[0] + im[0] * im[0] < 1e-12f)
            v.State = VoiceState.Free;
    }

    /// <summary>Transparent below the ceiling, rounds off above it instead of clipping.</summary>
    private static float SoftCeiling(float x)
    {
        float a = MathF.Abs(x);
        if (a <= Ceiling) return x;
        float shaped = Ceiling + (1f - Ceiling) * MathF.Tanh((a - Ceiling) / (1f - Ceiling));
        return MathF.CopySign(shaped, x);
    }
}
