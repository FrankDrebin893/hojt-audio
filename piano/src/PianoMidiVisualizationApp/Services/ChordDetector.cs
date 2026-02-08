namespace PianoMidiVisualizationApp.Services;

public class ChordDetector
{
    private static readonly string[] NoteNames = { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" };

    private static readonly (string Name, int[] Intervals)[] ChordPatterns =
    {
        // 5-note chords
        ("9", new[] { 0, 4, 7, 10, 14 }),       // Dominant 9th
        ("maj9", new[] { 0, 4, 7, 11, 14 }),    // Major 9th
        ("m9", new[] { 0, 3, 7, 10, 14 }),      // Minor 9th
        ("add9", new[] { 0, 4, 7, 14 }),        // Add 9

        // 4-note chords
        ("maj7", new[] { 0, 4, 7, 11 }),
        ("m7", new[] { 0, 3, 7, 10 }),
        ("7", new[] { 0, 4, 7, 10 }),
        ("dim7", new[] { 0, 3, 6, 9 }),
        ("m7b5", new[] { 0, 3, 6, 10 }),
        ("mMaj7", new[] { 0, 3, 7, 11 }),       // Minor-major 7th
        ("7sus4", new[] { 0, 5, 7, 10 }),       // 7sus4
        ("6", new[] { 0, 4, 7, 9 }),            // Major 6th
        ("m6", new[] { 0, 3, 7, 9 }),           // Minor 6th

        // 3-note chords
        ("", new[] { 0, 4, 7 }),       // Major
        ("m", new[] { 0, 3, 7 }),      // Minor
        ("dim", new[] { 0, 3, 6 }),    // Diminished
        ("aug", new[] { 0, 4, 8 }),    // Augmented
        ("sus4", new[] { 0, 5, 7 }),   // Suspended 4th
        ("sus2", new[] { 0, 2, 7 }),   // Suspended 2nd

        // 2-note intervals
        ("5", new[] { 0, 7 }),         // Power chord (perfect 5th)
        ("", new[] { 0, 4 }),          // Major 3rd (show as just root)
        ("m", new[] { 0, 3 }),         // Minor 3rd
    };

    public string? Detect(IEnumerable<int> midiNoteNumbers)
    {
        var notes = midiNoteNumbers.ToList();

        // No notes
        if (notes.Count == 0)
            return null;

        // Sort notes to find bass note
        notes.Sort();
        int bassNote = notes[0];

        // Single note - just show the note name
        if (notes.Count == 1)
        {
            return NoteNames[bassNote % 12];
        }

        // Convert to unique pitch classes (0-11)
        var pitchClasses = notes
            .Select(n => n % 12)
            .Distinct()
            .OrderBy(p => p)
            .ToList();

        // If all notes are the same pitch class (octaves), show the note
        if (pitchClasses.Count == 1)
        {
            return NoteNames[pitchClasses[0]];
        }

        // Try each pitch class as potential root
        foreach (var pattern in ChordPatterns)
        {
            for (int rootPitchClass = 0; rootPitchClass < 12; rootPitchClass++)
            {
                // Calculate expected pitch classes for this root
                var expectedPitchClasses = pattern.Intervals
                    .Select(interval => (rootPitchClass + interval) % 12)
                    .Distinct()
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

        // No known chord pattern matched - show the notes
        return string.Join("-", pitchClasses.Select(p => NoteNames[p]));
    }
}
