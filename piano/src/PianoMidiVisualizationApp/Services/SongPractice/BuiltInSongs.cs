using System.IO;
using Melanchall.DryWetMidi.Common;
using Melanchall.DryWetMidi.Core;
using Melanchall.DryWetMidi.Interaction;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>
/// Synthesizes and maintains the six built-in public-domain classic piano pieces in
/// <c>%LOCALAPPDATA%\PianoMidiVisualizationApp\BuiltInSongs</c> so Song Practice is ready
/// to play out of the box with two-hand parts.
/// </summary>
public static class BuiltInSongs
{
    private const int Ticks = 480; // per quarter note

    // MIDI note constants (C4 = 60, middle C)
    private const int D2 = 38, E2 = 40, FSharp2 = 42, G2 = 43, A2 = 45, B2 = 47;
    private const int C3 = 48, CSharp3 = 49, D3 = 50, E3 = 52, FSharp3 = 54, G3 = 55, GSharp3 = 56, A3 = 57, B3 = 59;
    private const int C4 = 60, CSharp4 = 61, D4 = 62, E4 = 64, F4 = 65, FSharp4 = 66, G4 = 67, GSharp4 = 68, A4 = 69, B4 = 71;
    private const int C5 = 72, CSharp5 = 73, D5 = 74, DSharp5 = 75, E5 = 76, F5 = 77, FSharp5 = 78, G5 = 79, A5 = 81;

    /// <summary>
    /// Ensures all six built-in MIDI files exist on disk and returns their <see cref="SongBankItem"/> descriptors.
    /// </summary>
    public static IReadOnlyList<SongBankItem> EnsureAll()
    {
        string folder = GetBuiltInFolder();

        var specs = new (string FileName, string Title, string Subtitle, Action<string> Writer)[]
        {
            ("ode-to-joy.mid", "Ode to Joy", "Beethoven · C Major · 100 BPM", WriteOdeToJoy),
            ("bach-prelude-c.mid", "Prelude in C Major (BWV 846)", "J.S. Bach · C Major · 84 BPM", WriteBachPreludeInC),
            ("minuet-in-g.mid", "Minuet in G Major", "Petzold / Bach · G Major · 108 BPM", WriteMinuetInG),
            ("gymnopedie-no-1.mid", "Gymnopédie No. 1", "Erik Satie · D Major · 72 BPM", WriteGymnopedieNo1),
            ("fur-elise.mid", "Für Elise (Theme)", "Beethoven · A Minor · 92 BPM", WriteFurElise),
            ("canon-in-d.mid", "Canon in D (Theme)", "Pachelbel · D Major · 80 BPM", WriteCanonInD),
        };

        var items = new List<SongBankItem>(specs.Length);
        foreach (var (fileName, title, subtitle, writer) in specs)
        {
            string path = Path.Combine(folder, fileName);
            try
            {
                if (!File.Exists(path) || new FileInfo(path).Length == 0)
                    writer(path);
            }
            catch
            {
                // Fallback to temp directory if writing to LocalApplicationData fails.
                string fallbackFolder = Path.Combine(Path.GetTempPath(), "PianoMidiVisualizationApp", "BuiltInSongs");
                Directory.CreateDirectory(fallbackFolder);
                path = Path.Combine(fallbackFolder, fileName);
                try
                {
                    if (!File.Exists(path) || new FileInfo(path).Length == 0)
                        writer(path);
                }
                catch
                {
                    // Ignore write errors; item will remain listed.
                }
            }

            items.Add(new SongBankItem
            {
                Title = title,
                Subtitle = subtitle,
                FilePath = path,
                IsBuiltIn = true
            });
        }

        return items;
    }

    private static string GetBuiltInFolder()
    {
        try
        {
            string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            if (!string.IsNullOrWhiteSpace(localAppData))
            {
                string folder = Path.Combine(localAppData, "PianoMidiVisualizationApp", "BuiltInSongs");
                Directory.CreateDirectory(folder);
                return folder;
            }
        }
        catch
        {
            // Fall through to temp path.
        }

        string fallback = Path.Combine(Path.GetTempPath(), "PianoMidiVisualizationApp", "BuiltInSongs");
        Directory.CreateDirectory(fallback);
        return fallback;
    }

    // ------------------------------------------------------------------ 1. Ode to Joy (Beethoven)

    private static void WriteOdeToJoy(string path)
    {
        (int Note, double Beats)[] melody =
        [
            (E4, 1), (E4, 1), (F4, 1), (G4, 1),    (G4, 1), (F4, 1), (E4, 1), (D4, 1),
            (C4, 1), (C4, 1), (D4, 1), (E4, 1),    (E4, 1.5), (D4, 0.5), (D4, 2),
            (E4, 1), (E4, 1), (F4, 1), (G4, 1),    (G4, 1), (F4, 1), (E4, 1), (D4, 1),
            (C4, 1), (C4, 1), (D4, 1), (E4, 1),    (D4, 1.5), (C4, 0.5), (C4, 2),
            (D4, 1), (D4, 1), (E4, 1), (C4, 1),    (D4, 1), (E4, 0.5), (F4, 0.5), (E4, 1), (C4, 1),
            (D4, 1), (E4, 0.5), (F4, 0.5), (E4, 1), (D4, 1),    (C4, 1), (D4, 1), (G3, 2),
            (E4, 1), (E4, 1), (F4, 1), (G4, 1),    (G4, 1), (F4, 1), (E4, 1), (D4, 1),
            (C4, 1), (C4, 1), (D4, 1), (E4, 1),    (D4, 1.5), (C4, 0.5), (C4, 2),
        ];

        int[] cChord = [C3, E3, G3];
        int[] gChord = [G2, B2, D3];
        int[][] chords =
        [
            cChord, cChord,  gChord, gChord,  cChord, cChord,  gChord, gChord,
            cChord, cChord,  gChord, gChord,  cChord, cChord,  gChord, cChord,
            gChord, cChord,  gChord, cChord,  gChord, gChord,  cChord, gChord,
            cChord, cChord,  gChord, gChord,  cChord, cChord,  gChord, cChord,
        ];

        var rightNotes = new List<Note>();
        long time = 0;
        foreach (var (note, beats) in melody)
        {
            long length = (long)(beats * Ticks);
            rightNotes.Add(new Note((SevenBitNumber)note, length - Ticks / 16, time) { Velocity = (SevenBitNumber)80 });
            time += length;
        }

        var leftNotes = new List<Note>();
        for (int i = 0; i < chords.Length; i++)
        {
            foreach (int note in chords[i])
                leftNotes.Add(new Note((SevenBitNumber)note, 2 * Ticks - Ticks / 8, i * 2L * Ticks) { Velocity = (SevenBitNumber)60 });
        }

        WriteTwoHandMidi(path, bpm: 100, numerator: 4, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ 2. Prelude in C Major, BWV 846 (J.S. Bach)

    private static void WriteBachPreludeInC(string path)
    {
        // Each bar of BWV 846 has 5 voices arpeggiated in 16th notes twice per 4/4 bar:
        // Left hand holds the bottom two notes (root + tenor); Right hand plays the upper three voices twice.
        (int Bass, int Tenor, int Alto, int Soprano1, int Soprano2)[] bars =
        [
            (C4, E4, G4, C5, E5),       // Bar 1: C major
            (C4, D4, A4, D5, F5),       // Bar 2: Dm7/C
            (B3, D4, G4, D5, F5),       // Bar 3: G7/B
            (C4, E4, G4, C5, E5),       // Bar 4: C major
            (C4, E4, A4, E5, A5),       // Bar 5: Am/C
            (C4, D4, FSharp4, A4, D5),  // Bar 6: D7/C
            (B3, D4, G4, D5, G5),       // Bar 7: G/B
            (C3, G3, E4, G4, C5),       // Bar 8: C major cadence
        ];

        var rightNotes = new List<Note>();
        var leftNotes = new List<Note>();
        long sixteenth = Ticks / 4;

        for (int bar = 0; bar < bars.Length; bar++)
        {
            var (bass, tenor, alto, sop1, sop2) = bars[bar];
            for (int half = 0; half < 2; half++)
            {
                long halfStart = (bar * 4L + half * 2L) * Ticks;

                // Left hand: bass on 1st sixteenth (held for half-bar), tenor on 2nd sixteenth
                leftNotes.Add(new Note((SevenBitNumber)bass, 8 * sixteenth - Ticks / 16, halfStart)
                {
                    Velocity = (SevenBitNumber)68
                });
                leftNotes.Add(new Note((SevenBitNumber)tenor, 7 * sixteenth - Ticks / 16, halfStart + sixteenth)
                {
                    Velocity = (SevenBitNumber)64
                });

                // Right hand: 6 sixteenth notes (steps 2..7 of the 8-sixteenth half-bar)
                int[] rhPattern = [alto, sop1, sop2, alto, sop1, sop2];
                for (int step = 0; step < rhPattern.Length; step++)
                {
                    long noteStart = halfStart + (step + 2) * sixteenth;
                    rightNotes.Add(new Note((SevenBitNumber)rhPattern[step], sixteenth - Ticks / 32, noteStart)
                    {
                        Velocity = (SevenBitNumber)(step is 2 or 5 ? 78 : 72)
                    });
                }
            }
        }

        WriteTwoHandMidi(path, bpm: 84, numerator: 4, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ 3. Minuet in G Major (Petzold / Bach)

    private static void WriteMinuetInG(string path)
    {
        // 8 bars in 3/4 time (24 beats total)
        (int Note, double Beats)[] rightSeq =
        [
            // Bar 1
            (D5, 1.0), (G4, 0.5), (A4, 0.5), (B4, 0.5), (C5, 0.5),
            // Bar 2
            (D5, 1.0), (G4, 1.0), (G4, 1.0),
            // Bar 3
            (E5, 1.0), (C5, 0.5), (D5, 0.5), (E5, 0.5), (FSharp5, 0.5),
            // Bar 4
            (G5, 1.0), (G4, 1.0), (G4, 1.0),
            // Bar 5
            (C5, 1.0), (D5, 0.5), (C5, 0.5), (B4, 0.5), (A4, 0.5),
            // Bar 6
            (B4, 1.0), (C5, 0.5), (B4, 0.5), (A4, 0.5), (G4, 0.5),
            // Bar 7
            (FSharp4, 1.0), (G4, 0.5), (A4, 0.5), (B4, 0.5), (G4, 0.5),
            // Bar 8
            (B4, 1.0), (A4, 2.0),
        ];

        (int Note, double Beats)[] leftSeq =
        [
            // Bar 1
            (G3, 2.0), (A3, 1.0),
            // Bar 2
            (B3, 3.0),
            // Bar 3
            (C4, 3.0),
            // Bar 4
            (B3, 3.0),
            // Bar 5
            (A3, 3.0),
            // Bar 6
            (G3, 3.0),
            // Bar 7
            (D4, 1.0), (B3, 1.0), (G3, 1.0),
            // Bar 8
            (D4, 1.0), (D3, 0.5), (C4, 0.5), (B3, 0.5), (A3, 0.5),
        ];

        var rightNotes = BuildSequentialNotes(rightSeq, velocity: 80);
        var leftNotes = BuildSequentialNotes(leftSeq, velocity: 64);

        WriteTwoHandMidi(path, bpm: 108, numerator: 3, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ 4. Gymnopédie No. 1 (Erik Satie)

    private static void WriteGymnopedieNo1(string path)
    {
        var rightNotes = new List<Note>();
        var leftNotes = new List<Note>();

        // 8 bars in 3/4 time: alternating Gmaj7 (odd bars) and Dmaj7 (even bars)
        int[] gMaj7Chord = [B3, D4, FSharp4];
        int[] dMaj7Chord = [A3, CSharp4, FSharp4];

        for (int bar = 0; bar < 8; bar++)
        {
            long barStart = bar * 3L * Ticks;
            bool isGMaj = bar % 2 == 0;
            int bass = isGMaj ? G2 : D2;
            int[] chord = isGMaj ? gMaj7Chord : dMaj7Chord;

            // Beat 1: deep bass note
            leftNotes.Add(new Note((SevenBitNumber)bass, 3 * Ticks - Ticks / 8, barStart)
            {
                Velocity = (SevenBitNumber)56
            });

            // Beat 2: warm floating maj7 chord held for 2 beats
            foreach (int n in chord)
            {
                leftNotes.Add(new Note((SevenBitNumber)n, 2 * Ticks - Ticks / 8, barStart + Ticks)
                {
                    Velocity = (SevenBitNumber)52
                });
            }
        }

        // Satie's melody enters on beat 2 of Bar 1 (4 beats into the piece)
        (int? Note, double Beats)[] rightEvents =
        [
            // Bar 1
            (null, 1.0), (FSharp5, 1.0), (A5, 1.0),
            // Bar 2
            (G5, 1.0), (FSharp5, 1.0), (CSharp5, 1.0),
            // Bar 3
            (B4, 1.0), (CSharp5, 1.0), (D5, 1.0),
            // Bar 4
            (A4, 3.0),
            // Bar 5
            (FSharp4, 3.0),
            // Bar 6
            (null, 1.0), (FSharp5, 1.0), (A5, 1.0),
            // Bar 7
            (G5, 1.0), (FSharp5, 1.0), (CSharp5, 1.0),
            // Bar 8
            (D5, 3.0),
        ];

        long time = 0;
        foreach (var (note, beats) in rightEvents)
        {
            long length = (long)(beats * Ticks);
            if (note is { } pitch)
            {
                rightNotes.Add(new Note((SevenBitNumber)pitch, length - Ticks / 16, time)
                {
                    Velocity = (SevenBitNumber)74
                });
            }
            time += length;
        }

        WriteTwoHandMidi(path, bpm: 72, numerator: 3, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ 5. Für Elise (Theme) (Beethoven)

    private static void WriteFurElise(string path)
    {
        // 8 bars in 3/4 time (24 beats total)
        (int? Note, double Beats)[] rightEvents =
        [
            // Bar 1: iconic E5-D#5 motif
            (E5, 0.5), (DSharp5, 0.5), (E5, 0.5), (DSharp5, 0.5), (E5, 0.5), (B4, 0.5),
            // Bar 2: D5-C5-A4 resolution + pickup
            (D5, 0.5), (C5, 0.5), (A4, 1.0), (C4, 0.5), (E4, 0.5),
            // Bar 3: A4-B4 + E major pickup
            (A4, 0.5), (B4, 1.0), (E4, 0.5), (GSharp4, 0.5), (B4, 0.5),
            // Bar 4: C5 + return to E5-D#5 motif
            (C5, 1.0), (E4, 0.5), (E5, 0.5), (DSharp5, 0.5), (E5, 0.5),
            // Bar 5
            (DSharp5, 0.5), (E5, 0.5), (B4, 0.5), (D5, 0.5), (C5, 0.5), (A4, 0.5),
            // Bar 6
            (C4, 0.5), (E4, 0.5), (A4, 0.5), (B4, 1.0), (E4, 0.5),
            // Bar 7: C5-B4-A4 cadence
            (C5, 0.5), (B4, 0.5), (A4, 2.0),
            // Bar 8: final A minor tonic arpeggio flourish
            (E4, 0.5), (A4, 0.5), (C5, 0.5), (E5, 0.5), (A5, 1.0),
        ];

        (int? Note, double Beats)[] leftEvents =
        [
            // Bar 1
            (null, 3.0),
            // Bar 2: A minor arpeggio
            (A2, 0.5), (E3, 0.5), (A3, 1.0), (null, 1.0),
            // Bar 3: E major arpeggio
            (E2, 0.5), (E3, 0.5), (GSharp3, 1.0), (null, 1.0),
            // Bar 4: A minor arpeggio
            (A2, 0.5), (E3, 0.5), (A3, 1.0), (null, 1.0),
            // Bar 5
            (null, 3.0),
            // Bar 6
            (A2, 0.5), (E3, 0.5), (A3, 1.0), (E2, 1.0),
            // Bar 7
            (E3, 0.5), (GSharp3, 0.5), (A2, 0.5), (E3, 0.5), (A3, 1.0),
            // Bar 8
            (A2, 1.0), (E3, 1.0), (A3, 1.0),
        ];

        var rightNotes = BuildNullableNotes(rightEvents, velocity: 78);
        var leftNotes = BuildNullableNotes(leftEvents, velocity: 60);

        WriteTwoHandMidi(path, bpm: 92, numerator: 3, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ 6. Canon in D (Theme) (Pachelbel)

    private static void WriteCanonInD(string path)
    {
        // 8 bars in 4/4 time (32 beats total)
        // Left hand: D - A - Bm - F#m - G - D - G - A (2 beats per chord = 4bars, repeated twice = 8 bars)
        (int Root, int Fifth, int Tenth)[] progression =
        [
            (D3, A3, FSharp4),      // D major
            (A2, E3, CSharp4),      // A major
            (B2, FSharp3, D4),      // B minor
            (FSharp2, CSharp3, A3), // F# minor
            (G2, D3, B3),           // G major
            (D2, A2, FSharp3),      // D major
            (G2, D3, B3),           // G major
            (A2, E3, CSharp4),      // A major
        ];

        var leftNotes = new List<Note>();
        for (int cycle = 0; cycle < 2; cycle++)
        {
            for (int c = 0; c < progression.Length; c++)
            {
                long chordStart = (cycle * 16L + c * 2L) * Ticks;
                var (root, fifth, tenth) = progression[c];

                // Arpeggiated bass pattern: quarter-note root + fifth/tenth harmony
                leftNotes.Add(new Note((SevenBitNumber)root, 2 * Ticks - Ticks / 8, chordStart)
                {
                    Velocity = (SevenBitNumber)64
                });
                leftNotes.Add(new Note((SevenBitNumber)fifth, Ticks - Ticks / 16, chordStart + Ticks / 2)
                {
                    Velocity = (SevenBitNumber)58
                });
                leftNotes.Add(new Note((SevenBitNumber)tenth, Ticks - Ticks / 16, chordStart + Ticks)
                {
                    Velocity = (SevenBitNumber)60
                });
            }
        }

        // Right hand: Bars 1-4 lyrical half-note descent; Bars 5-8 flowing quarter/eighth variation
        (int Note, double Beats)[] rightSeq =
        [
            // Bars 1-2
            (FSharp5, 2.0), (E5, 2.0),
            (D5, 2.0), (CSharp5, 2.0),
            // Bars 3-4
            (B4, 2.0), (A4, 2.0),
            (B4, 2.0), (CSharp5, 2.0),
            // Bar 5
            (D5, 1.0), (FSharp5, 1.0), (A5, 1.0), (G5, 1.0),
            // Bar 6
            (FSharp5, 1.0), (D5, 1.0), (FSharp5, 1.0), (E5, 1.0),
            // Bar 7
            (D5, 1.0), (B4, 1.0), (D5, 0.5), (E5, 0.5), (FSharp5, 1.0),
            // Bar 8
            (G5, 1.0), (E5, 1.0), (CSharp5, 1.0), (D5, 1.0),
        ];

        var rightNotes = BuildSequentialNotes(rightSeq, velocity: 78);

        WriteTwoHandMidi(path, bpm: 80, numerator: 4, denominator: 4, rightNotes, leftNotes);
    }

    // ------------------------------------------------------------------ Helpers

    private static List<Note> BuildSequentialNotes(IEnumerable<(int Note, double Beats)> seq, int velocity)
    {
        var list = new List<Note>();
        long time = 0;
        foreach (var (note, beats) in seq)
        {
            long length = (long)(beats * Ticks);
            list.Add(new Note((SevenBitNumber)note, Math.Max(Ticks / 8, length - Ticks / 16), time)
            {
                Velocity = (SevenBitNumber)velocity
            });
            time += length;
        }
        return list;
    }

    private static List<Note> BuildNullableNotes(IEnumerable<(int? Note, double Beats)> seq, int velocity)
    {
        var list = new List<Note>();
        long time = 0;
        foreach (var (note, beats) in seq)
        {
            long length = (long)(beats * Ticks);
            if (note is { } pitch)
            {
                list.Add(new Note((SevenBitNumber)pitch, Math.Max(Ticks / 8, length - Ticks / 16), time)
                {
                    Velocity = (SevenBitNumber)velocity
                });
            }
            time += length;
        }
        return list;
    }

    private static void WriteTwoHandMidi(string path, int bpm, byte numerator, byte denominator,
                                         IEnumerable<Note> rightNotes, IEnumerable<Note> leftNotes)
    {
        var rightTrack = rightNotes.ToTrackChunk();
        rightTrack.Events.Insert(0, new SequenceTrackNameEvent("Right hand"));
        rightTrack.Events.Insert(1, new SetTempoEvent(Tempo.FromBeatsPerMinute(bpm).MicrosecondsPerQuarterNote));
        rightTrack.Events.Insert(2, new TimeSignatureEvent(numerator, denominator));

        var leftTrack = leftNotes.ToTrackChunk();
        leftTrack.Events.Insert(0, new SequenceTrackNameEvent("Left hand"));

        var file = new MidiFile(rightTrack, leftTrack)
        {
            TimeDivision = new TicksPerQuarterNoteTimeDivision(Ticks)
        };

        if (Path.GetDirectoryName(path) is { } dir && !string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        file.Write(path, overwriteFile: true);
    }
}
