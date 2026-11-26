using PianoMidiVisualizationApp.Audio;
using Xunit;

namespace UpdateCheckTests;

public class SynthPianoTests
{
    private const int Rate = 44100;

    /// <summary>Renders the given number of seconds of stereo audio.</summary>
    private static float[] Render(SynthPianoSampleProvider piano, double seconds)
    {
        var buffer = new float[(int)(seconds * Rate) * 2];
        // In blocks, as an audio driver asks for it.
        for (int i = 0; i < buffer.Length; i += 1024)
            piano.Read(buffer, i, Math.Min(1024, buffer.Length - i) & ~1);
        return buffer;
    }

    private static double Rms(float[] buffer, int fromFrame, int toFrame)
    {
        double sum = 0;
        for (int f = fromFrame; f < toFrame; f++) sum += buffer[2 * f] * buffer[2 * f];
        return Math.Sqrt(sum / (toFrame - fromFrame));
    }

    /// <summary>The frequency of the strongest partial between 50 Hz and 1.2 kHz of the left channel.</summary>
    private static double Dominant(float[] buffer, int fromFrame, int frames)
    {
        double best = 0, bestMag = 0;
        for (double f = 50; f < 1200; f += 1.0)
        {
            double re = 0, im = 0;
            for (int i = 0; i < frames; i += 2)   // every other frame is plenty below 1.2 kHz
            {
                double t = (fromFrame + i) / (double)Rate;
                double x = buffer[2 * (fromFrame + i)];
                re += x * Math.Cos(2 * Math.PI * f * t);
                im += x * Math.Sin(2 * Math.PI * f * t);
            }
            double mag = re * re + im * im;
            if (mag > bestMag) { bestMag = mag; best = f; }
        }
        return best;
    }

    [Fact]
    public void A_note_sounds_and_is_in_range()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 60, 100);
        var audio = Render(piano, 1.0);

        Assert.True(Rms(audio, Rate / 10, Rate / 2) > 0.005, "the note is silent");
        Assert.All(audio, s => Assert.InRange(s, -1f, 1f));
    }

    [Fact]
    public void Nothing_plays_before_a_note()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        Assert.All(Render(piano, 0.2), s => Assert.Equal(0f, s));
    }

    [Theory]
    [InlineData(69, 440.0)]   // A4
    [InlineData(57, 220.0)]   // A3
    public void A_note_has_its_pitch(int note, double hz)
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, note, 90);
        var audio = Render(piano, 0.6);

        double found = Dominant(audio, Rate / 10, Rate / 4);
        Assert.InRange(found, hz * 0.99, hz * 1.01);
    }

    [Fact]
    public void Harder_playing_is_louder()
    {
        double Level(int velocity)
        {
            var piano = new SynthPianoSampleProvider(Rate);
            piano.NoteOn(0, 60, velocity);
            return Rms(Render(piano, 0.5), Rate / 20, Rate / 3);
        }

        Assert.True(Level(110) > Level(40) * 2, "a hard note is not clearly louder than a soft one");
    }

    [Fact]
    public void Releasing_a_key_stops_the_note()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 72, 100);
        Render(piano, 0.3);
        piano.NoteOff(0, 72);
        var audio = Render(piano, 1.5);

        Assert.True(Rms(audio, (int)(1.2 * Rate), (int)(1.5 * Rate)) < 0.0005, "the note keeps ringing after the key is up");
    }

    [Fact]
    public void The_sustain_pedal_holds_a_released_note_until_it_is_lifted()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.SetSustainPedal(0, true);
        piano.NoteOn(0, 72, 100);
        Render(piano, 0.3);
        piano.NoteOff(0, 72);
        var held = Render(piano, 1.0);
        double whileDown = Rms(held, Rate / 2, Rate);
        Assert.True(whileDown > 0.002, "the pedal did not hold the note");

        piano.SetSustainPedal(0, false);
        var after = Render(piano, 1.5);
        Assert.True(Rms(after, (int)(1.2 * Rate), (int)(1.5 * Rate)) < 0.0005, "the note keeps ringing after the pedal is up");
    }

    [Fact]
    public void More_notes_than_voices_do_not_break_it()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        for (int note = 21; note < 21 + 60; note++)
            piano.NoteOn(0, note, 127);
        var audio = Render(piano, 0.5);

        Assert.All(audio, s => Assert.True(float.IsFinite(s) && Math.Abs(s) <= 1f));
        Assert.True(Rms(audio, Rate / 10, Rate / 2) > 0.01);
    }

    [Fact]
    public void The_built_in_piano_is_always_there_and_files_need_to_exist()
    {
        Assert.True(BuiltInPiano.IsBuiltIn(BuiltInPiano.Path));
        Assert.True(BuiltInPiano.Exists(BuiltInPiano.Path));
        Assert.False(BuiltInPiano.IsBuiltIn(@"C:\SoundFonts\piano.sf2"));
        Assert.False(BuiltInPiano.Exists(@"C:\definitely\not\here.sf2"));
        Assert.False(BuiltInPiano.Exists(""));
        Assert.False(BuiltInPiano.Exists(null));
    }
}
