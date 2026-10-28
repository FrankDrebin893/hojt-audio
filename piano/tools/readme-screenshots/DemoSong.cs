using System.IO;
using Melanchall.DryWetMidi.Common;
using Melanchall.DryWetMidi.Core;
using Melanchall.DryWetMidi.Interaction;

namespace ReadmeScreenshots;

/// <summary>
/// Writes "Ode to Joy" (Beethoven, public domain) as a one-track piano MIDI file for the song
/// practice screenshot: melody above middle C, chords below, so the loader splits it into a
/// right and a left hand. Generated rather than committed, so no third-party file is involved.
/// </summary>
internal static class DemoSong
{
    private const int Ticks = 480;   // per quarter note

    // Melody as (note, quarter notes). E4 = 64.
    private static readonly (int Note, double Beats)[] Melody =
    [
        (64, 1), (64, 1), (65, 1), (67, 1),    (67, 1), (65, 1), (64, 1), (62, 1),
        (60, 1), (60, 1), (62, 1), (64, 1),    (64, 1.5), (62, 0.5), (62, 2),
        (64, 1), (64, 1), (65, 1), (67, 1),    (67, 1), (65, 1), (64, 1), (62, 1),
        (60, 1), (60, 1), (62, 1), (64, 1),    (62, 1.5), (60, 0.5), (60, 2),
        (62, 1), (62, 1), (64, 1), (60, 1),    (62, 1), (64, 0.5), (65, 0.5), (64, 1), (60, 1),
        (62, 1), (64, 0.5), (65, 0.5), (64, 1), (62, 1),    (60, 1), (62, 1), (55, 2),
        (64, 1), (64, 1), (65, 1), (67, 1),    (67, 1), (65, 1), (64, 1), (62, 1),
        (60, 1), (60, 1), (62, 1), (64, 1),    (62, 1.5), (60, 0.5), (60, 2),
    ];

    private static readonly int[] C = [48, 52, 55];
    private static readonly int[] G = [43, 47, 50];

    // Left hand: one chord per half bar.
    private static readonly int[][] Chords =
    [
        C, C,  G, G,  C, C,  G, G,
        C, C,  G, G,  C, C,  G, C,
        G, C,  G, C,  G, G,  C, G,
        C, C,  G, G,  C, C,  G, C,
    ];

    public static string Write(string folder)
    {
        var notes = new List<Note>();

        long time = 0;
        foreach (var (note, beats) in Melody)
        {
            long length = (long)(beats * Ticks);
            // A little shorter than written, so repeated notes are visibly separate as they fall.
            notes.Add(new Note((SevenBitNumber)note, length - Ticks / 16, time) { Velocity = (SevenBitNumber)80 });
            time += length;
        }

        for (int i = 0; i < Chords.Length; i++)
        {
            foreach (var note in Chords[i])
                notes.Add(new Note((SevenBitNumber)note, 2 * Ticks - Ticks / 8, i * 2L * Ticks) { Velocity = (SevenBitNumber)60 });
        }

        var track = notes.ToTrackChunk();
        track.Events.Insert(0, new SequenceTrackNameEvent("Piano"));
        track.Events.Insert(1, new SetTempoEvent(Tempo.FromBeatsPerMinute(100).MicrosecondsPerQuarterNote));
        track.Events.Insert(2, new TimeSignatureEvent(4, 4));

        var file = new MidiFile(track) { TimeDivision = new TicksPerQuarterNoteTimeDivision(Ticks) };

        Directory.CreateDirectory(folder);
        string path = Path.Combine(folder, "Ode to Joy.mid");
        file.Write(path, overwriteFile: true);
        return path;
    }
}
