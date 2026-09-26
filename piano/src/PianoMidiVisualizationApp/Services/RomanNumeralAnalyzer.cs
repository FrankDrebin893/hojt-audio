using System.Numerics;

namespace PianoMidiVisualizationApp.Services;

/// <summary>How a chord relates to the selected key.</summary>
public enum RomanNumeralKind
{
    /// <summary>Every pitch class is in the key.</summary>
    Diatonic,

    /// <summary>
    /// V or V7 of a diatonic chord other than the tonic ("V/V", "V7/ii"), or its leading-tone
    /// chord ("vii°7/V").
    /// </summary>
    SecondaryDominant,

    /// <summary>Not in the key, but in its parallel major or minor ("iv", "bVI", "bVII" in major).</summary>
    Borrowed,

    /// <summary>A recognised chord that fits none of the above ("bII", "#iv", "II7").</summary>
    Chromatic,
}

/// <param name="Text">
/// The numeral with its quality and figured-bass inversion, e.g. "V65/ii", "bVI", "viiø7".
/// Empty when the notes aren't a chord this analyser recognises.
/// </param>
/// <param name="Degree">
/// The scale degree the chord root sits on, 1-7, counted by letter from the tonic and ignoring
/// accidentals: "bVI" is 6, "#iv°" is 4, and "V/V" is 2 because its root is the second degree.
/// Null when <paramref name="Text"/> is empty.
/// </param>
public record RomanNumeral(string Text, int? Degree, RomanNumeralKind Kind)
{
    public static readonly RomanNumeral None = new("", null, RomanNumeralKind.Chromatic);

    public bool IsEmpty => string.IsNullOrEmpty(Text);
}

/// <summary>
/// Names a chord as a Roman numeral relative to a key, from its pitch classes alone.
/// </summary>
/// <remarks>
/// <para>
/// Quality comes from the intervals above the root (which third, which fifth, which seventh),
/// never from parsing a DryWetMIDI chord name, which is unreliable to parse. The given root is
/// trusted first so the numeral matches the chord name shown beside it; only when it yields no
/// recognisable chord is every other held pitch class tried as the root. Diminished sevenths
/// are the exception: being symmetric, they are always read from their functional root.
/// </para>
/// <para>
/// Numerals follow the key's own scale degrees, so a diatonic root never carries an accidental:
/// A minor's chords read i, ii°, III, iv, v, VI, VII. A root outside the scale is measured
/// against the tonic's major scale, flat-side first: bII, bIII, bVI, bVII, and on the tritone
/// #iv for minor-third chords and bV for the rest. Pentatonic and blues keys take their letters
/// from the parent major or natural minor, since five or six notes can't name seven degrees.
/// </para>
/// </remarks>
public static class RomanNumeralAnalyzer
{
    /// <summary>Numeral for the chord in root position.</summary>
    public static RomanNumeral Analyze(int rootPc, int pitchClassMask, MusicKey key) =>
        Analyze(rootPc, pitchClassMask, rootPc, key);

    /// <summary>Numeral for the chord with <paramref name="bassPc"/> sounding lowest, figured-bass style: I6, V43.</summary>
    public static RomanNumeral Analyze(int rootPc, int pitchClassMask, int bassPc, MusicKey key)
    {
        int mask = pitchClassMask & AllPitchClasses;
        int root = Mod12(rootPc);
        int bass = Mod12(bassPc);
        if (!Has(mask, bass))
            bass = root;

        if (FindShape(root, bass, mask) is not { } match)
            return RomanNumeral.None;

        // A diminished seventh is symmetric: any of its four notes can be the root, and the
        // detector's choice is just whichever one sat in the bass. Pick the functional reading.
        if (match.Shape.Quality == Quality.Diminished7 && match.Tensions == 0)
            return ClassifyDiminishedSeventh(match.Root, mask, bass, key);

        return Classify(match, mask, bass, key);
    }

    // ----------------------------------------------------------------- chord shapes

    private const int AllPitchClasses = 0xFFF;

    private enum Quality
    {
        Major, Minor, Diminished, Augmented,
        Dominant7, Major7, Minor7, HalfDiminished7, Diminished7,
        MinorMajor7, AugmentedMajor7, Augmented7,
        Sus4, Sus2, Dominant7Sus4,
    }

    /// <param name="Mask">Intervals above the root, bit 0 = the root itself.</param>
    /// <param name="Upper">Uppercase numeral: a major third (or no third at all, for sus).</param>
    /// <param name="Symbol">What follows the numeral letters, before any figure: "°", "ø", "maj".</param>
    /// <param name="Third">Semitones to the third, or -1 for a sus chord.</param>
    /// <param name="Fifth">Semitones to the fifth, or -1 when it's omitted.</param>
    /// <param name="Seventh">Semitones to the seventh, or -1 for a triad.</param>
    /// <param name="Tensions">Extra intervals allowed on top when read as an extended chord.</param>
    private sealed record Shape(
        int Mask, Quality Quality, bool Upper, string Symbol,
        int Third, int Fifth, int Seventh, int Tensions = 0)
    {
        public bool HasSeventh => Seventh >= 0;
        public bool IsSus => Third < 0;
    }

    private readonly record struct Match(int Root, Shape Shape, int Tensions);

    private static int Bits(params int[] intervals) => intervals.Aggregate(0, (m, i) => m | (1 << i));

    // Tension bits: b9 = 1, 9 = 2, #9 = 3, 11 = 5, #11 = 6, 13 (or an added 6th) = 9.
    private static readonly Shape[] Shapes =
    {
        new(Bits(0, 4, 7),     Quality.Major,           true,  "",     4, 7, -1, Bits(2, 9)),
        new(Bits(0, 3, 7),     Quality.Minor,           false, "",     3, 7, -1, Bits(2, 9)),
        new(Bits(0, 3, 6),     Quality.Diminished,      false, "°",    3, 6, -1),
        new(Bits(0, 4, 8),     Quality.Augmented,       true,  "+",    4, 8, -1),

        new(Bits(0, 4, 7, 10), Quality.Dominant7,       true,  "",     4, 7, 10, Bits(1, 2, 3, 6, 9)),
        new(Bits(0, 4, 10),    Quality.Dominant7,       true,  "",     4, -1, 10, Bits(1, 2, 3, 9)),
        new(Bits(0, 4, 7, 11), Quality.Major7,          true,  "maj",  4, 7, 11, Bits(2, 6, 9)),
        new(Bits(0, 4, 11),    Quality.Major7,          true,  "maj",  4, -1, 11, Bits(2, 9)),
        new(Bits(0, 3, 7, 10), Quality.Minor7,          false, "",     3, 7, 10, Bits(2, 5, 9)),
        new(Bits(0, 3, 10),    Quality.Minor7,          false, "",     3, -1, 10, Bits(2, 5, 9)),
        new(Bits(0, 3, 6, 10), Quality.HalfDiminished7, false, "ø",    3, 6, 10),
        new(Bits(0, 3, 6, 9),  Quality.Diminished7,     false, "°",    3, 6, 9),
        new(Bits(0, 3, 7, 11), Quality.MinorMajor7,     false, "maj",  3, 7, 11, Bits(2)),
        new(Bits(0, 3, 11),    Quality.MinorMajor7,     false, "maj",  3, -1, 11, Bits(2)),
        new(Bits(0, 4, 8, 11), Quality.AugmentedMajor7, true,  "+maj", 4, 8, 11),
        new(Bits(0, 4, 8, 10), Quality.Augmented7,      true,  "+",    4, 8, 10),

        new(Bits(0, 5, 7),     Quality.Sus4,            true,  "",     -1, 7, -1),
        new(Bits(0, 2, 7),     Quality.Sus2,            true,  "",     -1, 7, -1),
        new(Bits(0, 5, 7, 10), Quality.Dominant7Sus4,   true,  "",     -1, 7, 10),
    };

    /// <summary>
    /// Finds the chord shape on the given root, then on the bass — each as an exact triad or
    /// seventh first, then with added notes on top (6, 9, 11, 13) — and only then as an exact
    /// shape on any other held pitch class. The given root comes first so the numeral agrees
    /// with the chord name and voicing line beside it: "C6" reads I(add6), not vi65 beside
    /// "Root position". Added notes never go root-hunting, or a stack of every white key would
    /// come back as "ii13".
    /// </summary>
    private static Match? FindShape(int root, int bass, int mask)
    {
        var anchors = bass == root ? new[] { root } : new[] { root, bass };
        foreach (int r in anchors)
        {
            if (ExactShape(mask, r) is { } exact)
                return exact;
            if (ExtendedShape(mask, r) is { } extended)
                return extended;
        }

        for (int step = 1; step < 12; step++)
        {
            int r = (root + step) % 12;
            if (Has(mask, r) && r != bass && ExactShape(mask, r) is { } exact)
                return exact;
        }

        return null;
    }

    private static Match? ExactShape(int mask, int root)
    {
        int rel = Rotate(mask, root);
        foreach (var shape in Shapes)
            if (rel == shape.Mask)
                return new Match(root, shape, 0);
        return null;
    }

    private static Match? ExtendedShape(int mask, int root)
    {
        int rel = Rotate(mask, root);
        foreach (var shape in Shapes)
        {
            int extra = rel & ~shape.Mask;
            if ((rel & shape.Mask) == shape.Mask && extra != 0 && (extra & ~shape.Tensions) == 0)
                return new Match(root, shape, extra);
        }
        return null;
    }

    // ----------------------------------------------------------------- classification

    private static RomanNumeral Classify(Match match, int mask, int bass, MusicKey key)
    {
        var (root, shape, tensions) = match;
        var (degree, numeral) = Spell(root, key, shape.Upper);
        string body = Body(shape, tensions, Mod12(bass - root));

        if (IsWithin(mask, key.PitchClassMask))
            return new RomanNumeral(numeral + body, degree, RomanNumeralKind.Diatonic);

        bool borrowed = IsBorrowed(mask, key);
        bool majorTriad = shape.Quality == Quality.Major && tensions == 0;
        bool dominant = shape.Quality == Quality.Dominant7;
        bool leadingTone = tensions == 0 && shape.Quality is Quality.Diminished
                                                          or Quality.HalfDiminished7
                                                          or Quality.Diminished7;

        // A plain major triad that the parallel key supplies is mixture first: in a minor key a
        // major I is a Picardy third and a major IV is Dorian, not V/iv and V/bVII.
        if (majorTriad && borrowed)
            return new RomanNumeral(numeral + body, degree, RomanNumeralKind.Borrowed);

        if ((majorTriad || dominant) && TargetNumeral(root + 5, key) is { } dominantTarget)
            return new RomanNumeral($"V{body}/{dominantTarget}", degree, RomanNumeralKind.SecondaryDominant);

        if (leadingTone && TargetNumeral(root + 1, key) is { } leadingToneTarget)
            return new RomanNumeral($"vii{body}/{leadingToneTarget}", degree, RomanNumeralKind.SecondaryDominant);

        return new RomanNumeral(numeral + body,
            degree, borrowed ? RomanNumeralKind.Borrowed : RomanNumeralKind.Chromatic);
    }

    /// <summary>
    /// Keeps the detector's root if it already reads as a leading-tone chord (vii°7 of the key,
    /// or of a secondary target), since that root was the bass. Otherwise re-roots on the key's
    /// leading tone, then on the secondary leading tone with the most common target.
    /// B°7 over D in C major is vii°65, not a borrowed "ii°7".
    /// </summary>
    private static RomanNumeral ClassifyDiminishedSeventh(int root, int mask, int bass, MusicKey key)
    {
        var shape = Shapes.First(s => s.Quality == Quality.Diminished7);
        RomanNumeral At(int r) => Classify(new Match(r, shape, 0), mask, bass, key);

        bool IsTonicLeadingTone(int r, RomanNumeral n) =>
            Mod12(r - key.Tonic) == 11 && n.Kind is RomanNumeralKind.Diatonic or RomanNumeralKind.Borrowed;

        var given = At(root);
        if (given.Kind == RomanNumeralKind.SecondaryDominant || IsTonicLeadingTone(root, given))
            return given;

        int[] rotations = { (root + 3) % 12, (root + 6) % 12, (root + 9) % 12 };

        foreach (int r in rotations)
            if (At(r) is var n && IsTonicLeadingTone(r, n))
                return n;

        // Each rotation leads to a different target; F#°7 (to V) beats D#°7 (to iii) in C major.
        var secondary = rotations
            .Select(r => (Root: r, Numeral: At(r)))
            .Where(c => c.Numeral.Kind == RomanNumeralKind.SecondaryDominant)
            .OrderBy(c => Array.IndexOf(TargetPreference, Mod12(c.Root + 1 - key.Tonic)))
            .Select(c => c.Numeral)
            .FirstOrDefault();

        return secondary ?? given;
    }

    /// <summary>Secondary targets by how often they're tonicised, as semitones above the tonic: V, ii, IV, vi, iii...</summary>
    private static readonly int[] TargetPreference = { 7, 2, 5, 9, 4, 10, 3, 8, 1, 6, 11 };

    /// <summary>
    /// The numeral of the diatonic chord a secondary dominant resolves to, e.g. "ii" for A7 in
    /// C major. Null unless the target is a major or minor triad built entirely from key tones and
    /// isn't the tonic — V/I is just V, and a diminished triad can't be tonicised.
    /// </summary>
    private static string? TargetNumeral(int targetPc, MusicKey key)
    {
        int target = Mod12(targetPc);
        int keyMask = key.PitchClassMask;

        if (target == key.Tonic || !Has(keyMask, target) || !Has(keyMask, target + 7))
            return null;

        bool major = Has(keyMask, target + 4);
        bool minor = Has(keyMask, target + 3);
        if (!major && !minor)
            return null;

        return Spell(target, key, upper: major).Numeral;
    }

    private static bool IsBorrowed(int mask, MusicKey key)
    {
        // The harmonic minor is in the list because the major-key vii°7 and V7b9 come from it.
        foreach (var scale in new[] { ScaleType.Major, ScaleType.NaturalMinor, ScaleType.HarmonicMinor })
            if (IsWithin(mask, new MusicKey(key.Tonic, scale).PitchClassMask))
                return true;
        return false;
    }

    // ----------------------------------------------------------------- spelling

    private static readonly string[] Numerals = { "I", "II", "III", "IV", "V", "VI", "VII" };

    /// <summary>Letter degree and accidental for a root outside the scale, by semitones above the tonic.</summary>
    private static readonly (int Degree, string Accidental)[] ChromaticDegrees =
    {
        (1, ""), (2, "b"), (2, ""), (3, "b"), (3, ""), (4, ""),
        (5, "b"),   // tritone; minor-third chords read as #iv instead (see Spell)
        (5, ""), (6, "b"), (6, ""), (7, "b"), (7, ""),
    };

    /// <summary>The numeral for a root, e.g. "bVI" or "ii", with its 1-7 letter degree.</summary>
    private static (int Degree, string Numeral) Spell(int rootPc, MusicKey key, bool upper)
    {
        int rel = Mod12(rootPc - key.Tonic);
        int scale = Rotate(SpellingScale(key).PitchClassMask, key.Tonic);

        int degree;
        string accidental;
        if (Has(scale, rel))
        {
            // Heptatonic, so one note per letter: the degree is how many scale notes lie at or below.
            degree = BitOperations.PopCount((uint)(scale & ((2 << rel) - 1)));
            accidental = "";
        }
        else if (rel == 6 && !upper)
        {
            (degree, accidental) = (4, "#");   // #iv°, the leading tone of V
        }
        else
        {
            (degree, accidental) = ChromaticDegrees[rel];
        }

        string letters = upper ? Numerals[degree - 1] : Numerals[degree - 1].ToLowerInvariant();
        return (degree, accidental + letters);
    }

    /// <summary>The seven-note scale whose letters name the degrees of this key.</summary>
    private static MusicKey SpellingScale(MusicKey key) => key.Scale switch
    {
        _ when key.IsHeptatonic => key,
        ScaleType.MajorPentatonic => new MusicKey(key.Tonic, ScaleType.Major),
        _ => new MusicKey(key.Tonic, ScaleType.NaturalMinor),   // minor pentatonic, blues
    };

    /// <summary>Everything after the numeral letters: quality symbol plus inversion or extensions.</summary>
    private static string Body(Shape shape, int tensions, int bassInterval)
    {
        if (shape.IsSus)
            return shape.Quality switch
            {
                Quality.Sus2 => "sus2",
                Quality.Dominant7Sus4 => "7sus4",
                _ => "sus4",
            };

        // Extended chords keep the root-position spelling: figured bass has no clean way to
        // show a 9th chord in inversion, and "V9" is what a player would write.
        if (tensions != 0)
            return shape.Symbol + Extensions(shape.Quality, tensions);

        if (shape.HasSeventh)
        {
            string figure = bassInterval == shape.Third ? "65"
                          : bassInterval == shape.Fifth ? "43"
                          : bassInterval == shape.Seventh ? "42"
                          : "7";
            return shape.Symbol + figure;
        }

        string triadFigure = bassInterval == shape.Third ? "6"
                           : bassInterval == shape.Fifth ? "64"
                           : "";
        return shape.Symbol + triadFigure;
    }

    private static string Extensions(Quality quality, int tensions)
    {
        bool flat9 = Has(tensions, 1), nine = Has(tensions, 2), sharp9 = Has(tensions, 3);
        bool eleven = Has(tensions, 5), sharp11 = Has(tensions, 6), thirteen = Has(tensions, 9);

        string natural = thirteen ? "13" : eleven ? "11" : nine ? "9" : "7";

        return quality switch
        {
            // Parenthesised, since a bare "I6" is figured bass for a first-inversion triad.
            Quality.Major or Quality.Minor => (nine, thirteen) switch
            {
                (true, true) => "(6/9)",
                (false, true) => "(add6)",
                _ => "(add9)",
            },
            Quality.Dominant7 => natural + (flat9 ? "b9" : "") + (sharp9 ? "#9" : "") + (sharp11 ? "#11" : ""),
            Quality.Major7 => natural + (sharp11 ? "#11" : ""),
            _ => natural,   // minor 7th and minor-major 7th: "ii9", "ii11", "imaj9"
        };
    }

    // ----------------------------------------------------------------- bit helpers

    private static int Mod12(int value) => ((value % 12) + 12) % 12;

    private static bool Has(int mask, int bit) => (mask & (1 << Mod12(bit))) != 0;

    private static bool IsWithin(int mask, int scaleMask) => (mask & ~scaleMask) == 0;

    /// <summary>Re-expresses a pitch-class mask relative to <paramref name="root"/>, so bit 0 is the root.</summary>
    private static int Rotate(int mask, int root)
    {
        int r = Mod12(root);
        return ((mask >> r) | (mask << (12 - r))) & AllPitchClasses;
    }
}
