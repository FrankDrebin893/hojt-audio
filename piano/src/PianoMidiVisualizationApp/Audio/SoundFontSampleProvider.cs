using MeltySynth;
using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

public class SoundFontSampleProvider : INotePlayer
{
    private readonly Synthesizer _synthesizer;
    private readonly object _synthLock = new();

    public WaveFormat WaveFormat { get; }

    public SoundFontSampleProvider(string soundFontPath, int sampleRate = 44100)
    {
        WaveFormat = WaveFormat.CreateIeeeFloatWaveFormat(sampleRate, 2);
        var soundFont = new SoundFont(soundFontPath);
        var settings = new SynthesizerSettings(sampleRate);
        _synthesizer = new Synthesizer(soundFont, settings);
    }

    public void NoteOn(int channel, int note, int velocity)
    {
        lock (_synthLock)
        {
            _synthesizer.NoteOn(channel, note, velocity);
        }
    }

    public void NoteOff(int channel, int note)
    {
        lock (_synthLock)
        {
            _synthesizer.NoteOff(channel, note);
        }
    }

    /// <summary>MeltySynth implements the hold pedal itself; it only needs the controller message.</summary>
    public void SetSustainPedal(int channel, bool isDown)
    {
        lock (_synthLock)
        {
            _synthesizer.ProcessMidiMessage(channel, 0xB0, 64, isDown ? 127 : 0);
        }
    }

    public int Read(float[] buffer, int offset, int count)
    {
        lock (_synthLock)
        {
            // MeltySynth RenderInterleaved expects the span to cover the buffer area
            _synthesizer.RenderInterleaved(buffer.AsSpan(offset, count));
        }
        return count;
    }
}
