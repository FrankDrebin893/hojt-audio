using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// Sums its sources and applies the master volume, entirely in private buffers, then writes
/// the caller's buffer exactly once at the end.
///
/// This replaces NAudio's MixingSampleProvider and VolumeSampleProvider, which both work in
/// place on the buffer they are handed: the mixer accumulates with += and the volume stage
/// multiplies what is already there. On this machine NAudio's ASIO buffer can be overwritten
/// by the native layer mid-render, so it must never be read back or accumulated into.
/// </summary>
public sealed class MasterMixSampleProvider : ISampleProvider
{
    private readonly ISampleProvider[] _sources;
    private float[] _mix = Array.Empty<float>();
    private float[] _sourceBuffer = Array.Empty<float>();
    private volatile float _volume = 1f;

    public WaveFormat WaveFormat { get; }

    public MasterMixSampleProvider(params ISampleProvider[] sources)
    {
        if (sources.Length == 0)
            throw new ArgumentException("At least one source is required.", nameof(sources));

        WaveFormat = sources[0].WaveFormat;
        foreach (var source in sources)
        {
            if (source.WaveFormat.Encoding != WaveFormatEncoding.IeeeFloat
                || source.WaveFormat.SampleRate != WaveFormat.SampleRate
                || source.WaveFormat.Channels != WaveFormat.Channels)
            {
                throw new ArgumentException(
                    $"Every source must be {WaveFormat.SampleRate} Hz {WaveFormat.Channels}-channel float; " +
                    $"got {source.WaveFormat}.", nameof(sources));
            }
        }

        _sources = sources;
    }

    /// <summary>Master gain applied after mixing, so it scales every source alike.</summary>
    public float Volume
    {
        get => _volume;
        set => _volume = value;
    }

    public int Read(float[] buffer, int offset, int count)
    {
        if (_mix.Length < count)
        {
            _mix = new float[count];
            _sourceBuffer = new float[count];
        }
        Array.Clear(_mix, 0, count);

        foreach (var source in _sources)
        {
            int read = source.Read(_sourceBuffer, 0, count);
            for (int i = 0; i < read; i++)
                _mix[i] += _sourceBuffer[i];
        }

        // Element by element: Array.Copy throws on NAudio's ASIO buffer.
        float volume = _volume;
        for (int i = 0; i < count; i++)
            buffer[offset + i] = _mix[i] * volume;

        // Always a full buffer: a source running short must not end playback.
        return count;
    }
}
