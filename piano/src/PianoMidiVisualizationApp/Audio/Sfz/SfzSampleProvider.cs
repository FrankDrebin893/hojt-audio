using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzSampleProvider : INotePlayer
{
    private readonly List<SfzRegion> _regions;
    private readonly SfzSampleCache _sampleCache = new();
    private readonly List<SfzVoice> _voices = new();
    private readonly object _voicesLock = new();
    private readonly int _sampleRate;
    private float[] _scratch = Array.Empty<float>();

    public WaveFormat WaveFormat { get; }

    public SfzSampleProvider(string sfzPath, int sampleRate = 44100)
    {
        _sampleRate = sampleRate;
        _regions = SfzParser.Parse(sfzPath);
        WaveFormat = WaveFormat.CreateIeeeFloatWaveFormat(sampleRate, 2);
    }

    public void NoteOn(int channel, int note, int velocity)
    {
        var region = _regions.FirstOrDefault(r => r.Matches(note, velocity));
        if (region == null)
            return;

        var sampleData = _sampleCache.Get(region.SamplePath);
        var voice = new SfzVoice(sampleData, region, channel, note, velocity, _sampleRate);
        lock (_voicesLock)
        {
            _voices.Add(voice);
        }
    }

    public void NoteOff(int channel, int note)
    {
        lock (_voicesLock)
        {
            foreach (var voice in _voices)
            {
                if (voice.Channel == channel && voice.Note == note)
                    voice.Release();
            }
        }
    }

    public int Read(float[] buffer, int offset, int count)
    {
        // Mix into a private buffer that only we ever touch, then copy the finished result
        // into the caller's buffer at the end. NAudio's ASIO buffer can otherwise get written
        // into by the native interop layer while we're still accumulating into it mid-render,
        // corrupting the tail of the buffer with stale/unrelated audio.
        if (_scratch.Length < count)
            _scratch = new float[count];
        Array.Clear(_scratch, 0, count);

        lock (_voicesLock)
        {
            for (var i = _voices.Count - 1; i >= 0; i--)
            {
                if (!_voices[i].Mix(_scratch, 0, count / 2))
                    _voices.RemoveAt(i);
            }
        }

        for (var i = 0; i < count; i++)
            buffer[offset + i] = _scratch[i];

        return count;
    }
}
