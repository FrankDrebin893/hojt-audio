namespace PianoMidiVisualizationApp.Services;

public class ChordDetector
{
    private static readonly string[] NoteNames = { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" };

    private static readonly (string Name, int[] Intervals)[] ChordPatterns =
    {
        // 4-note chords (check first for specificity)
        ("maj7", new[] { 0, 4, 7, 11 }),
        ("m7", new[] { 0, 3, 7, 10 }),
        ("7", new[] { 0, 4, 7, 10 }),
        ("dim7", new[] { 0, 3, 6, 9 }),
        ("m7b5", new[] { 0, 3, 6, 10 }),

        // 3-note chords
        ("", new[] { 0, 4, 7 }),       // Major
        ("m", new[] { 0, 3, 7 }),      // Minor
        ("dim", new[] { 0, 3, 6 }),    // Diminished
        ("aug", new[] { 0, 4, 8 }),    // Augmented
        ("sus4", new[] { 0, 5, 7 }),   // Suspended 4th
        ("sus2", new[] { 0, 2, 7 }),   // Suspended 2nd

        // 2-note chords
        ("5", new[] { 0, 7 }),         // Power chord
    };

    public string? Detect(IEnumerable<int> midiNoteNumbers)
    {
        var notes = midiNoteNumbers.ToList();

        // Need at least 2 notes for a chord
        if (notes.Count < 2)
            return null;

        // Sort notes to find bass note
        notes.Sort();
        int bassNote = notes[0];

        // Convert to unique pitch classes (0-11)
        var pitchClasses = notes
            .Select(n => n % 12)
            .Distinct()
            .OrderBy(p => p)
            .ToList();

        // Need at least 2 unique pitch classes
        if (pitchClasses.Count < 2)
            return null;

        // Try each pitch class as potential root
        foreach (var pattern in ChordPatterns)
        {
            for (int rootPitchClass = 0; rootPitchClass < 12; rootPitchClass++)
            {
                // Calculate expected pitch classes for this root
                var expectedPitchClasses = pattern.Intervals
                    .Select(interval => (rootPitchClass + interval) % 12)
                    .OrderBy(p => p)
                    .ToList();

                // Check if pitch classes match
                if (pitchClasses.SequenceEqual(expectedPitchClasses))
                {
                    string rootName = NoteNames[rootPitchClass];
                    string chordName = rootName + pattern.Name;

                    // Check for inversion (bass note different from root)
                    int bassPitchClass = bassNote % 12;
                    if (bassPitchClass != rootPitchClass)
                    {
                        string bassName = NoteNames[bassPitchClass];
                        chordName += "/" + bassName;
                    }

                    return chordName;
                }
            }
        }

        return null;
    }
}
