using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio.Sfz;

/// <summary>
/// Decodes each WAV sample once and reuses it for every subsequent note that references
/// the same file, so repeated notes don't re-decode from disk (which allocates enough to
/// trigger audible GC pauses under ASIO's tight buffering).
/// </summary>
public class SfzSampleCache
{
    private readonly Dictionary<string, SfzSampleData> _cache = new();
    private readonly object _lock = new();

    public SfzSampleData Get(string samplePath)
    {
        lock (_lock)
        {
            if (_cache.TryGetValue(samplePath, out var cached))
                return cached;
        }

        var data = Decode(samplePath);

        lock (_lock)
        {
            _cache[samplePath] = data;
        }

        return data;
    }

    private static SfzSampleData Decode(string samplePath)
    {
        using var reader = new AudioFileReader(samplePath);

        var totalSamples = (int)(reader.Length / sizeof(float));
        var samples = new float[totalSamples];

        var offset = 0;
        int read;
        while (offset < totalSamples && (read = reader.Read(samples, offset, totalSamples - offset)) > 0)
            offset += read;

        if (offset != totalSamples)
            Array.Resize(ref samples, offset);

        return new SfzSampleData
        {
            Samples = samples,
            Channels = reader.WaveFormat.Channels,
            SampleRate = reader.WaveFormat.SampleRate
        };
    }
}
