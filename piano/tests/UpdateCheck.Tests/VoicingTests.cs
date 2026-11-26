using NAudio.Wave;
using PianoMidiVisualizationApp.Audio;
using Xunit;

namespace UpdateCheckTests;

/// <summary>A steady stereo sine, so the voicing's effect can be measured.</summary>
internal sealed class SineSource(float freq, float amplitude) : ISampleProvider
{
    private long _n;
    public WaveFormat WaveFormat { get; } = WaveFormat.CreateIeeeFloatWaveFormat(44100, 2);

    public int Read(float[] buffer, int offset, int count)
    {
        for (int i = 0; i + 1 < count; i += 2, _n++)
        {
            float v = amplitude * MathF.Sin(2f * MathF.PI * freq * _n / 44100f);
            buffer[offset + i] = v;
            buffer[offset + i + 1] = v;
        }
        return count;
    }
}

public class VoicingTests
{
    /// <summary>RMS of the left channel after a second of settling.</summary>
    private static float Rms(float freq, float amplitude, PianoVoicing voicing)
    {
        var provider = new VoicingSampleProvider(new SineSource(freq, amplitude)) { Voicing = voicing };
        var buffer = new float[2 * 44100];
        provider.Read(buffer, 0, buffer.Length);   // settle
        provider.Read(buffer, 0, buffer.Length);
        double sum = 0;
        for (int i = 0; i < buffer.Length; i += 2) sum += buffer[i] * buffer[i];
        return (float)Math.Sqrt(sum / (buffer.Length / 2));
    }

    [Fact]
    public void Natural_leaves_the_sound_untouched()
    {
        var plain = new SineSource(440f, 0.5f);
        var expected = new float[1024];
        plain.Read(expected, 0, expected.Length);

        var voiced = new VoicingSampleProvider(new SineSource(440f, 0.5f));
        var actual = new float[1024];
        voiced.Read(actual, 0, actual.Length);

        Assert.Equal(expected, actual);
    }

    [Fact]
    public void Stage_grand_is_brighter_and_leaner_than_natural()
    {
        // Same quiet level (below the compressor's threshold) so only the EQ shows.
        float lowMid = Rms(320f, 0.05f, PianoVoicing.StageGrand) / Rms(320f, 0.05f, PianoVoicing.Natural);
        float presence = Rms(3200f, 0.05f, PianoVoicing.StageGrand) / Rms(3200f, 0.05f, PianoVoicing.Natural);
        float rumble = Rms(30f, 0.05f, PianoVoicing.StageGrand) / Rms(30f, 0.05f, PianoVoicing.Natural);

        Assert.True(presence > lowMid * 1.5f, $"presence {presence}, low-mid {lowMid}");
        Assert.True(rumble < 0.5f, $"rumble {rumble}");
    }

    [Fact]
    public void Stage_grand_compresses_loud_playing()
    {
        // A tenfold louder input must come out well under tenfold louder.
        float quiet = Rms(1000f, 0.04f, PianoVoicing.StageGrand);
        float loud = Rms(1000f, 0.4f, PianoVoicing.StageGrand);
        Assert.True(loud / quiet < 8f, $"ratio {loud / quiet}");
    }

    [Fact]
    public void Stage_grand_never_exceeds_full_scale()
    {
        var provider = new VoicingSampleProvider(new SineSource(1000f, 4f)) { Voicing = PianoVoicing.StageGrand };
        var buffer = new float[2 * 44100];
        provider.Read(buffer, 0, buffer.Length);
        Assert.All(buffer, s => Assert.InRange(s, -1f, 1f));
    }

    [Fact]
    public void Switching_voicing_while_running_keeps_the_output_finite()
    {
        var provider = new VoicingSampleProvider(new SineSource(440f, 0.5f));
        var buffer = new float[2048];
        foreach (var v in new[] { PianoVoicing.StageGrand, PianoVoicing.Natural, PianoVoicing.StageGrand })
        {
            provider.Voicing = v;
            provider.Read(buffer, 0, buffer.Length);
            Assert.All(buffer, s => Assert.True(float.IsFinite(s)));
        }
    }

    [Theory]
    [InlineData("StageGrand", PianoVoicing.StageGrand)]
    [InlineData("Natural", PianoVoicing.Natural)]
    [InlineData(null, PianoVoicing.Natural)]
    [InlineData("", PianoVoicing.Natural)]
    [InlineData("1", PianoVoicing.Natural)]
    [InlineData("99", PianoVoicing.Natural)]
    [InlineData("Nonsense", PianoVoicing.Natural)]
    public void A_saved_voicing_reads_back_or_falls_back_to_natural(string? saved, PianoVoicing expected) =>
        Assert.Equal(expected, PianoVoicingInfo.Parse(saved));
}
