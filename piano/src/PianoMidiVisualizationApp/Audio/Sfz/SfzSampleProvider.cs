using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzSampleProvider : INotePlayer
{
    /// <summary>
    /// Voices allowed to sound at full level at once. The sustain pedal lets voices pile up
    /// (a pedalled glissando keeps every note ringing), so past this the oldest make way.
    /// Voices already fading out are not counted: they end on their own within a release.
    /// </summary>
    public const int MaxVoices = 96;

    private readonly List<SfzRegion> _regions;
    private readonly SfzSampleCache _sampleCache = new();
    private readonly List<SfzVoice> _voices = new();
    private readonly object _voicesLock = new();
    private readonly int _sampleRate;
    private float[] _scratch = Array.Empty<float>();

    /// <summary>The sustain pedal (CC64) per MIDI channel. Guarded by <see cref="_voicesLock"/>.</summary>
    private readonly bool[] _pedalDown = new bool[16];

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
            // Struck again while the pedal still holds the last strike: the old sound gives way
            // to the new one, as a re-struck string would, rather than doubling up under it.
            foreach (var old in _voices)
            {
                if (old.IsSustained && old.Channel == channel && old.Note == note)
                    old.Cut();
            }

            MakeRoomLocked();
            _voices.Add(voice);
        }
    }

    public void NoteOff(int channel, int note)
    {
        lock (_voicesLock)
        {
            bool pedalDown = IsPedalDownLocked(channel);
            foreach (var voice in _voices)
            {
                if (voice.Channel != channel || voice.Note != note || voice.IsReleasing)
                    continue;

                if (pedalDown)
                    voice.Sustain();
                else
                    voice.Release();
            }
        }
    }

    public void SetSustainPedal(int channel, bool isDown)
    {
        if (channel is < 0 or > 15)
            return;

        lock (_voicesLock)
        {
            _pedalDown[channel] = isDown;
            if (isDown)
                return;

            // Lifting the pedal releases what only the pedal was holding. A key still held
            // down never got its note-off, so it is not sustained and keeps sounding.
            foreach (var voice in _voices)
            {
                if (voice.IsSustained && voice.Channel == channel)
                    voice.Release();
            }
        }
    }

    private bool IsPedalDownLocked(int channel) => channel is >= 0 and <= 15 && _pedalDown[channel];

    /// <summary>
    /// Fades out the oldest voices until a new one fits under <see cref="MaxVoices"/>. Those
    /// only the pedal is holding go first; a key still held down goes only if nothing else can.
    /// </summary>
    private void MakeRoomLocked()
    {
        int sounding = _voices.Count(v => !v.IsReleasing);
        while (sounding >= MaxVoices)
        {
            var victim = _voices.FirstOrDefault(v => v.IsSustained)
                         ?? _voices.First(v => !v.IsReleasing);
            victim.Cut();
            sounding--;
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
