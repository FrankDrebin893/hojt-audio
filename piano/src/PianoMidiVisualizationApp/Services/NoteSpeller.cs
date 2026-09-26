namespace PianoMidiVisualizationApp.Services;

/// <summary>
/// A MIDI note written the way it would be on a staff: a letter, an accidental and an octave.
/// </summary>
/// <param name="Letter">0-6 for C D E F G A B.</param>
/// <param name="Accidental">-2 (double flat) to +2 (double sharp).</param>
/// <param name="Octave">
/// The written octave, which is not always the sounding one: B#3 is MIDI 60 and Cb4 is MIDI 59,
/// because the octave number changes at C, not at the pitch.
/// </param>
public readonly record struct SpelledNote(int MidiNote, int Letter, int Accidental, int Octave)
{
    private static readonly string[] LetterNames = { "C", "D", "E", "F", "G", "A", "B" };

    /// <summary>
    /// Lines and spaces from middle C: C4 = 0, D4 = 1, B3 = -1. Each step is one staff line
    /// or space, so this is the vertical position on either staff.
    /// </summary>
    public int StaffStep => (Octave - 4) * 7 + Letter;

    /// <summary>ASCII name with octave, e.g. "E#4", "Cb4", "Fx5" (x = double sharp), "Bbb3".</summary>
    public string Name => $"{LetterNames[Letter]}{AccidentalText(Accidental)}{Octave}";

    public static string AccidentalText(int accidental) => accidental switch
    {
        -2 => "bb",
        -1 => "b",
        1 => "#",
        2 => "x",
        _ => ""
    };
}

/// <summary>
/// A key signature as a position on the circle of fifths: +3 is three sharps (A major),
/// -2 is two flats (Bb major), 0 is none.
/// </summary>
public readonly record struct KeySignature(int Fifths)
{
    /// <summary>Letters in the order sharps are added: F C G D A E B. Flats go the other way.</summary>
    private static readonly int[] SharpOrder = { 3, 0, 4, 1, 5, 2, 6 };

    public static KeySignature None => new(0);

    /// <summary>The letters carrying an accidental, in the order they are written.</summary>
    public IEnumerable<int> Letters => Fifths >= 0
        ? SharpOrder.Take(Fifths)
        : SharpOrder.Reverse().Take(-Fifths);

    /// <summary>+1, -1 or 0: what this signature does to every note on that letter.</summary>
    public int AccidentalOf(int letter)
    {
        int rank = Array.IndexOf(SharpOrder, letter);
        if (Fifths > 0) return rank < Fifths ? 1 : 0;
        if (Fifths < 0) return 6 - rank < -Fifths ? -1 : 0;
        return 0;
    }

    /// <summary>
    /// The letter this signature spells a pitch class with, or null when the pitch class is
    /// not one of its seven notes.
    /// </summary>
    public int? LetterOf(int pitchClass)
    {
        for (int letter = 0; letter < 7; letter++)
        {
            if (NoteSpeller.PitchClassOf(letter, AccidentalOf(letter)) == Wrap(pitchClass))
                return letter;
        }
        return null;
    }

    /// <summary>
    /// The accidental a note needs printed beside it against this signature: null when the
    /// signature already implies it, 0 for a natural sign.
    /// </summary>
    public int? AccidentalToPrint(SpelledNote note) =>
        note.Accidental == AccidentalOf(note.Letter) ? null : note.Accidental;

    /// <summary>
    /// The signature a key is written with: that of its <see cref="MusicKey.SignatureMajorTonic"/>,
    /// on the flat or sharp side as <see cref="MusicKey.UsesFlats"/> says. No key reads as C major.
    /// </summary>
    public static KeySignature For(MusicKey? key)
    {
        if (key is not { } k) return None;

        // A major key on pitch class p sits 7p fifths round the circle (7 * 7 = 49 = 1 mod 12,
        // so 7 is its own inverse): 0-11 on the sharp side, that minus 12 on the flat side.
        int sharpSide = Wrap(k.SignatureMajorTonic * 7);
        int fifths = k.UsesFlats && sharpSide != 0 ? sharpSide - 12 : sharpSide;

        // A theoretical key (G# major, eight sharps) is written as its enharmonic instead.
        if (fifths > 7) fifths -= 12;
        if (fifths < -7) fifths += 12;
        return new KeySignature(fifths);
    }

    private static int Wrap(int pitchClass) => ((pitchClass % 12) + 12) % 12;
}

/// <summary>
/// Letter-correct spelling of MIDI notes in a key, for staff notation. This is deliberately
/// separate from <see cref="MusicNaming"/>: the text labels elsewhere use a simplified
/// sharp-or-flat table (see the remark on <see cref="MusicKey"/>), but a staff cannot, because
/// the letter decides which line the note sits on.
/// </summary>
/// <remarks>
/// <para>Notes in the key follow it exactly, including E#, B#, Cb and Fb where the scale demands
/// them: the 7th of F# major is E#, the 4th of Gb major is Cb, the 7th of G# harmonic minor
/// is F double-sharp.</para>
/// <para>Chromatic notes take naturals where one exists, otherwise sharps in sharp keys and
/// flats in flat keys. Keys with no signature use C# Eb F# G# Bb, the chromatic notes nearest
/// to C round the circle of fifths. One exception: a semitone below the tonic is always the
/// leading tone, so D minor's C# is never Db.</para>
/// <para>When a chord is spelled as a whole, a chromatic note may switch to its other name if
/// that makes more of the chord's thirds read as thirds on the staff (A C# E rather than
/// A Db E in F major), or keeps two notes off the same letter (C Db, not C C#).</para>
/// </remarks>
public static class NoteSpeller
{
    /// <summary>Pitch class of each natural letter, C D E F G A B.</summary>
    private static readonly int[] NaturalPitchClass = { 0, 2, 4, 5, 7, 9, 11 };

    /// <summary>
    /// Letter offset from the tonic for a scale note of a pentatonic or blues scale, by its
    /// semitones above the tonic. Only the heptatonic scales can count letters by degree;
    /// these name each note by its interval instead, so the blues b5 is a fifth (E blues: Bb).
    /// </summary>
    private static readonly int[] IntervalLetterOffset = { 0, 1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6 };

    // Chord-pass weights. A chromatic note leaves its default name only for a real gain: one
    // repaired third outweighs the switch. An E#/B#/Cb/Fb costs a little more, so a tie between
    // two spellings never goes its way, but one third is still enough: AbmM7 in Eb is
    // Ab Cb Eb G, and the Neapolitan of Eb is Fb Ab Cb.
    private const double AlternativeCost = 1.0;
    private const double WhiteKeyAccidentalCost = 1.4;
    private const double SameLetterCost = 2.0;
    private const double ThirdBonus = 1.5;

    public static int PitchClassOf(int letter, int accidental) =>
        Wrap(NaturalPitchClass[letter] + accidental);

    /// <summary>Spells one note in a key, with the key's own signature. No key reads as C major.</summary>
    public static SpelledNote Spell(int midiNote, MusicKey? key) =>
        Spell(midiNote, key, KeySignature.For(key));

    /// <summary>
    /// Spells one note in a key written with a given signature — how F# major (+6) and Gb major
    /// (-6), which are the same <see cref="MusicKey"/>, are told apart. A signature that does
    /// not contain the key's tonic is ignored in favour of the key's own.
    /// </summary>
    public static SpelledNote Spell(int midiNote, MusicKey? key, KeySignature signature)
    {
        var context = ContextFor(key, signature);
        int pitchClass = Wrap(midiNote);
        var (letter, accidental) = FixedSpelling(pitchClass, context) ?? ChromaticDefault(pitchClass, context);
        return Build(midiNote, letter, accidental);
    }

    /// <summary>
    /// Spells a chord: every note, lowest first, duplicates removed. Chromatic notes are spelled
    /// with the rest of the chord in view (see the remarks on this class).
    /// </summary>
    public static IReadOnlyList<SpelledNote> SpellChord(IEnumerable<int> midiNotes, MusicKey? key) =>
        SpellChord(midiNotes, key, KeySignature.For(key));

    public static IReadOnlyList<SpelledNote> SpellChord(
        IEnumerable<int> midiNotes, MusicKey? key, KeySignature signature)
    {
        var notes = midiNotes.Distinct().OrderBy(n => n).ToList();
        if (notes.Count == 0) return Array.Empty<SpelledNote>();

        var context = ContextFor(key, signature);
        var pitchClasses = notes.Select(Wrap).Distinct().ToArray();
        var options = pitchClasses.Select(pc => Candidates(pc, context)).ToArray();
        var chosen = BestAssignment(pitchClasses, options);

        var spellingOf = new Dictionary<int, Candidate>();
        for (int i = 0; i < pitchClasses.Length; i++)
            spellingOf[pitchClasses[i]] = chosen[i];

        return notes
            .Select(n => Build(n, spellingOf[Wrap(n)].Letter, spellingOf[Wrap(n)].Accidental))
            .ToList();
    }

    // ------------------------------------------------------------------------------------

    private readonly record struct Context(KeySignature Signature, int Tonic, int TonicLetter, ScaleType Scale);

    private readonly record struct Candidate(int Letter, int Accidental, double Cost);

    private static Context ContextFor(MusicKey? key, KeySignature signature)
    {
        var k = key ?? new MusicKey(0, ScaleType.Major);

        // The tonic is always one of its own signature's seven notes; a mismatched signature
        // could not name it, so fall back to the key's own rather than guess a letter.
        if (signature.LetterOf(k.Tonic) is not { } tonicLetter)
        {
            signature = KeySignature.For(k);
            tonicLetter = signature.LetterOf(k.Tonic) ?? 0;
        }

        return new Context(signature, k.Tonic, tonicLetter, k.Scale);
    }

    /// <summary>
    /// The spelling a note has because it belongs to the key: by scale degree for scale notes,
    /// by the signature for the rest of its seven notes. Null for a chromatic note.
    /// </summary>
    private static (int Letter, int Accidental)? FixedSpelling(int pitchClass, Context context)
    {
        int interval = Wrap(pitchClass - context.Tonic);
        var steps = context.Scale.Steps();

        for (int degree = 0; degree < steps.Count; degree++)
        {
            if (steps[degree] != interval) continue;

            int offset = steps.Count == 7 ? degree : IntervalLetterOffset[interval];
            int letter = (context.TonicLetter + offset) % 7;
            int accidental = AccidentalBetween(letter, pitchClass);
            if (Math.Abs(accidental) <= 2)
                return (letter, accidental);
        }

        // Signature notes outside the scale: a pentatonic's missing 4th and 7th, or harmonic
        // minor's unraised 7th. The signature already says how to write them.
        if (context.Signature.LetterOf(pitchClass) is { } signatureLetter)
            return (signatureLetter, context.Signature.AccidentalOf(signatureLetter));

        return null;
    }

    private static (int Letter, int Accidental) ChromaticDefault(int pitchClass, Context context)
    {
        // The leading tone reads as a raised 7th whatever the signature: C# in D minor, B# in
        // C# minor. Only single accidentals, so G# minor's F-double-sharp stays a plain G.
        if (pitchClass == Wrap(context.Tonic - 1))
        {
            int letter = (context.TonicLetter + 6) % 7;
            int accidental = AccidentalBetween(letter, pitchClass);
            if (Math.Abs(accidental) <= 1)
                return (letter, accidental);
        }

        int natural = Array.IndexOf(NaturalPitchClass, pitchClass);
        if (natural >= 0)
            return (natural, 0);

        bool sharp = context.Signature.Fifths switch
        {
            > 0 => true,
            < 0 => false,
            _ => pitchClass is 1 or 6 or 8    // C# F# G#, but Eb and Bb
        };

        return sharp
            ? (Array.IndexOf(NaturalPitchClass, pitchClass - 1), 1)
            : (Array.IndexOf(NaturalPitchClass, pitchClass + 1), -1);
    }

    /// <summary>
    /// Every way the chord pass may write a pitch class, the default first. A note in the key
    /// has exactly one; a chromatic note may also take any other single-accidental name.
    /// </summary>
    private static Candidate[] Candidates(int pitchClass, Context context)
    {
        if (FixedSpelling(pitchClass, context) is { } fixedSpelling)
            return new[] { new Candidate(fixedSpelling.Letter, fixedSpelling.Accidental, 0) };

        var (defaultLetter, defaultAccidental) = ChromaticDefault(pitchClass, context);
        var list = new List<Candidate> { new(defaultLetter, defaultAccidental, 0) };
        bool isWhiteKey = Array.IndexOf(NaturalPitchClass, pitchClass) >= 0;

        for (int letter = 0; letter < 7; letter++)
        {
            int accidental = AccidentalBetween(letter, pitchClass);
            if (letter == defaultLetter || Math.Abs(accidental) > 1) continue;

            // E#, B#, Cb and Fb are the odd ones out: legitimate, but never chosen on a tie.
            double cost = isWhiteKey && accidental != 0 ? WhiteKeyAccidentalCost : AlternativeCost;
            list.Add(new Candidate(letter, accidental, cost));
        }
        return list.ToArray();
    }

    /// <summary>
    /// Tries every combination (at most five chromatic pitch classes with three names each) and
    /// keeps the cheapest. Ties keep the earlier one, which favours default names.
    /// </summary>
    private static Candidate[] BestAssignment(int[] pitchClasses, Candidate[][] options)
    {
        int n = pitchClasses.Length;
        var index = new int[n];
        var current = new Candidate[n];
        Candidate[] best = options.Select(o => o[0]).ToArray();
        double bestScore = double.MaxValue;

        while (true)
        {
            for (int i = 0; i < n; i++)
                current[i] = options[i][index[i]];

            double score = Score(pitchClasses, current);
            if (score < bestScore - 1e-9)
            {
                bestScore = score;
                best = (Candidate[])current.Clone();
            }

            // Odometer step; done once every digit has rolled over.
            int digit = 0;
            while (digit < n && ++index[digit] == options[digit].Length)
                index[digit++] = 0;
            if (digit == n) return best;
        }
    }

    private static double Score(int[] pitchClasses, Candidate[] spelling)
    {
        double score = 0;
        for (int i = 0; i < spelling.Length; i++)
        {
            score += spelling[i].Cost;

            for (int j = 0; j < spelling.Length; j++)
            {
                if (i == j) continue;

                int semitones = Wrap(pitchClasses[j] - pitchClasses[i]);
                int letters = ((spelling[j].Letter - spelling[i].Letter) % 7 + 7) % 7;

                if (semitones is 3 or 4 && letters == 2)
                    score -= ThirdBonus;
                if (i < j && letters == 0)
                    score += SameLetterCost;
            }
        }
        return score;
    }

    private static SpelledNote Build(int midiNote, int letter, int accidental)
    {
        // The written octave counts from the letter's natural pitch, so B#3 lands on MIDI 60.
        int octave = (midiNote - accidental - NaturalPitchClass[letter]) / 12 - 1;
        return new SpelledNote(midiNote, letter, accidental, octave);
    }

    /// <summary>The accidental that turns a letter into a pitch class, from -6 to +5.</summary>
    private static int AccidentalBetween(int letter, int pitchClass)
    {
        int difference = Wrap(pitchClass - NaturalPitchClass[letter]);
        return difference >= 6 ? difference - 12 : difference;
    }

    private static int Wrap(int value) => ((value % 12) + 12) % 12;
}
