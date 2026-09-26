using System.IO;
using System.Text.RegularExpressions;
using Melanchall.DryWetMidi.Core;
using Melanchall.DryWetMidi.Interaction;
using Melanchall.DryWetMidi.Standards;

namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>
/// Reads a Standard MIDI File into a <see cref="Song"/>: notes resolved to real time through
/// the file's tempo map, grouped into parts, with practice roles picked for a pianist.
/// </summary>
public static partial class MidiSongLoader
{
    private const int DrumChannel = 9;

    /// <summary>Middle C. A lone piano track is split into hands here.</summary>
    public const int HandSplitNote = 60;

    /// <summary>Guards the bar walk against a corrupt tempo map that never reaches the end.</summary>
    private const int MaxBars = 20_000;

    /// <summary>
    /// Files in the wild are often slightly malformed (wrong chunk sizes, a missing
    /// end-of-track). Reading what is there beats refusing a file that plays fine elsewhere.
    /// </summary>
    private static readonly ReadingSettings LenientReading = new()
    {
        InvalidChunkSizePolicy = InvalidChunkSizePolicy.Ignore,
        NotEnoughBytesPolicy = NotEnoughBytesPolicy.Ignore,
        NoHeaderChunkPolicy = NoHeaderChunkPolicy.Ignore,
        MissedEndOfTrackPolicy = MissedEndOfTrackPolicy.Ignore,
        UnexpectedTrackChunksCountPolicy = UnexpectedTrackChunksCountPolicy.Ignore,
        InvalidChannelEventParameterValuePolicy = InvalidChannelEventParameterValuePolicy.ReadValid,
        InvalidMetaEventParameterValuePolicy = InvalidMetaEventParameterValuePolicy.SnapToLimits,
    };

    public static Song Load(string path) =>
        FromMidiFile(MidiFile.Read(path, LenientReading), Path.GetFileNameWithoutExtension(path));

    public static Song FromMidiFile(MidiFile file, string title)
    {
        var tempoMap = file.GetTempoMap();
        var chunks = file.GetTrackChunks().ToList();

        var strands = new List<Strand>();
        for (int c = 0; c < chunks.Count; c++)
        {
            var chunk = chunks[c];
            var notes = chunk.GetNotes();
            if (notes.Count == 0) continue;

            string? trackName = chunk.Events.OfType<SequenceTrackNameEvent>()
                .Select(e => e.Text?.Trim())
                .FirstOrDefault(t => !string.IsNullOrEmpty(t));

            // A format-0 file keeps every instrument in one track, told apart only by channel.
            var byChannel = notes.GroupBy(n => (int)n.Channel).OrderBy(g => g.Key).ToList();
            foreach (var group in byChannel)
            {
                int channel = group.Key;
                bool isDrums = channel == DrumChannel;
                int? program = ProgramOf(chunk, channel) ?? chunks.Select(ch => ProgramOf(ch, channel)).FirstOrDefault(p => p != null);

                string name = (trackName, byChannel.Count > 1) switch
                {
                    (null, false) => $"Track {c + 1}",
                    (null, true) => $"Channel {channel + 1}",
                    (_, false) => trackName,
                    (_, true) => $"{trackName} (ch {channel + 1})"
                };

                strands.Add(new Strand(
                    name,
                    isDrums ? "Drums" : InstrumentName(program ?? 0),
                    isDrums,
                    // No program change at all means the GM default, which is a grand piano.
                    IsPiano: !isDrums && (program ?? 0) <= 7,
                    group.Select(n => (
                            Note: (int)n.NoteNumber,
                            Velocity: (int)n.Velocity,
                            Start: ToTimeSpan(n.TimeAs<MetricTimeSpan>(tempoMap)),
                            Duration: ToTimeSpan(n.LengthAs<MetricTimeSpan>(tempoMap))))
                        .ToList()));
            }
        }

        if (strands.Count == 0)
            throw new InvalidDataException("This MIDI file has no notes.");

        var (parts, songNotes) = BuildParts(strands);
        songNotes.Sort((a, b) => a.Start != b.Start ? a.Start.CompareTo(b.Start) : a.Note.CompareTo(b.Note));

        var duration = songNotes.Count == 0 ? TimeSpan.Zero : songNotes.Max(n => n.End);
        double initialBpm = tempoMap.GetTempoAtTime(new MidiTimeSpan(0)).BeatsPerMinute;

        return new Song(title, parts, songNotes, BarStarts(tempoMap, duration), initialBpm);
    }

    /// <summary>
    /// Picks roles for a pianist: the piano parts (at most two) are yours, everything else is
    /// accompaniment, and drums are muted since app-played notes all sound as piano. A lone
    /// piano part is split into hands at middle C so each hand can be practised on its own.
    /// </summary>
    private static (List<SongPart> Parts, List<SongNote> Notes) BuildParts(List<Strand> strands)
    {
        var melodic = strands.Where(s => !s.IsDrums).ToList();
        var pianos = melodic.Where(s => s.IsPiano).ToList();

        var yours = (pianos.Count > 0 ? pianos : melodic)
            .OrderByDescending(s => s.Notes.Count)
            .Take(pianos.Count > 0 ? 2 : 1)
            .ToHashSet();

        // With two parts of yours, the higher one is the right hand, coloured to match.
        Strand? rightHand = yours.Count == 2 ? yours.MaxBy(s => s.Notes.Average(n => n.Note)) : null;

        var parts = new List<SongPart>();
        var notes = new List<SongNote>();
        int nextOtherColor = 2;

        void AddPart(string name, Strand s, IEnumerable<(int Note, int Velocity, TimeSpan Start, TimeSpan Duration)> partNotes,
                     PartRole role, int colorIndex)
        {
            int index = parts.Count;
            int count = 0;
            foreach (var n in partNotes)
            {
                notes.Add(new SongNote(index, n.Note, n.Velocity, n.Start, n.Duration));
                count++;
            }
            parts.Add(new SongPart(index, name, s.Instrument, count, s.IsDrums, s.IsPiano, role, colorIndex));
        }

        foreach (var s in strands)
        {
            if (yours.Contains(s))
            {
                if (yours.Count == 1 && s.IsPiano && SpansBothHands(s))
                {
                    AddPart($"{s.Name} · right hand", s, s.Notes.Where(n => n.Note >= HandSplitNote), PartRole.YouPlay, 0);
                    AddPart($"{s.Name} · left hand", s, s.Notes.Where(n => n.Note < HandSplitNote), PartRole.YouPlay, 1);
                }
                else
                {
                    AddPart(s.Name, s, s.Notes, PartRole.YouPlay,
                            yours.Count == 2 && s != rightHand ? 1 : 0);
                }
            }
            else
            {
                AddPart(s.Name, s, s.Notes, s.IsDrums ? PartRole.Mute : PartRole.AutoPlay, nextOtherColor++);
            }
        }

        return (parts, notes);
    }

    /// <summary>Worth splitting only if each hand gets a real share, not one stray note.</summary>
    private static bool SpansBothHands(Strand s)
    {
        int right = s.Notes.Count(n => n.Note >= HandSplitNote);
        int left = s.Notes.Count - right;
        int minimum = Math.Max(1, s.Notes.Count / 20);
        return right >= minimum && left >= minimum;
    }

    /// <summary>
    /// Bar boundaries through the tempo map and any time-signature changes, walking until a
    /// bar starts at or after the last note ends. Always yields at least one whole bar.
    /// </summary>
    private static List<TimeSpan> BarStarts(TempoMap tempoMap, TimeSpan end)
    {
        var starts = new List<TimeSpan>();
        for (int bar = 0; bar <= MaxBars; bar++)
        {
            long ticks = TimeConverter.ConvertFrom(new BarBeatTicksTimeSpan(bar, 0, 0), tempoMap);
            var start = ToTimeSpan(TimeConverter.ConvertTo<MetricTimeSpan>(ticks, tempoMap));
            starts.Add(start);
            if (bar >= 1 && start >= end) break;
        }
        return starts;
    }

    private static int? ProgramOf(TrackChunk chunk, int channel) =>
        chunk.Events.OfType<ProgramChangeEvent>()
            .Where(e => e.Channel == channel)
            .Select(e => (int?)(int)e.ProgramNumber)
            .FirstOrDefault();

    private static TimeSpan ToTimeSpan(MetricTimeSpan metric) =>
        TimeSpan.FromTicks(metric.TotalMicroseconds * 10);

    /// <summary>"AcousticGrandPiano" → "Acoustic Grand Piano", "ElectricPiano1" → "Electric Piano 1".</summary>
    private static string InstrumentName(int program) =>
        program is >= 0 and <= 127
            ? CamelCaseBoundary().Replace(((GeneralMidiProgram)program).ToString(), " ")
            : $"Program {program}";

    [GeneratedRegex(@"(?<=[a-z])(?=[A-Z0-9])|(?<=[0-9])(?=[A-Z])")]
    private static partial Regex CamelCaseBoundary();

    private sealed record Strand(
        string Name,
        string Instrument,
        bool IsDrums,
        bool IsPiano,
        List<(int Note, int Velocity, TimeSpan Start, TimeSpan Duration)> Notes);
}
