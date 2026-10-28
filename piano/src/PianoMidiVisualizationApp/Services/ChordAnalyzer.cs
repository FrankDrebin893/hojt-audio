namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// Turns the set of currently held notes into what the readout shows: chord name,
/// inversion and voicing, the held notes, and each note's interval above the chord root.
/// </summary>
public class ChordAnalyzer
{
    private readonly ChordDetector _detector = new();

    /// <summary>Interval names for a note within an octave of the root.</summary>
    private static readonly string[] SimpleLabels =
        { "R", "b2", "2", "b3", "3", "4", "b5", "5", "#5", "6", "b7", "7" };

    /// <summary>Interval names for a note more than an octave above the root.</summary>
    private static readonly string[] CompoundLabels =
        { "R", "b9", "9", "b3", "3", "11", "#11", "5", "b13", "13", "b7", "7" };

    /// <param name="useFlats">
    /// Spell note and root names with flats, as the selected key signature requires.
    /// Interval labels are relative to the chord root and never change with the key.
    /// </param>
    /// <param name="key">The selected key, for the Roman numeral. None leaves it empty.</param>
    public ChordAnalysis Analyze(IEnumerable<int> midiNoteNumbers, bool useFlats = false, MusicKey? key = null)
    {
        var notes = midiNoteNumbers.ToList();
        if (notes.Count == 0)
            return ChordAnalysis.Empty;

        notes.Sort();

        var detected = _detector.Detect(notes);
        if (detected is not { } chord)
            return ChordAnalysis.Empty;

        int rootPitchClass = chord.RootPitchClass;

        // Measure octave distance from the lowest sounding instance of the root, so that a
        // 9th reads as "9" but the same pitch class an octave lower reads as "2".
        int rootReference = notes.FirstOrDefault(
            n => MusicNaming.PitchClassOf(n) == rootPitchClass,
            notes[0]);

        var noteTokens = new string[notes.Count];
        var intervalTokens = new string[notes.Count];

        for (int i = 0; i < notes.Count; i++)
        {
            noteTokens[i] = MusicNaming.WithOctave(notes[i], useFlats);

            int semitones = ((MusicNaming.PitchClassOf(notes[i]) - rootPitchClass) % 12 + 12) % 12;
            bool isCompound = notes[i] - rootReference > 12;
            intervalTokens[i] = isCompound ? CompoundLabels[semitones] : SimpleLabels[semitones];
        }

        // Pad both rows to a common per-column width so the tokens line up in a monospace font.
        for (int i = 0; i < notes.Count; i++)
        {
            int width = Math.Max(noteTokens[i].Length, intervalTokens[i].Length);
            noteTokens[i] = noteTokens[i].PadRight(width);
            intervalTokens[i] = intervalTokens[i].PadRight(width);
        }

        // A note list's "root" is only its bass, so it has no inversion, voicing or numeral to report.
        var inversion = ChordInversion.None;
        string voicing = "";
        var numeral = RomanNumeral.None;
        if (chord.IsChord)
        {
            int mask = 0;
            foreach (var n in notes)
                mask |= 1 << MusicNaming.PitchClassOf(n);
            int bassPitchClass = MusicNaming.PitchClassOf(notes[0]);

            inversion = ChordVoicing.InversionOf(rootPitchClass, mask, bassPitchClass);
            voicing = ChordVoicing.Describe(notes, rootPitchClass);

            // Figured from the bass itself rather than from Inversion: the analyser reads a
            // diminished seventh from its leading tone, and the figure has to follow that root.
            if (key is { } k)
                numeral = RomanNumeralAnalyzer.Analyze(rootPitchClass, mask, bassPitchClass, k);
        }

        return new ChordAnalysis(
            MusicNaming.Respell(chord.Name, useFlats),
            string.Join(" ", noteTokens).TrimEnd(),
            string.Join(" ", intervalTokens).TrimEnd(),
            rootPitchClass,
            voicing)
        {
            Inversion = inversion,
            Function = numeral.Text,
            FunctionKind = numeral.Kind,
        };
    }
}
