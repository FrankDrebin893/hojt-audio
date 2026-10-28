namespace PianoMidiVisualizationApp.Services.SongPractice;

/// <summary>A set of notes wait mode treats as one thing to play.</summary>
/// <param name="Notes">Distinct pitches, ascending, as written (not folded into the keyboard's range).</param>
public sealed record PracticeChord(int Index, TimeSpan Time, IReadOnlyList<int> Notes);

public static class ChordGrouper
{
    /// <summary>
    /// Notes from a human performance rarely start on the same tick; this is loose enough to
    /// catch a played chord and tight enough not to swallow a fast grace note or arpeggio.
    /// </summary>
    public static readonly TimeSpan DefaultWindow = TimeSpan.FromMilliseconds(30);

    /// <summary>
    /// Groups notes into chords. Each chord is anchored on its first note and takes every
    /// note starting within <paramref name="window"/> of it, so a chord never spreads wider
    /// than the window — chaining would let a rolled run of notes merge into one huge chord.
    /// </summary>
    public static List<PracticeChord> Group(IEnumerable<SongNote> notes, TimeSpan window)
    {
        var sorted = notes.OrderBy(n => n.Start).ThenBy(n => n.Note).ToList();
        var chords = new List<PracticeChord>();

        int i = 0;
        while (i < sorted.Count)
        {
            var anchor = sorted[i].Start;
            var pitches = new SortedSet<int>();
            while (i < sorted.Count && sorted[i].Start - anchor <= window)
            {
                pitches.Add(sorted[i].Note);
                i++;
            }
            chords.Add(new PracticeChord(chords.Count, anchor, pitches.ToList()));
        }

        return chords;
    }
}
