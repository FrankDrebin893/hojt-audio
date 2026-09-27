namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzVoice
{
    /// <summary>
    /// How long a voice takes to fade when it has to make way: struck again while the pedal
    /// holds it, or stolen to stay under the voice limit. Short enough not to smear the new
    /// note, long enough not to click.
    /// </summary>
    public const double QuickFadeSeconds = 0.06;

    private readonly float[] _samples;
    private readonly int _channels;
    private readonly double _step;
    private readonly float _gain;
    private readonly int _releaseFrames;
    private readonly int _quickFadeFrames;

    private double _position;

    /// <summary>The release envelope: 1 while held, falling linearly to 0 once released.</summary>
    private float _level = 1f;

    /// <summary>How much <see cref="_level"/> drops per frame; 0 until the voice is released.</summary>
    private float _fadePerFrame;

    public int Channel { get; }
    public int Note { get; }

    /// <summary>
    /// Its key was let go while the sustain pedal was down, so it rings on at full level until
    /// the pedal lifts (<see cref="Release"/>) or the key is struck again (<see cref="Cut"/>).
    /// </summary>
    public bool IsSustained { get; private set; }

    /// <summary>Fading out: released, cut short or stolen. It ends on its own once silent.</summary>
    public bool IsReleasing => _fadePerFrame > 0;

    public SfzVoice(SfzSampleData sampleData, SfzRegion region, int channel, int note, int velocity, int outputSampleRate)
    {
        Channel = channel;
        Note = note;

        _samples = sampleData.Samples;
        _channels = sampleData.Channels;

        var pitchRatio = Math.Pow(2.0, (note - region.PitchKeyCenter) / 12.0);
        _step = pitchRatio * sampleData.SampleRate / outputSampleRate;

        var trackFraction = Math.Clamp(region.AmpVelTrack, 0, 100) / 100.0;
        _gain = (float)((1 - trackFraction) + trackFraction * (velocity / 127.0));

        _releaseFrames = Math.Max(1, (int)(region.AmpegRelease * outputSampleRate));
        _quickFadeFrames = Math.Max(1, (int)(QuickFadeSeconds * outputSampleRate));
    }

    /// <summary>The key was let go under the pedal: keep sounding. Ignored once releasing.</summary>
    public void Sustain()
    {
        if (!IsReleasing)
            IsSustained = true;
    }

    /// <summary>Starts the region's own release, from whatever level the voice is at.</summary>
    public void Release() => FadeOver(_releaseFrames);

    /// <summary>Fades out fast, for a voice that has to make way for a new one.</summary>
    public void Cut() => FadeOver(_quickFadeFrames);

    private void FadeOver(int frames)
    {
        IsSustained = false;

        // The faster fade wins: a cut can hurry a slow release along, but a release arriving
        // after a cut must not slow it back down.
        _fadePerFrame = Math.Max(_fadePerFrame, Math.Max(_level, 1e-6f) / frames);
    }

    /// <returns>false once the voice has nothing more to contribute and should be removed.</returns>
    public bool Mix(float[] buffer, int offset, int frameCount)
    {
        var totalFrames = _samples.Length / _channels;

        for (var i = 0; i < frameCount; i++)
        {
            var idx0 = (int)_position;
            if (idx0 + 1 >= totalFrames)
                return false;

            var frac = (float)(_position - idx0);
            float left, right;
            if (_channels == 2)
            {
                var b0 = idx0 * 2;
                var b1 = b0 + 2;
                left = Lerp(_samples[b0], _samples[b1], frac);
                right = Lerp(_samples[b0 + 1], _samples[b1 + 1], frac);
            }
            else
            {
                left = right = Lerp(_samples[idx0], _samples[idx0 + 1], frac);
            }

            var amplitude = _gain * _level;
            buffer[offset + i * 2] += left * amplitude;
            buffer[offset + i * 2 + 1] += right * amplitude;

            _position += _step;

            if (_fadePerFrame > 0 && (_level -= _fadePerFrame) <= 0)
                return false;
        }

        return true;
    }

    private static float Lerp(float a, float b, float t) => a + (b - a) * t;
}
