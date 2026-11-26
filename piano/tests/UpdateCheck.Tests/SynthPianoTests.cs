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

    /// <summary>The level of the left channel between two times, in dB.</summary>
    private static double Db(float[] buffer, double from, double to) =>
        20 * Math.Log10(Rms(buffer, (int)(from * Rate), (int)(to * Rate)) + 1e-12);

    /// <summary>The strongest frequency between lo and hi (in steps of step Hz) of the left channel.</summary>
    private static double Strongest(float[] buffer, double lo, double hi, double step, int fromFrame, int frames)
    {
        double best = 0, bestMag = -1;
        for (double f = lo; f < hi; f += step)
        {
            double mag = Magnitude(buffer, f, fromFrame, frames);
            if (mag > bestMag) { bestMag = mag; best = f; }
        }
        return best;
    }

    private static double Magnitude(float[] buffer, double f, int fromFrame, int frames)
    {
        double re = 0, im = 0;
        for (int i = 0; i < frames; i++)
        {
            double t = (fromFrame + i) / (double)Rate;
            double x = buffer[2 * (fromFrame + i)];
            re += x * Math.Cos(2 * Math.PI * f * t);
            im += x * Math.Sin(2 * Math.PI * f * t);
        }
        return re * re + im * im;
    }

    /// <summary>The energy of the left channel between two times, summed over a band in 10 Hz steps.</summary>
    private static double BandEnergy(float[] buffer, double lo, double hi, double from, double to)
    {
        int fromFrame = (int)(from * Rate), frames = (int)(to * Rate) - fromFrame;
        double sum = 0;
        for (double f = lo; f < hi; f += 10.0)
            sum += Magnitude(buffer, f, fromFrame, frames);
        return sum;
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
    public void A_held_note_falls_fast_first_and_then_rings_on_slowly()
    {
        // Two-stage decay: the strings' prompt sound dies quickly, their aftersound lingers.
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 60, 100);
        var audio = Render(piano, 5.0);

        double early = (Db(audio, 0.05, 0.35) - Db(audio, 0.65, 0.95)) / 0.6;   // dB per second
        double late = (Db(audio, 2.0, 3.0) - Db(audio, 4.0, 5.0)) / 2.0;
        Assert.True(late > 0, $"the tail does not fall ({late:F1} dB/s)");
        Assert.True(early > 1.8 * late, $"one even decay, like an organ: {early:F1} dB/s early, {late:F1} dB/s late");
    }

    [Fact]
    public void Bass_partials_are_stretched_sharp_like_a_stiff_string()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 33, 80);   // A1, 55 Hz: a harmonic 10th partial would be at 550 Hz
        var audio = Render(piano, 1.2);

        double tenth = Strongest(audio, 540, 570, 0.1, Rate / 20, Rate);
        Assert.InRange(tenth, 553.0, 565.0);
    }

    [Fact]
    public void A_high_note_has_the_soundboards_knock_under_it()
    {
        // C7's partials are all above 2 kHz; what sounds between 80 and 700 Hz just after the
        // strike is the hammer's knock ringing the body modes.
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 96, 100);
        var audio = Render(piano, 0.3);

        double low = BandEnergy(audio, 80, 700, 0.015, 0.06);
        double all = BandEnergy(audio, 50, 6000, 0.015, 0.06);
        double shareDb = 10 * Math.Log10(low / all);
        Assert.True(shareDb > -24, $"no knock: the body band is {shareDb:F1} dB of the sound");
    }

    [Fact]
    public void The_top_strings_have_no_dampers()
    {
        double After(int note, bool release)
        {
            var piano = new SynthPianoSampleProvider(Rate);
            piano.NoteOn(0, note, 100);
            Render(piano, 0.1);
            if (release) piano.NoteOff(0, note);
            return Rms(Render(piano, 0.4), Rate / 10, (int)(0.3 * Rate));
        }

        Assert.True(After(100, true) > 0.6 * After(100, false), "a top note is cut short by a damper it does not have");
        Assert.True(After(72, true) < 0.4 * After(72, false), "a note below them is not damped");
    }

    [Fact]
    public void The_same_key_struck_twice_is_never_quite_the_same()
    {
        var piano = new SynthPianoSampleProvider(Rate);
        piano.NoteOn(0, 60, 100);
        var first = Render(piano, 0.4);
        piano.NoteOff(0, 60);
        Render(piano, 2.5);
        piano.NoteOn(0, 60, 100);
        var second = Render(piano, 0.4);

        double diff = 0, size = 0;
        for (int i = 0; i < first.Length; i++)
        {
            diff += Math.Abs(first[i] - second[i]);
            size += Math.Abs(first[i]);
        }
        Assert.True(diff > 0.05 * size, "two strikes are identical copies");
        Assert.InRange(Db(first, 0.05, 0.4) - Db(second, 0.05, 0.4), -1.5, 1.5);
    }

    [Fact]
    public void Two_pianos_played_alike_sound_alike()
    {
        float[] Play()
        {
            var piano = new SynthPianoSampleProvider(Rate);
            piano.NoteOn(0, 48, 90);
            piano.NoteOn(0, 64, 70);
            return Render(piano, 0.5);
        }

        Assert.Equal(Play(), Play());
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
