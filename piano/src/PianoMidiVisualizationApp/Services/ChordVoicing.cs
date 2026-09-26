using System.Numerics;

namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// Which chord member is in the bass. Kept apart from the display text so figured bass
/// (I6, V65...) can be derived without parsing <see cref="ChordAnalysis.Voicing"/>.
/// </summary>
public enum ChordInversion
{
    /// <summary>
    /// No inversion applies: a single note, a dyad, an unnamed note list, or a bass that
    /// isn't the root, 3rd, 5th or 7th (a sus chord's 2nd or 4th, an added 9th...).
    /// </summary>
    None,
    RootPosition,
    First,
    Second,
    Third,
}

/// <summary>
/// Describes how a named chord is voiced: which member is in the bass, and whether the
/// notes form a close, open, shell or drop 2 voicing. Only says what it can say for certain.
/// </summary>
public static class ChordVoicing
{
    private const string Separator = " · ";

    /// <summary>Which member of the chord rooted on <paramref name="rootPitchClass"/> is in the bass.</summary>
    /// <param name="pitchClassMask">Bit n set when pitch class n (C = 0) is held.</param>
    public static ChordInversion InversionOf(int rootPitchClass, int pitchClassMask, int bassPitchClass)
    {
        int intervals = IntervalsAbove(rootPitchClass, pitchClassMask);
        if (!IsChord(intervals))
            return ChordInversion.None;

        // A semitone distance counts as a chord member only when nothing else in the chord
        // claims it; otherwise it's an extension, and calling it an inversion would be wrong.
        return Above(rootPitchClass, bassPitchClass) switch
        {
            0 => ChordInversion.RootPosition,
            4 => ChordInversion.First,
            3 when !Has(intervals, 4) => ChordInversion.First,     // else a #9
            7 => ChordInversion.Second,
            6 when IsFlatFifth(intervals) => ChordInversion.Second,
            8 when IsSharpFifth(intervals) => ChordInversion.Second,
            10 => ChordInversion.Third,
            11 when !Has(intervals, 10) => ChordInversion.Third,
            9 when IsDiminishedSeventh(intervals) => ChordInversion.Third,
            _ => ChordInversion.None,
        };
    }

    /// <summary>
    /// The readout line, e.g. "1st inversion · close" or "4th in bass · open". Empty when
    /// the notes are too few to call a chord.
    /// </summary>
    /// <param name="sortedNotes">The held MIDI notes, lowest first.</param>
    public static string Describe(IReadOnlyList<int> sortedNotes, int rootPitchClass)
    {
        if (sortedNotes.Count == 0)
            return "";

        int mask = 0;
        foreach (int note in sortedNotes)
            mask |= 1 << MusicNaming.PitchClassOf(note);

        int intervals = IntervalsAbove(rootPitchClass, mask);
        if (!IsChord(intervals))
            return "";

        int bassPitchClass = MusicNaming.PitchClassOf(sortedNotes[0]);
        string? position = InversionOf(rootPitchClass, mask, bassPitchClass) switch
        {
            ChordInversion.RootPosition => "Root position",
            ChordInversion.First => "1st inversion",
            ChordInversion.Second => "2nd inversion",
            ChordInversion.Third => "3rd inversion",
            _ => BassPhrase(Above(rootPitchClass, bassPitchClass), intervals),
        };

        string tag = VoicingTag(sortedNotes, intervals);
        return position is null ? tag : position + Separator + tag;
    }

    /// <summary>
    /// Names a bass that isn't an inversion by what it is to the chord: a sus chord's 2nd or
    /// 4th, or an extension. Null when even that would be a guess.
    /// </summary>
    private static string? BassPhrase(int bassInterval, int intervals)
    {
        bool hasThird = Has(intervals, 3) || Has(intervals, 4);
        bool hasSeventh = Has(intervals, 10) || Has(intervals, 11);

        return bassInterval switch
        {
            1 => "b9 in bass",
            2 => hasThird ? "9th in bass" : "2nd in bass",
            3 => "#9 in bass",
            5 => hasThird ? "11th in bass" : "4th in bass",
            6 => "#11 in bass",
            8 => hasSeventh ? "b13 in bass" : "b6 in bass",
            9 => hasSeventh ? "13th in bass" : "6th in bass",
            _ => null,
        };
    }

    /// <summary>
    /// One tag, most specific first: drop 2 and shell are kinds of open or close voicing,
    /// so they replace that tag rather than join it.
    /// </summary>
    private static string VoicingTag(IReadOnlyList<int> sortedNotes, int intervals)
    {
        if (IsDropTwo(sortedNotes)) return "drop 2";
        if (IsShell(intervals)) return "shell";
        return sortedNotes[^1] - sortedNotes[0] <= 12 ? "close" : "open";
    }

    /// <summary>
    /// Four different notes that close up into a single octave when the bass goes up an
    /// octave and lands second from the top — i.e. the second-highest voice of a close
    /// voicing was dropped an octave.
    /// </summary>
    private static bool IsDropTwo(IReadOnlyList<int> n)
    {
        if (n.Count != 4) return false;

        int mask = 0;
        foreach (int note in n)
            mask |= 1 << MusicNaming.PitchClassOf(note);
        if (BitOperations.PopCount((uint)mask) != 4) return false;

        int raised = n[0] + 12;
        return n[2] < raised && raised < n[3]   // lands second from the top...
            && n[3] - n[1] < 12;                // ...and the result is close position
    }

    /// <summary>Root, 3rd and 7th and nothing else (doublings allowed).</summary>
    private static bool IsShell(int intervals) =>
        BitOperations.PopCount((uint)intervals) == 3
        && (Has(intervals, 3) || Has(intervals, 4))
        && (Has(intervals, 10) || Has(intervals, 11));

    /// <summary>
    /// A b5 standing in for the 5th: no perfect 5th, and either a minor 3rd (dim, m7b5) or no
    /// major 7th — beside a major 3rd and major 7th it's a Lydian #11 (Cmaj7(#11) omits the 5th).
    /// </summary>
    private static bool IsFlatFifth(int intervals) =>
        !Has(intervals, 7) && (Has(intervals, 3) || !Has(intervals, 11));

    /// <summary>
    /// A #5 standing in for the 5th: an augmented sound, so a major 3rd and no perfect 5th.
    /// Beside a natural 6th it's a b6/b13 colour tone instead (m6/9, 6/9 voicings).
    /// </summary>
    private static bool IsSharpFifth(int intervals) =>
        !Has(intervals, 7) && Has(intervals, 4) && !Has(intervals, 9);

    private static bool IsDiminishedSeventh(int intervals) =>
        Has(intervals, 3) && Has(intervals, 6) && Has(intervals, 9)
        && !Has(intervals, 4) && !Has(intervals, 7) && !Has(intervals, 10) && !Has(intervals, 11);

    /// <summary>
    /// Three or more pitch classes including the root. Inversions and voicings aren't
    /// meaningful for fewer, and a root that isn't held means the name can't be trusted.
    /// </summary>
    private static bool IsChord(int intervals) =>
        Has(intervals, 0) && BitOperations.PopCount((uint)intervals) >= 3;

    /// <summary>Re-bases a pitch-class mask so bit n means "n semitones above the root".</summary>
    private static int IntervalsAbove(int rootPitchClass, int pitchClassMask)
    {
        int intervals = 0;
        for (int pc = 0; pc < 12; pc++)
        {
            if ((pitchClassMask & (1 << pc)) != 0)
                intervals |= 1 << Above(rootPitchClass, pc);
        }
        return intervals;
    }

    private static int Above(int rootPitchClass, int pitchClass) =>
        ((pitchClass - rootPitchClass) % 12 + 12) % 12;

    private static bool Has(int intervals, int semitones) => (intervals & (1 << semitones)) != 0;
}
