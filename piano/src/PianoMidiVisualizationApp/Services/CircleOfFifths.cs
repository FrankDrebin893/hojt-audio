namespace PianoMidiVisualizationApp.Services;

/// <summary>The two rings of the circle: major keys outside, their relative minors inside.</summary>
public enum CircleRing { Major, Minor }

/// <summary>How a segment relates to the selected key.</summary>
public enum CircleSegmentRole
{
    None,

    /// <summary>A neighbour a fifth either side (IV and V), or the relative major/minor.</summary>
    Related,

    Selected
}

/// <summary>
/// The circle of fifths as plain arithmetic: twelve segments clockwise from C at the top, each
/// a perfect fifth (7 semitones) from the last. Both rings share segment indices, since a
/// major key and its relative minor share a key signature.
/// </summary>
/// <remarks>
/// 7 is its own inverse mod 12 (7 × 7 = 49 = 4 × 12 + 1), so the same multiplication maps a
/// segment to its pitch class and a pitch class back to its segment.
/// <para>
/// Everything this class returns for display is typeset with real ♯ and ♭ glyphs, as a
/// printed circle is. The minor ring uses the textbook lower case (a, f♯, b♭): "bb" in ASCII
/// would be unreadable, and "G#m" is too wide for an inner segment at the minimum window.
/// </para>
/// </remarks>
public static class CircleOfFifths
{
    public const int SegmentCount = 12;

    /// <summary>The F#/Gb segment at the bottom, the only one with two spellings.</summary>
    public const int EnharmonicSegment = 6;

    /// <summary>Order in which sharps, then flats, are added to a key signature.</summary>
    private static readonly string[] SharpOrder = { "F#", "C#", "G#", "D#", "A#", "E#", "B#" };
    private static readonly string[] FlatOrder = { "Bb", "Eb", "Ab", "Db", "Gb", "Cb", "Fb" };

    private static int Mod12(int value) => ((value % 12) + 12) % 12;

    /// <summary>Tonic pitch class of the major key at a segment: C, G, D, … F.</summary>
    public static int MajorTonicAt(int segment) => Mod12(segment) * 7 % 12;

    /// <summary>Tonic pitch class of the relative minor at a segment: A, E, B, … D.</summary>
    public static int MinorTonicAt(int segment) => (MajorTonicAt(segment) + 9) % 12;

    public static int TonicAt(CircleRing ring, int segment) =>
        ring == CircleRing.Major ? MajorTonicAt(segment) : MinorTonicAt(segment);

    /// <summary>The segment of the major key on this tonic.</summary>
    public static int SegmentOfMajor(int pitchClass) => Mod12(pitchClass) * 7 % 12;

    /// <summary>The segment of the minor key on this tonic — its relative major's segment.</summary>
    public static int SegmentOfMinor(int pitchClass) => SegmentOfMajor(pitchClass + 3);

    /// <summary>
    /// Sharps in the segment's signature, or flats as a negative count. The enharmonic bottom
    /// segment reports +6 (F# major); its 6-flat reading is Gb major.
    /// </summary>
    public static int SharpsAt(int segment)
    {
        int s = Mod12(segment);
        return s <= EnharmonicSegment ? s : s - SegmentCount;
    }

    /// <summary>
    /// A segment's tonic in plain ASCII ("Bb"), spelled for its side of the circle: sharps
    /// clockwise from C down to the bottom, flats on the way back up. The bottom segment takes
    /// whichever spelling is asked for.
    /// </summary>
    public static string TonicNameAt(CircleRing ring, int segment, bool flatsAtBottom = false)
    {
        int s = Mod12(segment);
        bool useFlats = s > EnharmonicSegment || (s == EnharmonicSegment && flatsAtBottom);
        return MusicNaming.PitchClassName(TonicAt(ring, s), useFlats);
    }

    /// <summary>
    /// The label drawn on a segment: "C", "B♭", "F♯/G♭" on the major ring; "a", "b♭",
    /// "d♯/e♭" on the minor ring.
    /// </summary>
    public static string LabelAt(CircleRing ring, int segment)
    {
        string label = Typeset(TonicNameAt(ring, segment));
        if (Mod12(segment) == EnharmonicSegment)
            label += "/" + Typeset(TonicNameAt(ring, segment, flatsAtBottom: true));

        return ring == CircleRing.Minor ? label.ToLowerInvariant() : label;
    }

    /// <summary>The compact count drawn on a segment: "0", "3♯", "2♭", or "6♯/6♭".</summary>
    public static string SignatureLabelAt(int segment)
    {
        int s = Mod12(segment);
        if (s == EnharmonicSegment) return "6♯/6♭";

        int sharps = SharpsAt(s);
        return sharps switch
        {
            0 => "0",
            > 0 => $"{sharps}♯",
            _ => $"{-sharps}♭"
        };
    }

    /// <summary>"no sharps or flats", "1 sharp", "3 flats".</summary>
    public static string DescribeSignature(int sharps) => sharps switch
    {
        0 => "no sharps or flats",
        1 => "1 sharp",
        -1 => "1 flat",
        > 0 => $"{sharps} sharps",
        _ => $"{-sharps} flats"
    };

    /// <summary>"3 sharps: F♯ C♯ G♯" — the signature spelled out, in the order it is written.</summary>
    public static string SpellSignature(int sharps)
    {
        if (sharps == 0) return DescribeSignature(0);

        var accidentals = sharps > 0 ? SharpOrder.Take(sharps) : FlatOrder.Take(-sharps);
        return $"{DescribeSignature(sharps)}: {string.Join(" ", accidentals.Select(Typeset))}";
    }

    /// <summary>
    /// Hover text for a segment: each key it names, with its signature. The bottom segment
    /// gets a line per spelling, since F# major and Gb major are different signatures.
    /// </summary>
    public static string TooltipAt(CircleRing ring, int segment)
    {
        int s = Mod12(segment);
        string quality = ring == CircleRing.Major ? "major" : "minor";

        if (s != EnharmonicSegment)
            return $"{Typeset(TonicNameAt(ring, s))} {quality}\n{SpellSignature(SharpsAt(s))}";

        return $"{Typeset(TonicNameAt(ring, s))} {quality}: {SpellSignature(6)}\n"
             + $"{Typeset(TonicNameAt(ring, s, flatsAtBottom: true))} {quality}: {SpellSignature(-6)}";
    }

    /// <summary>The segment whose signature a key uses — C for D Dorian and for A minor.</summary>
    public static int SignatureSegment(MusicKey key) => SegmentOfMajor(key.SignatureMajorTonic);

    /// <summary>
    /// The ring a key is shown on. Major and minor keys sit on their own rings. Any other scale
    /// sits on whichever ring holds its tonic at its signature segment — A harmonic minor on
    /// a, C major pentatonic on C — and a mode whose tonic is on neither (D Dorian) on the
    /// major ring, since that segment is what names its signature.
    /// </summary>
    public static CircleRing RingOf(MusicKey key)
    {
        if (key.Scale == ScaleType.Major) return CircleRing.Major;
        if (key.Scale == ScaleType.NaturalMinor) return CircleRing.Minor;

        return MinorTonicAt(SignatureSegment(key)) == key.Tonic ? CircleRing.Minor : CircleRing.Major;
    }

    /// <summary>
    /// How a segment relates to the selected key: the key's own segment is selected; its
    /// neighbours a fifth either way on the same ring (IV and V), and the relative key across
    /// the ring, are related. Everything is unrelated when no key is selected.
    /// </summary>
    public static CircleSegmentRole RoleOf(MusicKey? key, CircleRing ring, int segment)
    {
        if (key is not { } k) return CircleSegmentRole.None;

        int home = SignatureSegment(k);
        CircleRing homeRing = RingOf(k);
        int s = Mod12(segment);

        if (s == home)
            return ring == homeRing ? CircleSegmentRole.Selected : CircleSegmentRole.Related;

        bool isNeighbour = s == Mod12(home + 1) || s == Mod12(home - 1);
        return isNeighbour && ring == homeRing ? CircleSegmentRole.Related : CircleSegmentRole.None;
    }

    /// <summary>
    /// The line under the circle. Major and minor keys add their signature; any other scale
    /// names the major signature it borrows, e.g. "D Dorian · C major signature".
    /// </summary>
    public static string CaptionFor(MusicKey? key)
    {
        if (key is not { } k) return "Click a key to select it";

        // DisplayName leads with the tonic ("Eb Major"); only that token is a note name.
        var parts = k.DisplayName.Split(' ', 2);
        string name = $"{Typeset(parts[0])} {parts[1]}";

        if (k.Scale is ScaleType.Major or ScaleType.NaturalMinor)
            return $"{name} · {DescribeSignature(SignedCountFor(k))}";

        string signatureTonic = Typeset(MusicNaming.PitchClassName(k.SignatureMajorTonic, k.UsesFlats));
        return $"{name} · {signatureTonic} major signature";
    }

    /// <summary>The key's own signature count, reading the enharmonic segment as flats when the key does.</summary>
    private static int SignedCountFor(MusicKey key)
    {
        int sharps = SharpsAt(SignatureSegment(key));
        return sharps == 6 && key.UsesFlats ? -6 : sharps;
    }

    /// <summary>"F#" → "F♯", "Bb" → "B♭": a single ASCII note name with a real accidental glyph.</summary>
    public static string Typeset(string note) =>
        note.Length >= 2 && note[1] == 'b'
            ? $"{note[0]}♭{note[2..]}"
            : note.Replace('#', '♯');
}
