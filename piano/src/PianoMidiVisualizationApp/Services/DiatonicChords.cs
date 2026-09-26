namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// One chord built on a scale degree by stacking the scale's own thirds.
/// </summary>
/// <param name="Degree">1-based scale degree of the root, tonic = 1.</param>
/// <param name="RootPitchClass">The chord root, 0-11.</param>
/// <param name="Intervals">Semitones above the root, root first, in stacking order.</param>
/// <param name="PitchClassMask">12-bit set of the chord's pitch classes, bit 0 = C.</param>
/// <param name="Name">Chord symbol with a letter-correct root, e.g. "C#dim" in D harmonic minor.</param>
/// <param name="Numeral">The key-relative numeral, from <see cref="RomanNumeralAnalyzer"/>.</param>
/// <param name="Voicing">
/// A root-position close voicing around middle C, lowest first: what the strip hints on the
/// keyboard and plays when auditioned.
/// </param>
/// <param name="NoteNames">The voicing's notes spelled for the key, without octaves.</param>
public sealed record DiatonicChord(
    int Degree,
    int RootPitchClass,
    IReadOnlyList<int> Intervals,
    int PitchClassMask,
    string Name,
    RomanNumeral Numeral,
    IReadOnlyList<int> Voicing,
    IReadOnlyList<string> NoteNames);

/// <summary>
/// A key's diatonic chords and the scale they were stacked from. For a heptatonic key that is
/// the key itself; a pentatonic or blues key borrows its parent's (see <see cref="IsFromParentScale"/>).
/// </summary>
public sealed record DiatonicChordSet(
    MusicKey Key,
    MusicKey HarmonyKey,
    bool Sevenths,
    IReadOnlyList<DiatonicChord> Chords)
{
    public bool IsFromParentScale => HarmonyKey != Key;
}

/// <summary>
/// Builds a key's diatonic chords, and decides which of them the held notes are.
/// </summary>
public static class DiatonicChords
{
    private static readonly string[] LetterNames = { "C", "D", "E", "F", "G", "A", "B" };

    /// <summary>Chord-symbol suffixes, keyed by the interval set above the root (bit 0 = root).</summary>
    /// <remarks>
    /// Written the way <see cref="ChordDetector"/> names these chords, so a lit tile reads like
    /// the readout above it: "Bdim" rather than "B°", "AmM7" rather than "Am(maj7)".
    /// </remarks>
    private static readonly Dictionary<int, string> Suffixes = new()
    {
        [Mask(0, 4, 7)] = "",
        [Mask(0, 3, 7)] = "m",
        [Mask(0, 3, 6)] = "dim",
        [Mask(0, 4, 8)] = "aug",
        [Mask(0, 4, 7, 11)] = "maj7",
        [Mask(0, 4, 7, 10)] = "7",
        [Mask(0, 3, 7, 10)] = "m7",
        [Mask(0, 3, 6, 10)] = "m7b5",
        [Mask(0, 3, 6, 9)] = "dim7",
        [Mask(0, 3, 7, 11)] = "mM7",
        [Mask(0, 4, 8, 11)] = "maj7#5",
    };

    /// <summary>Lowest root of the close voicing: F3, so every root lands between F3 and E4.</summary>
    private const int LowestVoicingRoot = 53;

    /// <summary>
    /// The seven-note scale a key's chords are stacked from. A heptatonic key uses its own. A
    /// pentatonic or blues scale is too sparse to stack thirds in, so it uses the parent scale on
    /// the same tonic: major pentatonic the major, minor pentatonic and blues the natural minor.
    /// These are the same parents <see cref="RomanNumeralAnalyzer"/> takes its numerals from.
    /// </summary>
    public static MusicKey HarmonyKeyFor(MusicKey key) =>
        key.IsHeptatonic
            ? key
            : key.Scale == ScaleType.MajorPentatonic
                ? new MusicKey(key.Tonic, ScaleType.Major)
                : new MusicKey(key.Tonic, ScaleType.NaturalMinor);

    /// <summary>Stacks a triad (or seventh chord) of the scale's own thirds on every degree.</summary>
    public static DiatonicChordSet Build(MusicKey key, bool sevenths)
    {
        var harmonyKey = HarmonyKeyFor(key);
        var steps = harmonyKey.Steps;
        int chordSize = sevenths ? 4 : 3;
        var chords = new List<DiatonicChord>(steps.Count);

        for (int degree = 0; degree < steps.Count; degree++)
        {
            int root = (harmonyKey.Tonic + steps[degree]) % 12;
            var intervals = new int[chordSize];
            int mask = 0;

            // Every other scale note above the root: its third, fifth, then seventh.
            for (int i = 0; i < chordSize; i++)
            {
                int step = steps[(degree + (2 * i)) % steps.Count];
                intervals[i] = Wrap(step - steps[degree]);
                mask |= 1 << Wrap(root + intervals[i]);
            }

            var voicing = CloseVoicing(root, intervals);

            // Spelled in the selected key, exactly as the grand staff spells held notes, so the
            // tile and the staff agree: C# (not Db) in D harmonic minor, E# in F# major. The
            // root comes first because the voicing is in root position.
            var spelled = NoteSpeller.SpellChord(voicing, key);
            var noteNames = spelled.Select(PitchName).ToArray();
            string name = noteNames[0] + SuffixFor(intervals);

            chords.Add(new DiatonicChord(
                degree + 1, root, intervals, mask, name,
                RomanNumeralAnalyzer.Analyze(root, mask, harmonyKey),
                voicing, noteNames));
        }

        return new DiatonicChordSet(key, harmonyKey, sevenths, chords);
    }

    /// <summary>The chord-symbol suffix for a set of intervals above the root, e.g. "m7b5".</summary>
    public static string SuffixFor(IReadOnlyList<int> intervals)
    {
        int intervalMask = 0;
        foreach (int interval in intervals)
            intervalMask |= 1 << Wrap(interval);

        // Every triad and seventh the heptatonic scales stack is in the table; this is only a
        // guard against a scale added later producing something new.
        return Suffixes.TryGetValue(intervalMask, out var suffix)
            ? suffix
            : "(" + string.Join(",", intervals) + ")";
    }

    /// <summary>
    /// Whether the held notes are this chord: exactly its pitch classes, in any octave, any
    /// inversion, with any doublings, and recognised by the readout as a chord with a root.
    /// </summary>
    /// <remarks>
    /// The readout's root is deliberately not required to be the tile's. The detector names a
    /// chord from its bass, so it calls Am7 over C "C6" and an inverted augmented or diminished-
    /// seventh chord after whichever note is lowest; requiring its root would stop exactly those
    /// inversions from lighting. Nothing is lost by it: no two chords in a key's strip share a
    /// pitch-class set, so the set alone pins down one tile.
    /// </remarks>
    public static bool Matches(DiatonicChord chord, int? playedRootPitchClass, int heldPitchClassMask) =>
        playedRootPitchClass is { } root
        && heldPitchClassMask == chord.PitchClassMask
        && (heldPitchClassMask & (1 << Wrap(root))) != 0;

    /// <summary>12-bit set of the pitch classes of some MIDI notes, bit 0 = C.</summary>
    public static int MaskOf(IEnumerable<int> midiNotes)
    {
        int mask = 0;
        foreach (int note in midiNotes)
            mask |= 1 << Wrap(note);
        return mask;
    }

    /// <summary>
    /// Root position, each tone stacked on the last, rooted between F3 and E4 so the chord sits
    /// around middle C: triads reach no higher than C5, sevenths D#5.
    /// </summary>
    private static int[] CloseVoicing(int rootPitchClass, IReadOnlyList<int> intervals)
    {
        int root = LowestVoicingRoot + Wrap(rootPitchClass - LowestVoicingRoot);
        return intervals.Select(interval => root + interval).ToArray();
    }

    /// <summary>A spelled note's name without its octave, e.g. "C#" or "Fx".</summary>
    public static string PitchName(SpelledNote note) =>
        LetterNames[note.Letter] + SpelledNote.AccidentalText(note.Accidental);

    private static int Mask(params int[] intervals) => MaskOf(intervals);

    private static int Wrap(int value) => ((value % 12) + 12) % 12;
}
