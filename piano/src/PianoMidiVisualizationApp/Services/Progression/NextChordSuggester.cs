namespace PianoMidiVisualizationApp.Services.Progression;

public enum TriadQuality { Major, Minor, Diminished, Augmented }

/// <summary>
/// A chord's role in a key: its root as semitones above the tonic, its triad quality, and the
/// Roman numeral this table knows it by ("IV", "V/V", "bVII").
/// </summary>
public readonly record struct ChordFunction(int Offset, TriadQuality Quality, string Label)
{
    /// <summary>Root, third, fifth — in that order, which is also their order of importance.</summary>
    public int[] PitchClasses(int tonic)
    {
        int root = (tonic + Offset) % 12;
        var (third, fifth) = Quality switch
        {
            TriadQuality.Major => (4, 7),
            TriadQuality.Minor => (3, 7),
            TriadQuality.Diminished => (3, 6),
            _ => (4, 8),
        };
        return new[] { root, (root + third) % 12, (root + fifth) % 12 };
    }
}

/// <summary>A next-chord idea, already voiced against the chord it follows.</summary>
public sealed record SuggestedChord(ChordFunction Function, int RootPitchClass, IReadOnlyList<int> Notes);

/// <summary>What the suggester made of the chord it was asked to continue from.</summary>
public sealed record SuggestionResult(string? FromLabel, IReadOnlyList<SuggestedChord> Suggestions)
{
    public static readonly SuggestionResult None = new(null, Array.Empty<SuggestedChord>());
}

/// <summary>
/// Proposes likely next chords from common functional-harmony motion: I to IV/V/vi/ii, ii to V,
/// IV to V/I/ii, V to I/vi, vi to ii/IV, iii to vi, vii° to I, V/x to x, bVI to bVII to I, and
/// iv to I, plus minor-key equivalents and a few near neighbours so every list has 3-4 entries.
/// </summary>
/// <remarks>
/// Functional harmony is a major/minor idea, so a modal key is read through its frame: a scale
/// with a major third above the tonic (Major, Lydian, Mixolydian, major pentatonic) uses the
/// major table, the rest the minor one. The minor frame is the usual textbook blend of natural
/// and harmonic minor — a major V and a leading-tone vii° alongside the subtonic VII. In a
/// seven-note mode, a frame chord the scale doesn't contain gives way to the scale's own triad
/// on that degree (Mixolydian's v and bVII, Dorian's IV), so the ideas stay in the mode; the
/// minor frame's V and vii° are kept, since a raised leading tone is their whole point.
/// </remarks>
public static class NextChordSuggester
{
    private static readonly ChordDetector Detector = new();

    private sealed record Frame(
        int[] DegreeOffsets,
        IReadOnlyDictionary<string, ChordFunction> Functions,
        IReadOnlyDictionary<string, string[]> Diatonic,
        IReadOnlyDictionary<string, string[]> Borrowed,
        IReadOnlyDictionary<string, string[]> Secondary,
        string[] Fallback);

    private static ChordFunction F(int offset, TriadQuality q, string label) => new(offset, q, label);

    private const TriadQuality Maj = TriadQuality.Major;
    private const TriadQuality Min = TriadQuality.Minor;
    private const TriadQuality Dim = TriadQuality.Diminished;

    private static readonly Frame MajorFrame = new(
        DegreeOffsets: new[] { 0, 2, 4, 5, 7, 9, 11 },
        Functions: new[]
        {
            F(0, Maj, "I"), F(2, Min, "ii"), F(4, Min, "iii"), F(5, Maj, "IV"),
            F(7, Maj, "V"), F(9, Min, "vi"), F(11, Dim, "vii°"),
            // Borrowed from the parallel minor, plus the Neapolitan.
            F(0, Min, "i"), F(2, Dim, "ii°"), F(3, Maj, "bIII"), F(5, Min, "iv"),
            F(7, Min, "v"), F(8, Maj, "bVI"), F(10, Maj, "bVII"), F(1, Maj, "bII"),
            // Secondary dominants, as plain major triads.
            F(9, Maj, "V/ii"), F(11, Maj, "V/iii"), F(0, Maj, "V/IV"), F(2, Maj, "V/V"), F(4, Maj, "V/vi"),
        }.ToDictionary(f => f.Label),
        Diatonic: new Dictionary<string, string[]>
        {
            ["I"] = new[] { "IV", "V", "vi", "ii" },
            ["ii"] = new[] { "V", "vii°", "IV" },
            ["iii"] = new[] { "vi", "IV", "ii" },
            ["IV"] = new[] { "V", "I", "ii", "vii°" },
            ["V"] = new[] { "I", "vi", "IV" },
            ["vi"] = new[] { "ii", "IV", "V" },
            ["vii°"] = new[] { "I", "vi", "iii" },
        },
        Borrowed: new Dictionary<string, string[]>
        {
            ["i"] = new[] { "iv", "bVI", "V" },
            ["ii°"] = new[] { "V", "vii°", "I" },
            ["bIII"] = new[] { "IV", "bVI", "bVII" },
            ["iv"] = new[] { "I", "V", "bVII" },
            ["v"] = new[] { "I", "IV", "bVII" },
            ["bVI"] = new[] { "bVII", "I", "V" },
            ["bVII"] = new[] { "I", "IV", "bVI" },
            ["bII"] = new[] { "V", "I", "vii°" },
        },
        // Keyed by the target: resolve to it, take the deceptive turn, or continue the chain
        // of dominants round the circle of fifths.
        Secondary: new Dictionary<string, string[]>
        {
            ["ii"] = new[] { "ii", "V/V", "IV" },
            ["iii"] = new[] { "iii", "V/vi", "I" },
            ["IV"] = new[] { "IV", "iv", "ii" },
            ["V"] = new[] { "V", "IV", "iii" },
            ["vi"] = new[] { "vi", "IV", "V/ii" },
        },
        Fallback: new[] { "I", "IV", "V", "vi" });

    private static readonly Frame MinorFrame = new(
        DegreeOffsets: new[] { 0, 2, 3, 5, 7, 8, 10 },
        Functions: new[]
        {
            F(0, Min, "i"), F(2, Dim, "ii°"), F(3, Maj, "III"), F(5, Min, "iv"),
            F(7, Maj, "V"), F(8, Maj, "VI"), F(10, Maj, "VII"), F(11, Dim, "vii°"),
            // From the parallel major and the other minor forms: the Picardy I, Dorian IV and ii,
            // the natural-minor v, and the Neapolitan.
            F(0, Maj, "I"), F(5, Maj, "IV"), F(2, Min, "ii"), F(7, Min, "v"), F(1, Maj, "bII"),
            F(0, Maj, "V/iv"), F(2, Maj, "V/V"), F(3, Maj, "V/VI"), F(5, Maj, "V/VII"),
        }.ToDictionary(f => f.Label),
        Diatonic: new Dictionary<string, string[]>
        {
            ["i"] = new[] { "iv", "V", "VI", "VII" },
            ["ii°"] = new[] { "V", "vii°", "iv" },
            ["III"] = new[] { "VI", "iv", "VII" },
            ["iv"] = new[] { "V", "i", "VII", "ii°" },
            ["V"] = new[] { "i", "VI", "iv" },
            ["VI"] = new[] { "VII", "iv", "V" },
            ["VII"] = new[] { "i", "III", "VI" },
            ["vii°"] = new[] { "i", "VI", "III" },
        },
        Borrowed: new Dictionary<string, string[]>
        {
            ["I"] = new[] { "iv", "IV", "V" },
            ["IV"] = new[] { "i", "V", "VII" },
            ["v"] = new[] { "i", "VI", "iv" },
            ["ii"] = new[] { "V", "vii°", "iv" },
            ["bII"] = new[] { "V", "i", "vii°" },
        },
        Secondary: new Dictionary<string, string[]>
        {
            ["iv"] = new[] { "iv", "VI", "V/VII" },
            ["V"] = new[] { "V", "i", "III" },
            ["VI"] = new[] { "VI", "iv", "VII" },
            ["VII"] = new[] { "VII", "i", "V" },
        },
        Fallback: new[] { "i", "iv", "V", "VI" });

    /// <summary>
    /// Suggests what could follow <paramref name="lastChordNotes"/> in <paramref name="key"/>,
    /// each voiced for the smoothest move from those exact notes.
    /// </summary>
    public static SuggestionResult Suggest(IReadOnlyList<int> lastChordNotes, MusicKey key)
    {
        if (lastChordNotes.Count == 0)
            return SuggestionResult.None;

        var frame = FrameFor(key);
        int tonic = key.Tonic;
        string? from = Classify(lastChordNotes, key, frame, out var labels);

        int lastMask = 0;
        foreach (int n in lastChordNotes)
            lastMask |= 1 << MusicNaming.PitchClassOf(n);

        var suggestions = new List<SuggestedChord>(4);
        var taken = new HashSet<(int, TriadQuality)>();
        foreach (string label in labels)
        {
            if (label == from || !frame.Functions.TryGetValue(label, out var function))
                continue;

            function = FitToScale(function, key, frame);
            var pitchClasses = function.PitchClasses(tonic);

            // Not a move at all if the last chord already holds it (C after C7), or a repeat.
            int mask = pitchClasses.Aggregate(0, (m, pc) => m | (1 << pc));
            if ((mask & ~lastMask) == 0 || !taken.Add((function.Offset, function.Quality)))
                continue;

            suggestions.Add(new SuggestedChord(
                function, pitchClasses[0], VoiceLeader.Voice(lastChordNotes, pitchClasses)));
            if (suggestions.Count == 4) break;
        }

        return new SuggestionResult(from, suggestions);
    }

    /// <summary>
    /// In a seven-note mode, swaps a diatonic frame chord that uses notes outside the scale for
    /// the scale's own triad on the same degree, when that is major or minor. Borrowed chords and
    /// secondary dominants are chromatic on purpose and pass through, as do major and minor keys,
    /// whose diatonic chords the frames already are.
    /// </summary>
    private static ChordFunction FitToScale(ChordFunction function, MusicKey key, Frame frame)
    {
        if (!key.IsHeptatonic || !frame.Diatonic.ContainsKey(function.Label))
            return function;
        if (frame == MinorFrame && function.Label is "V" or "vii°")
            return function;
        if (function.PitchClasses(key.Tonic).All(key.Contains))
            return function;

        int degree = Array.IndexOf(frame.DegreeOffsets, function.Offset);
        if (degree < 0)
            return function;

        var steps = key.Steps;
        int root = steps[degree];
        int third = MusicNaming.PitchClassOf(steps[(degree + 2) % 7] - root);
        int fifth = MusicNaming.PitchClassOf(steps[(degree + 4) % 7] - root);
        if (fifth != 7)
            return function;   // diminished or augmented: keep the frame's own chord

        return third switch
        {
            4 => function with { Offset = root, Quality = TriadQuality.Major },
            3 => function with { Offset = root, Quality = TriadQuality.Minor },
            _ => function,
        };
    }

    /// <summary>
    /// Names the chord's function in the key (null if it has none this table knows) and hands
    /// back the functions that usually follow it.
    /// </summary>
    private static string? Classify(IReadOnlyList<int> notes, MusicKey key, Frame frame, out string[] next)
    {
        int bass = MusicNaming.PitchClassOf(notes.Min());
        int root = Detector.Detect(notes)?.RootPitchClass ?? bass;
        int mask = 0;
        foreach (int n in notes)
            mask |= 1 << MusicNaming.PitchClassOf(n);

        int offset = MusicNaming.PitchClassOf(root - key.Tonic);
        var quality = QualityOf(mask, root);

        // Secondary dominants are recognised exactly as the numeral beside the chord reads, so
        // "V/V" on the card always gets V/V's continuations. That analyser also settles the
        // ambiguous cases: in a minor key a plain major I or IV is mixture, not V/iv or V/VII.
        var numeral = RomanNumeralAnalyzer.Analyze(root, mask, bass, key);
        if (numeral.Kind == RomanNumeralKind.SecondaryDominant)
        {
            // V/x resolves a fifth down; its leading-tone chord vii°/x a semitone up.
            bool leadingTone = numeral.Text.StartsWith("vii", StringComparison.Ordinal);
            int targetOffset = MusicNaming.PitchClassOf(offset + (leadingTone ? 1 : 5));
            var target = frame.Diatonic.Keys
                .Select(l => frame.Functions[l])
                .FirstOrDefault(f => f.Offset == targetOffset
                                     && f.Quality is TriadQuality.Major or TriadQuality.Minor);
            if (target.Label is { } t && frame.Secondary.TryGetValue(t, out var resolutions))
            {
                next = resolutions;
                return (leadingTone ? "vii°/" : "V/") + t;
            }
        }

        // Sus and power chords have no third; read them as whatever the key puts on that degree.
        var match = Lookup(frame.Diatonic, frame, offset, quality)
                 ?? Lookup(frame.Borrowed, frame, offset, quality);
        if (match is { } found)
        {
            next = found.Next;
            return found.Label;
        }

        next = frame.Fallback;
        return null;
    }

    private static (string Label, string[] Next)? Lookup(
        IReadOnlyDictionary<string, string[]> table, Frame frame, int offset, TriadQuality? quality)
    {
        foreach (var (label, next) in table)
        {
            var f = frame.Functions[label];
            if (f.Offset == offset && (quality is null || f.Quality == quality))
                return (label, next);
        }
        return null;
    }

    private static Frame FrameFor(MusicKey key) => key.Contains(key.Tonic + 4) ? MajorFrame : MinorFrame;

    /// <summary>Null when the chord has no third to decide it (sus, power chord, single note).</summary>
    private static TriadQuality? QualityOf(int mask, int root)
    {
        bool Has(int interval) => (mask & (1 << ((root + interval) % 12))) != 0;

        if (Has(4)) return Has(8) && !Has(7) ? TriadQuality.Augmented : TriadQuality.Major;
        if (Has(3)) return Has(6) && !Has(7) ? TriadQuality.Diminished : TriadQuality.Minor;
        return null;
    }
}
