using System.Runtime.InteropServices;
using NAudio.Wave;
using Vec = System.Numerics.Vector<float>;

namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// A piano made of arithmetic. Every partial of a note is two modes of its coupled strings: a
/// loud one that falls away quickly (the prompt sound) and a quiet one a fraction of a hertz off
/// that rings on several times longer (the aftersound), so a held note drops and then sings
/// instead of fading like an organ. The partials are stretched sharp as on a stiff string, most in
/// the bass and the top; the hammer's felt shapes them (harder playing is louder and brighter, and
/// the contact is a soft onset, not a click), and its strike point leaves a comb in their levels.
/// The soundboard radiates little of the lowest fundamentals and colours the rest, and the hammer
/// knocks it: a pulse rung through a handful of wooden body modes. The bass carries phantom
/// partials when played hard, the top strings have no dampers, a damper landing on a string makes
/// a faint thump, and no two strikes are quite alike. A small computed room sits around it, a
/// little longer while the sustain pedal is down. No samples, so it is ours.
///
/// Everything is allocated up front: <see cref="Read"/> and the note calls never allocate. The
/// partials are computed in SIMD lanes (<see cref="System.Numerics.Vector{T}"/>).
/// </summary>
public sealed class SynthPianoSampleProvider : INotePlayer
{
    private const int MaxVoices = 32;
    private const int MaxPartials = 48;
    private const int MaxPhantoms = 8;
    private const int MaxOscillators = 112;   // 2 per partial and the phantoms, padded to whole SIMD lanes
    private const double FrequencyCap = 10000.0;
    private const int BlockFrames = 1024;
    private const float VoiceLevel = 0.22f;
    private const float Ceiling = 0.9f;

    // The highest note with a damper: above it the strings ring on after the key is up, as on a piano.
    private const int TopDampedNote = 88;

    // The soundboard's body modes that the hammer's knock rings: frequency (Hz) and decay time (s).
    private static readonly double[] BodyFrequency = { 92, 138, 196, 262, 351, 478, 655, 905, 1290, 1870 };
    private static readonly double[] BodyDecay = { 0.10, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.025, 0.018, 0.012 };

    // The room: eight delay lines (in samples at 44.1 kHz) fed back through a Hadamard matrix.
    private static readonly int[] RoomDelays = { 557, 653, 797, 919, 1087, 1213, 1381, 1543 };
    private static readonly float[] RoomInputSign = { 1f, -1f, 1f, 1f, -1f, 1f, -1f, -1f };
    private const float RoomDamping = 0.35f;     // one-pole lowpass in each line: the walls take the top
    private const double RoomTime = 0.45;        // RT60 in seconds
    private const double PedalRoomTime = 0.9;    // the undamped strings ring along while the pedal is down
    private const float RoomSend = 0.07f;
    private const float PedalRoomSend = 0.10f;

    private enum VoiceState { Free, Held, Sustained, Releasing }

    private sealed class Voice
    {
        public VoiceState State;
        public int Channel;
        public int Note;
        public long Age;

        // Each oscillator is a decaying phasor: (Re, Im) turns by (Cos, Sin) each sample, which
        // already include its decay. Count is a whole number of SIMD lanes; the spare ones are zero.
        public int Count;
        public readonly float[] Re = new float[MaxOscillators];
        public readonly float[] Im = new float[MaxOscillators];
        public readonly float[] Cos = new float[MaxOscillators];
        public readonly float[] Sin = new float[MaxOscillators];
        public readonly float[] Freq = new float[MaxOscillators];

        public float GainL, GainR;
        public int RampPos, RampLen;       // the felt's contact: a raised-cosine onset
        public float Fade, FadeCoeff;      // 1 and 1 until the key is struck again
        public bool Damped;
        public float DamperTau;

        public int KnockPos, KnockLen;     // the hammer's force pulse into the soundboard
        public float KnockLevel;
        public float NoiseLevel, NoiseDecay;   // felt noise at the strike, the damper's thump at release
        public uint Rng;
    }

    private readonly Voice[] _voices = new Voice[MaxVoices];
    private readonly bool[] _pedal = new bool[16];
    private readonly object _lock = new();
    private readonly float _sampleRate;
    private long _clock;

    // The knock of every voice, summed, for the soundboard modes.
    private readonly float[] _knock = new float[BlockFrames];

    private readonly float[] _bodyGain = new float[BodyFrequency.Length];
    private readonly float[] _bodyA1 = new float[BodyFrequency.Length];
    private readonly float[] _bodyA2 = new float[BodyFrequency.Length];
    private readonly float[] _bodyY1 = new float[BodyFrequency.Length];
    private readonly float[] _bodyY2 = new float[BodyFrequency.Length];

    private readonly float[] _roomLine;
    private readonly int[] _roomStart = new int[8];
    private readonly int[] _roomLength = new int[8];
    private readonly int[] _roomPos = new int[8];
    private readonly float[] _roomLowpass = new float[8];
    private readonly float[] _roomTap = new float[8];
    private readonly float[] _roomGain = new float[8];
    private readonly float[] _pedalRoomGain = new float[8];

    public WaveFormat WaveFormat { get; }

    public SynthPianoSampleProvider(int sampleRate = 44100)
    {
        _sampleRate = sampleRate;
        WaveFormat = WaveFormat.CreateIeeeFloatWaveFormat(sampleRate, 2);
        for (int i = 0; i < MaxVoices; i++)
            _voices[i] = new Voice();

        for (int m = 0; m < BodyFrequency.Length; m++)
        {
            double r = Math.Exp(-1.0 / (BodyDecay[m] * sampleRate));
            double w = 2.0 * Math.PI * BodyFrequency[m] / sampleRate;
            _bodyA1[m] = (float)(2.0 * r * Math.Cos(w));
            _bodyA2[m] = (float)(r * r);
            _bodyGain[m] = (float)((1.0 - r) * 2.0 * Math.Pow(BodyFrequency[m] / 300.0, -0.3));
        }

        int total = 0;
        for (int j = 0; j < 8; j++)
        {
            int length = Math.Max(1, (int)Math.Round(RoomDelays[j] * sampleRate / 44100.0));
            _roomStart[j] = total;
            _roomLength[j] = length;
            total += length;
            _roomGain[j] = (float)Math.Pow(10.0, -3.0 * length / (RoomTime * sampleRate));
            _pedalRoomGain[j] = (float)Math.Pow(10.0, -3.0 * length / (PedalRoomTime * sampleRate));
        }
        _roomLine = new float[total];
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
            // A key struck again takes over its strings: what they still rang gives way to the new strike.
            foreach (var old in _voices)
            {
                if (old.State != VoiceState.Free && old.Channel == channel && old.Note == note)
                    BeginFade(old);
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
            for (int done = 0; done < frames; done += BlockFrames)
                RenderBlock(buffer, offset + 2 * done, Math.Min(BlockFrames, frames - done));
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

    /// <summary>The key is up: the damper lands, and takes the high partials faster than the low ones.</summary>
    private void BeginRelease(Voice v)
    {
        v.State = VoiceState.Releasing;
        if (!v.Damped) return;

        for (int k = 0; k < v.Count; k++)
        {
            float extra = (1f + 0.04f * MathF.Min(v.Freq[k] / 100f, 40f)) / v.DamperTau;
            float g = MathF.Exp(-extra / _sampleRate);
            v.Cos[k] *= g;
            v.Sin[k] *= g;
        }

        // The felt landing on the string: a faint thump through the soundboard.
        v.NoiseLevel = MathF.Max(v.NoiseLevel, 0.12f * v.KnockLevel);
        v.NoiseDecay = MathF.Exp(-1f / (0.008f * _sampleRate));
    }

    /// <summary>The same key struck again: the old strike gives way within a few tens of milliseconds.</summary>
    private void BeginFade(Voice v)
    {
        v.State = VoiceState.Releasing;
        v.FadeCoeff = MathF.Min(v.FadeCoeff, MathF.Exp(-1f / (0.05f * _sampleRate)));
    }

    /// <summary>A uniform number in [-1, 1] from the voice's own generator (xorshift32).</summary>
    private static float NextRandom(Voice v)
    {
        uint r = v.Rng;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        v.Rng = r;
        return r / (float)uint.MaxValue * 2f - 1f;
    }

    /// <summary>
    /// How much sharper than harmonic a string's partials are (B in f_n = n f0 sqrt(1 + B n^2)):
    /// about 1e-4 in the tenor, 5e-4 at the bottom and several 1e-3 at the top.
    /// </summary>
    internal static double Inharmonicity(int note) =>
        6e-5 * (Math.Pow(2.0, (note - 48) / 9.0) + Math.Pow(2.0, (48 - note) / 9.0));

    /// <summary>
    /// What the soundboard radiates of a partial at <paramref name="f"/> Hz: little below about
    /// 85 Hz (the lowest fundamentals are heard through their harmonics), a few gentle lumps, and
    /// a slow fall above 6.5 kHz.
    /// </summary>
    private static double BodyResponse(double f)
    {
        double x = f / 85.0;
        double highpass = x * x / Math.Sqrt(1.0 + x * x * x * x);
        double lowpass = 1.0 / Math.Sqrt(1.0 + (f / 6500.0) * (f / 6500.0));
        double octave = Math.Log2(Math.Max(f, 20.0) / 100.0);
        double rippleDb = 0.7 * Math.Sin(2.1 * octave + 0.6) + 0.4 * Math.Sin(5.3 * octave + 2.0);
        return highpass * lowpass * Math.Pow(10.0, rippleDb / 20.0);
    }

    private void Start(Voice v, int channel, int note, int velocity)
    {
        v.State = VoiceState.Held;
        v.Channel = channel;
        v.Note = note;
        v.Age = ++_clock;
        v.Rng = 0x9E3779B9u ^ (uint)(note * 7919 + velocity * 104729 + (int)_clock);
        if (v.Rng == 0) v.Rng = 1;

        double sr = _sampleRate;
        double vel = Math.Clamp(velocity, 1, 127) / 127.0;
        double f0 = 440.0 * Math.Pow(2.0, (note - 69) / 12.0);
        double stiffness = Inharmonicity(note);
        double level = VoiceLevel * (0.04 + 0.96 * Math.Pow(vel, 1.5));
        double register = Math.Clamp((note - 21) / 87.0, 0.0, 1.0);   // 0 at A0, 1 at C8

        // The hammer: harder playing compresses the felt more and shortens the contact, so its
        // cutoff rises; the strike point, about an eighth of the string, combs the partials.
        double cutoff = (500.0 + 2.2 * f0) * Math.Pow(2.0, 3.2 * vel - 0.6);
        double strike = (0.125 - 0.06 * register) * (1.0 + 0.03 * NextRandom(v));

        // Decay rates in 1/s: b1 from the bridge, b2 and b3 from losses that grow with frequency.
        double b1 = 0.45 * Math.Pow(2.0, (note - 21) / 27.0);
        const double b2 = 4e-4, b3 = 1.5e-7;

        // The aftersound: the second mode of the coupled strings, quieter, slower, a little off.
        double afterFraction = (0.20 - 0.06 * register) * (1.0 + 0.2 * NextRandom(v));
        double afterRatio = 4.5 - 1.5 * register;
        double afterDetune = (0.15 + 0.5 * register) * (1.0 + 0.6 * NextRandom(v));
        if (NextRandom(v) <= 0f) afterDetune = -afterDetune;

        Span<double> amp = stackalloc double[MaxOscillators];
        Span<double> freq = stackalloc double[MaxOscillators];
        Span<double> sigma = stackalloc double[MaxOscillators];
        Span<double> phase = stackalloc double[MaxOscillators];
        Span<double> partialAmp = stackalloc double[MaxPartials];
        Span<double> partialFreq = stackalloc double[MaxPartials];
        Span<double> partialSigma = stackalloc double[MaxPartials];

        int partials = 0, n = 0;
        double power = 0.0;
        for (int p = 1; p <= MaxPartials; p++)
        {
            double fp = p * f0 * Math.Sqrt(1.0 + stiffness * p * p);
            if (fp > 0.45 * sr || (p > 1 && fp > FrequencyCap)) break;

            double comb = 0.45 + 0.55 * Math.Abs(Math.Sin(Math.PI * p * strike));
            double ratio = fp / cutoff;
            double hammer = 1.0 / Math.Sqrt(1.0 + ratio * ratio * ratio * ratio);   // 12 dB/oct above the cutoff
            double a = comb * hammer * BodyResponse(fp) / p;
            a *= Math.Pow(10.0, 0.03 * NextRandom(v));                            // no two strikes alike
            double s = b1 + b2 * fp + b3 * fp * fp;

            partialAmp[partials] = a;
            partialFreq[partials] = fp;
            partialSigma[partials] = s;
            partials++;
            power += a * a;

            amp[n] = a * (1.0 - afterFraction);
            freq[n] = fp;
            sigma[n] = s;
            phase[n] = 0.0;
            n++;

            amp[n] = a * afterFraction;
            freq[n] = fp + afterDetune * Math.Sqrt(p);
            sigma[n] = b1 / afterRatio + (s - b1);   // only the bridge's share is slower
            phase[n] = 0.25 * Math.PI * NextRandom(v);
            n++;
        }
        if (partials == 0) { v.State = VoiceState.Free; return; }

        // Phantom partials: a hard-struck bass string's tension swing sounds sums of its partials.
        double phantom = Math.Max(0.0, (60 - note) / 39.0) * 0.35 * vel;
        if (phantom > 0.0)
        {
            for (int i = 0; i < MaxPhantoms; i++)
            {
                int j = PhantomFirst[i], k = PhantomSecond[i];
                if (k > partials) continue;
                double fp = partialFreq[j - 1] + partialFreq[k - 1];
                if (fp > FrequencyCap || fp > 0.45 * sr) continue;
                amp[n] = phantom * Math.Sqrt(partialAmp[j - 1] * partialAmp[k - 1]) * 0.5;
                freq[n] = fp;
                sigma[n] = partialSigma[j - 1] + partialSigma[k - 1];
                phase[n] = -0.5 * Math.PI;
                n++;
            }
        }

        // Scaled to the velocity's level by their power; the treble a touch quieter.
        double tilt = Math.Pow(10.0, -0.03 * Math.Max(0, note - 60) / 20.0);
        double scale = level * 0.62 * tilt / Math.Sqrt(power);
        double loudest = 0.0;
        for (int i = 0; i < n; i++)
            loudest = Math.Max(loudest, amp[i]);

        int count = 0;
        for (int i = 0; i < n; i++)
        {
            if (amp[i] < 1e-3 * loudest) continue;   // 60 dB down: not worth a lane
            double a = amp[i] * scale;
            double w = 2.0 * Math.PI * freq[i] / sr;
            double d = Math.Exp(-sigma[i] / sr);
            v.Re[count] = (float)(a * Math.Cos(phase[i]));
            v.Im[count] = (float)(a * Math.Sin(phase[i]));
            v.Cos[count] = (float)(Math.Cos(w) * d);
            v.Sin[count] = (float)(Math.Sin(w) * d);
            v.Freq[count] = (float)freq[i];
            count++;
        }
        int lanes = Vec.Count;
        int padded = (count + lanes - 1) / lanes * lanes;
        for (int i = count; i < padded; i++)
        {
            v.Re[i] = 0f;
            v.Im[i] = 0f;
            v.Cos[i] = 0f;
            v.Sin[i] = 0f;
            v.Freq[i] = 0f;
        }
        v.Count = padded;

        // Low notes sit a little left, high notes a little right, as at the keyboard.
        float pan = Math.Clamp((note - 60) / 48f * 0.35f, -0.5f, 0.5f);
        float angle = (pan + 1f) * MathF.PI / 4f;
        v.GainL = MathF.Cos(angle) * 1.41421356f;
        v.GainR = MathF.Sin(angle) * 1.41421356f;

        // The felt's contact time: longer for soft playing and in the bass.
        double contact = Math.Clamp(0.0035 * (1.0 - 0.6 * vel) * Math.Pow(2.0, -(note - 60) / 30.0), 0.0006, 0.005);
        v.RampLen = Math.Max(2, (int)(contact * sr));
        v.RampPos = 0;
        v.Fade = 1f;
        v.FadeCoeff = 1f;
        v.Damped = note <= TopDampedNote;
        v.DamperTau = Math.Clamp(0.30f * MathF.Pow(2f, -(note - 48) / 24f), 0.06f, 0.45f);

        // The knock: heard most under the short treble strings.
        v.KnockLevel = (float)(level * 0.05 * (0.14 + 1.86 * register * register) * (0.4 + 0.6 * vel));
        v.KnockLen = v.RampLen;
        v.KnockPos = 0;
        v.NoiseLevel = 0.25f * v.KnockLevel;
        v.NoiseDecay = MathF.Exp(-1f / (0.004f * _sampleRate));
    }

    private static readonly int[] PhantomFirst = { 2, 3, 2, 4, 3, 5, 4, 6 };
    private static readonly int[] PhantomSecond = { 2, 3, 3, 4, 4, 5, 5, 6 };

    private void RenderBlock(float[] buffer, int offset, int frames)
    {
        Array.Clear(_knock, 0, frames);
        foreach (var v in _voices)
        {
            if (v.State != VoiceState.Free)
                Render(v, buffer, offset, frames);
        }

        bool pedal = false;
        foreach (bool down in _pedal) pedal |= down;
        float[] roomGain = pedal ? _pedalRoomGain : _roomGain;
        float send = pedal ? PedalRoomSend : RoomSend;

        for (int i = 0; i < frames; i++)
        {
            // The soundboard rings with the knocks.
            float knock = _knock[i];
            float body = 0f;
            for (int m = 0; m < _bodyGain.Length; m++)
            {
                float y = _bodyGain[m] * knock + _bodyA1[m] * _bodyY1[m] - _bodyA2[m] * _bodyY2[m];
                if (MathF.Abs(y) < 1e-15f) y = 0f;   // no denormals once it has died away
                _bodyY2[m] = _bodyY1[m];
                _bodyY1[m] = y;
                body += y;
            }

            float left = buffer[offset + 2 * i] + body;
            float right = buffer[offset + 2 * i + 1] + body;

            // The room.
            var tap = _roomTap;
            for (int j = 0; j < 8; j++)
                tap[j] = _roomLine[_roomStart[j] + _roomPos[j]];
            float wetL = 0.5f * (tap[0] + tap[2] + tap[4] + tap[6]);
            float wetR = 0.5f * (tap[1] + tap[3] + tap[5] + tap[7]);
            Hadamard8(tap);
            float input = 0.5f * (left + right);
            for (int j = 0; j < 8; j++)
            {
                float x = tap[j] * roomGain[j] + RoomInputSign[j] * input;
                float lp = x + RoomDamping * (_roomLowpass[j] - x);
                if (MathF.Abs(lp) < 1e-15f) lp = 0f;
                _roomLowpass[j] = lp;
                _roomLine[_roomStart[j] + _roomPos[j]] = lp;
                if (++_roomPos[j] == _roomLength[j]) _roomPos[j] = 0;
            }

            buffer[offset + 2 * i] = SoftCeiling(left + send * wetL);
            buffer[offset + 2 * i + 1] = SoftCeiling(right + send * wetR);
        }
    }

    /// <summary>The 8-point Hadamard transform in place, scaled to keep energy (an orthogonal matrix).</summary>
    private static void Hadamard8(float[] x)
    {
        for (int h = 1; h < 8; h *= 2)
        {
            for (int i = 0; i < 8; i += 2 * h)
            {
                for (int j = i; j < i + h; j++)
                {
                    float a = x[j], b = x[j + h];
                    x[j] = a + b;
                    x[j + h] = a - b;
                }
            }
        }
        for (int j = 0; j < 8; j++)
            x[j] *= 0.35355339f;
    }

    private void Render(Voice v, float[] buffer, int offset, int frames)
    {
        var re = MemoryMarshal.Cast<float, Vec>(v.Re.AsSpan(0, v.Count));
        var im = MemoryMarshal.Cast<float, Vec>(v.Im.AsSpan(0, v.Count));
        var cos = MemoryMarshal.Cast<float, Vec>(v.Cos.AsSpan(0, v.Count));
        var sin = MemoryMarshal.Cast<float, Vec>(v.Sin.AsSpan(0, v.Count));
        int lanes = re.Length;
        var knockBus = _knock;

        for (int i = 0; i < frames; i++)
        {
            var sum = Vec.Zero;
            for (int k = 0; k < lanes; k++)
            {
                Vec r = re[k], q = im[k], c = cos[k], s = sin[k];
                re[k] = r * c - q * s;
                Vec nq = r * s + q * c;
                im[k] = nq;
                sum += nq;
            }
            float x = System.Numerics.Vector.Dot(sum, Vec.One);

            if (v.RampPos < v.RampLen)
            {
                v.RampPos++;
                x *= 0.5f - 0.5f * MathF.Cos(MathF.PI * v.RampPos / v.RampLen);
            }

            float knock = 0f;
            if (v.KnockPos < v.KnockLen)
            {
                knock = v.KnockLevel * MathF.Sin(MathF.PI * v.KnockPos / v.KnockLen);
                v.KnockPos++;
            }
            if (v.NoiseLevel > 1e-7f)
            {
                knock += v.NoiseLevel * NextRandom(v);
                v.NoiseLevel *= v.NoiseDecay;
            }

            if (v.FadeCoeff < 1f)
            {
                v.Fade *= v.FadeCoeff;
                x *= v.Fade;
                knock *= v.Fade;
            }

            buffer[offset + 2 * i] += x * v.GainL;
            buffer[offset + 2 * i + 1] += x * v.GainR;
            knockBus[i] += knock;
        }

        // A voice that has gone inaudible is free for the next note.
        if (v.Fade < 1e-4f)
        {
            v.State = VoiceState.Free;
            return;
        }
        float energy = 0f;
        for (int k = 0; k < v.Count; k++)
            energy += v.Re[k] * v.Re[k] + v.Im[k] * v.Im[k];
        if (energy * v.Fade * v.Fade < 1e-11f && v.KnockPos >= v.KnockLen && v.NoiseLevel <= 1e-7f)
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
