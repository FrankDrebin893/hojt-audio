using System.Text.RegularExpressions;
using Melanchall.DryWetMidi.MusicTheory;

namespace PianoMidiVisualizationApp.Services;

/// <summary>A detected chord: what to display, and the pitch class its intervals are measured from.</summary>
/// <param name="IsChord">
/// False for a single note or an unrecognised note list, whose "root" is only the note
/// itself or the bass, so inversions and voicings mean nothing.
/// </param>
public readonly record struct ChordNaming(string Name, int RootPitchClass, bool IsChord);

/// <summary>
/// Names the chord formed by a set of sounding MIDI notes, using DryWetMIDI's chord tables.
/// </summary>
public partial class ChordDetector
{
    /// <summary>Cache keyed by (pitch-class set, bass pitch class) — the only inputs to the result.</summary>
    private readonly Dictionary<int, ChordNaming> _cache = new();

    public ChordNaming? Detect(IEnumerable<int> midiNoteNumbers)
    {
        var noteNumbers = midiNoteNumbers.ToList();
        if (noteNumbers.Count == 0)
            return null;

        noteNumbers.Sort();
        int bassPitchClass = MusicNaming.PitchClassOf(noteNumbers[0]);

        int mask = 0;
        foreach (var n in noteNumbers)
            mask |= 1 << MusicNaming.PitchClassOf(n);

        int cacheKey = (mask * 12) + bassPitchClass;
        if (_cache.TryGetValue(cacheKey, out var cached))
            return cached;

        var result = Compute(mask, bassPitchClass);
        _cache[cacheKey] = result;
        return result;
    }

    private static ChordNaming Compute(int pitchClassMask, int bassPitchClass)
    {
        var pitchClasses = Enumerable.Range(0, 12).Where(pc => (pitchClassMask & (1 << pc)) != 0).ToArray();

        // A single pitch class (one note, or the same note in several octaves) is just that note.
        if (pitchClasses.Length == 1)
            return new ChordNaming(MusicNaming.PitchClass(pitchClasses[0]), pitchClasses[0], IsChord: false);

        if (ShellOf(pitchClasses) is { } shell)
            return Named(shell.Name, shell.Root, bassPitchClass);

        try
        {
            var noteNames = pitchClasses.Select(pc => (NoteName)pc).ToArray();
            var candidates = new Chord(noteNames).GetNames().ToList();

            if (candidates.Count > 0)
            {
                // GetNames() is NOT ordered by usefulness — its first entry for a Cmaj7 voicing
                // is "C/B", for Csus4 it is "C5/F", and for a Dm7 voicing it is "F6". Score the
                // candidates instead of taking [0].
                string best = candidates
                    .OrderBy(name => ScoreName(name, bassPitchClass))
                    .First();

                return Named(best, RootOf(best) ?? bassPitchClass, bassPitchClass);
            }
        }
        catch
        {
            // Not a chord DryWetMIDI recognises — fall through to listing the notes.
        }

        // No chord name available (e.g. a bare interval or a cluster): list the pitch classes,
        // and measure intervals from the bass.
        var spelled = pitchClasses.Select(MusicNaming.PitchClass);
        return new ChordNaming(string.Join("-", spelled), bassPitchClass, IsChord: false);
    }

    private static ChordNaming Named(string name, int rootPitchClass, int bassPitchClass)
    {
        // Show the inversion. Skip it if the chosen name already carries a slash, since
        // DryWetMIDI's own "X/Y" form means something different from slash-bass notation.
        string display = name;
        if (rootPitchClass != bassPitchClass && !name.Contains('/'))
            display = $"{name}/{MusicNaming.PitchClass(bassPitchClass)}";

        return new ChordNaming(display, rootPitchClass, IsChord: true);
    }

    /// <summary>
    /// Names a three-note shell — root, 3rd and 7th, the 5th left out — as its seventh chord.
    /// DryWetMIDI can't: it finds nothing for G-B-F and calls C-E-B "E5/C". Only one note of
    /// such a set can be the root, so the reading is unambiguous.
    /// </summary>
    private static (string Name, int Root)? ShellOf(int[] pitchClasses)
    {
        if (pitchClasses.Length != 3)
            return null;

        foreach (int root in pitchClasses)
        {
            int intervals = 0;
            foreach (int pc in pitchClasses)
                intervals |= 1 << (((pc - root) % 12 + 12) % 12);

            // Suffixes match what ScoreName picks for the full chord, so a shell and its
            // complete voicing read the same.
            string? quality = intervals switch
            {
                (1 << 0) | (1 << 4) | (1 << 10) => "7",
                (1 << 0) | (1 << 4) | (1 << 11) => "maj7",
                (1 << 0) | (1 << 3) | (1 << 10) => "m7",
                (1 << 0) | (1 << 3) | (1 << 11) => "mM7",
                _ => null,
            };
            if (quality is not null)
                return (MusicNaming.PitchClass(root) + quality, root);
        }

        return null;
    }

    /// <summary>
    /// Lower is better. Prefers a name rooted on the bass, without DryWetMIDI's "added note"
    /// slash form, spelled the way a player would write it.
    /// </summary>
    private static double ScoreName(string name, int bassPitchClass)
    {
        double score = 0;

        if (name.Contains('/')) score += 100;                       // "C5/F" for a Csus4
        if (RootOf(name) != bassPitchClass) score += 50;            // "F6" for a Dm7 voicing
        if (name.Contains('ø')) score += 20;                   // "Cø" over "Cm7b5"
        if (name.Contains("dim5")) score += 10;                     // "Cm7dim5" over "Cm7b5"
        if (name.Contains("min") || name.Contains("maj")) score += 4;  // "Amin" over "Am"
        if (MajorMarkerRegex().IsMatch(name)) score += 6;           // "CM" / "CM6" over "C" / "C6"

        return score + (name.Length * 0.1);                         // tie-break toward the shorter name
    }

    /// <summary>
    /// Pitch class of the leading note letter, or null if the name doesn't start with one.
    /// </summary>
    /// <remarks>
    /// Deliberately not <c>Chord.TryParse(...).RootNoteName</c>: for a slash name that returns the
    /// note *after* the slash (TryParse("C/E") gives E), which is the opposite of what's wanted.
    /// </remarks>
    private static int? RootOf(string name)
    {
        var match = RootNoteRegex().Match(name);
        if (!match.Success) return null;

        int pc = match.Groups[1].Value switch
        {
            "C" => 0, "D" => 2, "E" => 4, "F" => 5, "G" => 7, "A" => 9, "B" => 11, _ => -1
        };
        if (pc < 0) return null;

        if (match.Groups[2].Value == "#") pc++;
        else if (match.Groups[2].Value == "b") pc--;

        return ((pc % 12) + 12) % 12;
    }

    [GeneratedRegex(@"^([A-G])([#b]?)")]
    private static partial Regex RootNoteRegex();

    /// <summary>Matches an "M"-as-major marker, as in "CM" or "CM7", but not "Cmaj" or "Cm".</summary>
    [GeneratedRegex(@"^[A-G][#b]?M(?![a-z])")]
    private static partial Regex MajorMarkerRegex();
}
