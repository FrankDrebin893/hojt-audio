using Melanchall.DryWetMidi.Common;
using Melanchall.DryWetMidi.Core;
using PianoMidiVisualizationApp.Services.Playback;

namespace PianoMidiVisualizationApp.Services.Recording;

/// <summary>
/// Writes a take as a single-track Standard MIDI File. The notes were played in real time,
/// not to a grid, so ticks are derived from seconds at the written tempo: a DAW opening the
/// file at that tempo plays every note exactly when it was played.
/// </summary>
public static class TakeMidiExporter
{
    /// <summary>
    /// 960 ticks per quarter note is about half a millisecond per tick at 120 BPM, well inside
    /// what anyone can hear, and a resolution every DAW reads.
    /// </summary>
    public const short TicksPerQuarterNote = 960;

    public const int DefaultBpm = 120;

    public static void Export(Take take, string path, int bpm)
    {
        var file = ToMidiFile(take.Notes, bpm, take.Name);
        file.Write(path, overwriteFile: true, format: MidiFileFormat.SingleTrack);
    }

    public static MidiFile ToMidiFile(IReadOnlyList<TimedNote> notes, int bpm, string? trackName = null)
    {
        bpm = Math.Clamp(bpm, 20, 400);

        var timed = new List<(long Tick, int Order, MidiEvent Event)>(notes.Count * 2 + 2);

        if (!string.IsNullOrEmpty(trackName))
            timed.Add((0, 0, new SequenceTrackNameEvent(trackName)));
        timed.Add((0, 0, new SetTempoEvent(60_000_000L / bpm)));

        foreach (var note in notes)
        {
            if (note.Note is < 0 or > 127) continue;

            long on = ToTicks(note.Start, bpm);
            // A note must end at least a tick after it starts, or a reader drops it.
            long off = Math.Max(ToTicks(note.End, bpm), on + 1);
            var number = (SevenBitNumber)note.Note;

            // At the same tick a note-off goes before a note-on, so a key struck again the
            // instant it was released re-sounds instead of being cut off by its old release.
            timed.Add((on, 2, new NoteOnEvent(number, (SevenBitNumber)Math.Clamp(note.Velocity, 1, 127))));
            timed.Add((off, 1, new NoteOffEvent(number, (SevenBitNumber)0)));
        }

        var track = new TrackChunk();
        long previous = 0;
        foreach (var (tick, _, midiEvent) in timed.OrderBy(t => t.Tick).ThenBy(t => t.Order))
        {
            midiEvent.DeltaTime = tick - previous;
            previous = tick;
            track.Events.Add(midiEvent);
        }

        return new MidiFile(track)
        {
            TimeDivision = new TicksPerQuarterNoteTimeDivision(TicksPerQuarterNote)
        };
    }

    /// <summary>Real time to ticks at a constant tempo, rounded to the nearest tick.</summary>
    public static long ToTicks(TimeSpan time, int bpm) =>
        (long)Math.Round(time.TotalSeconds * bpm / 60.0 * TicksPerQuarterNote);
}
