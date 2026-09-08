namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// Turns the set of currently held notes into what the readout shows: chord name,
/// the held notes, and each note's interval above the chord root.
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

    public ChordAnalysis Analyze(IEnumerable<int> midiNoteNumbers)
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
            noteTokens[i] = MusicNaming.WithOctave(notes[i]);

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

        return new ChordAnalysis(
            chord.Name,
            string.Join(" ", noteTokens).TrimEnd(),
            string.Join(" ", intervalTokens).TrimEnd());
    }
}
