using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzSampleProvider : INotePlayer
{
    private readonly List<SfzRegion> _regions;
    private readonly SfzSampleCache _sampleCache = new();
    private readonly List<SfzVoice> _voices = new();
    private readonly object _voicesLock = new();
    private readonly int _sampleRate;

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
        Array.Clear(buffer, offset, count);

        lock (_voicesLock)
        {
            for (var i = _voices.Count - 1; i >= 0; i--)
            {
                if (!_voices[i].Mix(buffer, offset, count / 2))
                    _voices.RemoveAt(i);
            }
        }

        return count;
    }
}
