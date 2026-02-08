using Melanchall.DryWetMidi.Common;
using Melanchall.DryWetMidi.MusicTheory;

namespace PianoMidiVisualizationApp.Services;

public class ChordDetector
{
    public string? Detect(IEnumerable<int> midiNoteNumbers)
    {
        var noteNumbers = midiNoteNumbers.ToList();

        if (noteNumbers.Count == 0)
            return null;

        // Sort to find bass note
        noteNumbers.Sort();
        int bassNoteNumber = noteNumbers[0];

        // Convert MIDI numbers to NoteNames
        var noteNames = noteNumbers
            .Select(n => NoteUtilities.GetNoteName((SevenBitNumber)n))
            .ToArray();

        // Single note - show note name
        if (noteNames.Length == 1)
        {
            return FormatNoteName(noteNames[0]);
        }

        // Get unique pitch classes
        var uniqueNoteNames = noteNames.Distinct().ToArray();

        // All same pitch class (octaves)
        if (uniqueNoteNames.Length == 1)
        {
            return FormatNoteName(uniqueNoteNames[0]);
        }

        // Create chord and get names using DryWetMIDI
        try
        {
            var chord = new Chord(uniqueNoteNames);
            var chordNames = chord.GetNames().ToList();

            if (chordNames.Count > 0)
            {
                // Get the first (usually most common) chord name
                string chordName = chordNames[0];

                // Check for inversion (bass note different from root)
                var bassNoteName = NoteUtilities.GetNoteName((SevenBitNumber)bassNoteNumber);
                var rootNoteName = chord.NotesNames.FirstOrDefault();

                if (bassNoteName != rootNoteName)
                {
                    // Only add slash bass if not already in the name
                    if (!chordName.Contains('/'))
                    {
                        chordName += "/" + FormatNoteName(bassNoteName);
                    }
                }

                return chordName;
            }
        }
        catch
        {
            // Chord detection failed, fall through to manual display
        }

        // No chord detected - show note names
        var sortedNotes = uniqueNoteNames
            .OrderBy(n => (int)n)
            .Select(FormatNoteName);
        return string.Join("-", sortedNotes);
    }

    private static string FormatNoteName(NoteName noteName)
    {
        return noteName.ToString().Replace("Sharp", "#");
    }
}
