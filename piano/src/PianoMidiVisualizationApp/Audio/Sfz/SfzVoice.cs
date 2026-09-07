namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzVoice
{
    private readonly float[] _samples;
    private readonly int _channels;
    private readonly double _step;
    private readonly float _gain;
    private readonly int _releaseFrames;

    private double _position;
    private int _releaseFramesRemaining = -1;

    public int Channel { get; }
    public int Note { get; }

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
    }

    public void Release()
    {
        if (_releaseFramesRemaining < 0)
            _releaseFramesRemaining = _releaseFrames;
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

            var envelope = _releaseFramesRemaining < 0
                ? 1f
                : Math.Max(0f, (float)_releaseFramesRemaining / _releaseFrames);

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

            buffer[offset + i * 2] += left * _gain * envelope;
            buffer[offset + i * 2 + 1] += right * _gain * envelope;

            _position += _step;

            if (_releaseFramesRemaining >= 0 && --_releaseFramesRemaining < 0)
                return false;
        }

        return true;
    }

    private static float Lerp(float a, float b, float t) => a + (b - a) * t;
}
