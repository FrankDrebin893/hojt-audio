using System.Globalization;
using System.Windows;
using System.Windows.Media;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// A grand staff: treble and bass staves joined by a brace, their clefs, the key signature,
/// and the held notes as one chord of whole notes — written C4 and up on the treble staff,
/// the rest on the bass. Spelling is decided upstream by <see cref="NoteSpeller"/>; this
/// control only lays the result out.
/// </summary>
/// <remarks>
/// Everything is measured in staff spaces (the gap between two staff lines) and drawn at
/// the largest space that fits, snapped to whole device pixels so the five lines of each
/// staff stay evenly spaced and sharp. A notation staff can only scale uniformly, so unlike
/// the circle of fifths nothing here has a size floor. The width is kept steady on purpose —
/// room for two accidental columns and a displaced second is always reserved — so playing
/// never nudges the staff; only a key change, or a chord that outgrows the reserve, does.
/// </remarks>
public sealed class GrandStaffControl : FrameworkElement
{
    // ----- Vertical layout, in staff spaces -----

    /// <summary>
    /// Treble bottom line to bass top line: C4 on its ledger line and B3 above the bass still
    /// clear each other by a space. Kept tight because every space here is height the staff
    /// cannot spend on size: at 4.0 the default window's staff drops from 13px spaces to 12.
    /// </summary>
    private const double StaffGap = 3.6;

    /// <summary>Room kept above the treble staff: up to E6 plainly, or C6 with a sharp or natural.</summary>
    private const double DefaultAbove = 4.0;

    /// <summary>Room kept below the bass staff: down to C2, the lowest key drawn, or Db2.</summary>
    private const double DefaultBelow = 2.9;

    /// <summary>Clearance left beyond anything that grows past the default room.</summary>
    private const double EdgeClearance = 0.3;

    // ----- Horizontal layout, in staff spaces -----

    private const double BraceWidth = 0.9;
    private const double SystemLineX = 1.3;
    private const double ClefX = 1.9;
    private const double ClefToSignature = 0.8;
    private const double SignatureToChord = 1.0;
    private const double SignatureOverlap = 0.12;   // key-signature glyphs sit this close together
    private const double NoteheadWidth = 1.6;
    private const double NoteheadHeight = 1.0;
    private const double LedgerOverhang = 0.4;
    private const double AccidentalToNote = 0.25;
    private const double AccidentalColumnGap = 0.15;

    /// <summary>Accidental room kept left of the notes even when none are shown: two sharps' worth.</summary>
    private const double ReservedAccidentalWidth = 3.2;

    private const double EndPadding = 1.0;

    // ----- Strokes, in staff spaces -----

    private const double StaffLineThickness = 0.1;
    private const double LedgerThickness = 0.16;
    private const double SystemLineThickness = 0.16;

    /// <summary>Largest staff space drawn, in DIPs. Any bigger and the staff outweighs the chord name.</summary>
    private const double MaxSpace = 14;

    /// <summary>Staff steps of the outer lines, counted from middle C (see <see cref="SpelledNote.StaffStep"/>).</summary>
    private const int TrebleTopStep = 10, TrebleBottomStep = 2, BassTopStep = -2, BassBottomStep = -10;

    // Key-signature positions as staff steps on the treble staff, in the order they are written.
    // The bass staff uses the same shapes two octaves (14 steps) down.
    private static readonly int[] TrebleSharpSteps = { 10, 7, 11, 8, 5, 9, 6 };   // F C G D A E B
    private static readonly int[] TrebleFlatSteps = { 6, 9, 5, 8, 4, 7, 3 };      // B E A D G C F

    // ----- Dependency properties -----

    /// <summary>The chord to show, as spelled notes. Any order; duplicates are harmless.</summary>
    public static readonly DependencyProperty NotesProperty = DependencyProperty.Register(
        nameof(Notes), typeof(IReadOnlyList<SpelledNote>), typeof(GrandStaffControl),
        new FrameworkPropertyMetadata(Array.Empty<SpelledNote>(),
            FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty SignatureProperty = DependencyProperty.Register(
        nameof(Signature), typeof(KeySignature), typeof(GrandStaffControl),
        new FrameworkPropertyMetadata(KeySignature.None,
            FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>Staff lines, ledger lines and the system line.</summary>
    public static readonly DependencyProperty LineBrushProperty = DependencyProperty.Register(
        nameof(LineBrush), typeof(Brush), typeof(GrandStaffControl),
        new FrameworkPropertyMetadata(Brushes.Gray, FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>Brace, clefs and key signature: the furniture that does not change as you play.</summary>
    public static readonly DependencyProperty SymbolBrushProperty = DependencyProperty.Register(
        nameof(SymbolBrush), typeof(Brush), typeof(GrandStaffControl),
        new FrameworkPropertyMetadata(Brushes.Silver, FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>Noteheads and their accidentals.</summary>
    public static readonly DependencyProperty NoteBrushProperty = DependencyProperty.Register(
        nameof(NoteBrush), typeof(Brush), typeof(GrandStaffControl),
        new FrameworkPropertyMetadata(Brushes.White, FrameworkPropertyMetadataOptions.AffectsRender));

    public IReadOnlyList<SpelledNote> Notes
    {
        get => (IReadOnlyList<SpelledNote>)GetValue(NotesProperty);
        set => SetValue(NotesProperty, value);
    }

    public KeySignature Signature
    {
        get => (KeySignature)GetValue(SignatureProperty);
        set => SetValue(SignatureProperty, value);
    }

    public Brush LineBrush
    {
        get => (Brush)GetValue(LineBrushProperty);
        set => SetValue(LineBrushProperty, value);
    }

    public Brush SymbolBrush
    {
        get => (Brush)GetValue(SymbolBrushProperty);
        set => SetValue(SymbolBrushProperty, value);
    }

    public Brush NoteBrush
    {
        get => (Brush)GetValue(NoteBrushProperty);
        set => SetValue(NoteBrushProperty, value);
    }

    private StaffLayout _layout = StaffLayout.Compute(Array.Empty<SpelledNote>(), KeySignature.None);

    public GrandStaffControl()
    {
        // Pixel snapping below is relative to this element, so its own origin has to be whole.
        UseLayoutRounding = true;
    }

    /// <summary>The staff a note is written on: C4 and up on the treble, by written position.</summary>
    public static bool IsOnTreble(SpelledNote note) => note.StaffStep >= 0;

    // ----- Layout -----

    protected override Size MeasureOverride(Size availableSize)
    {
        _layout = StaffLayout.Compute(Notes ?? Array.Empty<SpelledNote>(), Signature);
        double space = SpaceFor(availableSize);

        // Rounded up to whole device pixels: layout rounding would otherwise shave a fraction
        // off, and OnRender, re-deriving the space from that smaller box, would floor it a
        // whole pixel lower and draw the staff small inside its own bounds.
        double pixelsPerDip = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        return new Size(Math.Ceiling(_layout.Width * space * pixelsPerDip) / pixelsPerDip,
                        Math.Ceiling(_layout.Height * space * pixelsPerDip) / pixelsPerDip);
    }

    protected override Size ArrangeOverride(Size finalSize) => finalSize;

    protected override void OnDpiChanged(DpiScale oldDpi, DpiScale newDpi) => InvalidateMeasure();

    /// <summary>
    /// The largest staff space that fits, floored to whole device pixels: every line then
    /// lands on a pixel boundary and the five lines stay evenly spaced.
    /// </summary>
    private double SpaceFor(Size size)
    {
        double space = MaxSpace;
        if (!double.IsInfinity(size.Width)) space = Math.Min(space, size.Width / _layout.Width);
        if (!double.IsInfinity(size.Height)) space = Math.Min(space, size.Height / _layout.Height);

        double pixelsPerDip = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        // The epsilon keeps a size that came from this very calculation from flooring one pixel lower.
        return Math.Max(0, Math.Floor(space * pixelsPerDip + 1e-6) / pixelsPerDip);
    }

    // ----- Rendering -----

    protected override void OnRender(DrawingContext dc)
    {
        double space = SpaceFor(RenderSize);
        if (space < 1) return;

        double pixelsPerDip = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        var painter = new Painter(dc, space, pixelsPerDip,
            // Centred vertically in whatever height the layout handed over, on a whole pixel.
            originY: Math.Floor((RenderSize.Height - _layout.Height * space) / 2 * pixelsPerDip) / pixelsPerDip);

        var layout = _layout;
        var lineBrush = LineBrush;
        var symbolBrush = SymbolBrush;
        var noteBrush = NoteBrush;

        // Staves and the system line joining them at the left.
        foreach (double top in new[] { layout.TrebleTop, layout.BassTop })
        {
            for (int line = 0; line < 5; line++)
                painter.HorizontalLine(SystemLineX, layout.Width, top + line, StaffLineThickness, lineBrush);
        }
        painter.VerticalLine(SystemLineX, layout.TrebleTop, layout.BassTop + 4, SystemLineThickness, lineBrush);

        // Brace: the glyph stretched to the system's height at a fixed width. Scaled uniformly
        // to that height it would be three spaces wide.
        painter.Glyph(Glyphs.Brace, 0, layout.TrebleTop, BraceWidth, layout.BassTop + 4 - layout.TrebleTop, symbolBrush);

        // Clefs, each hung from the line it names: G on the treble's second line, F on the bass's fourth.
        painter.Glyph(Glyphs.GClef, ClefX, layout.TrebleTop + 3, symbolBrush);
        painter.Glyph(Glyphs.FClef, ClefX, layout.BassTop + 1, symbolBrush);

        // Key signature, on both staves.
        foreach (var mark in layout.SignatureMarks)
        {
            painter.Glyph(mark.Glyph, mark.X, layout.TrebleTop + (TrebleTopStep - mark.TrebleStep) / 2.0, symbolBrush);
            painter.Glyph(mark.Glyph, mark.X, layout.BassTop + (BassTopStep - (mark.TrebleStep - 14)) / 2.0, symbolBrush);
        }

        foreach (var ledger in layout.Ledgers)
            painter.HorizontalLine(ledger.Left, ledger.Right, ledger.Y, LedgerThickness, lineBrush);

        foreach (var note in layout.Noteheads)
            painter.Notehead(note.X, note.Y, noteBrush);

        foreach (var accidental in layout.Accidentals)
            painter.Glyph(accidental.Glyph, accidental.X, accidental.Y, noteBrush);
    }

    /// <summary>Draws in staff spaces, converting to DIPs and snapping lines to device pixels.</summary>
    private readonly struct Painter(DrawingContext dc, double space, double pixelsPerDip, double originY)
    {
        private double X(double spaces) => spaces * space;
        private double Y(double spaces) => originY + spaces * space;

        private double Snap(double dip) => Math.Floor(dip * pixelsPerDip + 0.5) / pixelsPerDip;

        /// <summary>A stroke thickness in whole device pixels, never thinner than one.</summary>
        private double Thickness(double spaces) => Math.Max(1, Math.Round(spaces * space * pixelsPerDip)) / pixelsPerDip;

        public void HorizontalLine(double left, double right, double y, double thickness, Brush brush)
        {
            double t = Thickness(thickness);
            dc.DrawRectangle(brush, null, new Rect(Snap(X(left)), Snap(Y(y) - t / 2), Snap(X(right)) - Snap(X(left)), t));
        }

        public void VerticalLine(double x, double top, double bottom, double thickness, Brush brush)
        {
            double t = Thickness(thickness);
            double y0 = Snap(Y(top) - Thickness(StaffLineThickness) / 2);
            double y1 = Snap(Y(bottom) + Thickness(StaffLineThickness) / 2);
            dc.DrawRectangle(brush, null, new Rect(Snap(X(x) - t / 2), y0, t, y1 - y0));
        }

        /// <summary>A glyph at its natural proportions, its anchor on <paramref name="anchorY"/>.</summary>
        public void Glyph(StaffGlyph glyph, double left, double anchorY, Brush brush) =>
            Glyph(glyph, left, anchorY - glyph.Anchor * glyph.Height, glyph.Width, glyph.Height, brush);

        /// <summary>A glyph stretched into a box.</summary>
        public void Glyph(StaffGlyph glyph, double left, double top, double width, double height, Brush brush)
        {
            var box = new Rect(X(left), Y(top), X(width), height * space);
            dc.PushTransform(new MatrixTransform(
                box.Width / glyph.Shape.Bounds.Width, 0, 0, box.Height / glyph.Shape.Bounds.Height,
                box.X - glyph.Shape.Bounds.X * box.Width / glyph.Shape.Bounds.Width,
                box.Y - glyph.Shape.Bounds.Y * box.Height / glyph.Shape.Bounds.Height));
            dc.DrawGeometry(brush, null, glyph.Shape);
            dc.Pop();
        }

        public void Notehead(double left, double centreY, Brush brush)
        {
            dc.PushTransform(new MatrixTransform(space, 0, 0, space, X(left + NoteheadWidth / 2), Y(centreY)));
            dc.DrawGeometry(brush, null, Glyphs.WholeNote);
            dc.Pop();
        }
    }

    // ----- Glyphs -----

    /// <summary>
    /// A font glyph as a filled outline. <see cref="Height"/> is its drawn height in staff
    /// spaces and <see cref="Anchor"/> the fraction of that height, from the top, that sits on
    /// the note's line or space.
    /// </summary>
    internal sealed class StaffGlyph
    {
        public Geometry Shape { get; }
        public double Height { get; }
        public double Anchor { get; }

        /// <summary>Horizontal squeeze applied to the font's proportions (1 = as designed).</summary>
        public double Narrowing { get; }

        public double Width => Height * Narrowing * Shape.Bounds.Width / Shape.Bounds.Height;

        public StaffGlyph(int codePoint, double height, double anchor, double narrowing = 1.0)
        {
            // Built large so curve flattening is fine at any size it is scaled down to.
            var text = new FormattedText(char.ConvertFromUtf32(codePoint), CultureInfo.InvariantCulture,
                FlowDirection.LeftToRight, Glyphs.Typeface, 100, Brushes.Black, 1.0);
            var shape = text.BuildGeometry(new Point(0, 0));
            shape.Freeze();
            Shape = shape;
            Height = height;
            Anchor = anchor;
            Narrowing = narrowing;
        }

        public double ExtentAbove => Anchor * Height;
        public double ExtentBelow => (1 - Anchor) * Height;
    }

    /// <summary>
    /// The Segoe UI Symbol glyphs, with heights and anchors measured from their outlines: the
    /// G clef's curl sits 0.625 of the way down, the F clef's dots are exactly one space apart,
    /// and a flat's note sits in its bowl rather than at its middle. The sharp is drawn at 80%
    /// of its designed width: Segoe's is half again as wide as an engraved one, and at full
    /// width a seven-sharp signature or a stack of sharps sprawls across the staff.
    /// </summary>
    internal static class Glyphs
    {
        public static readonly Typeface Typeface = new(new FontFamily("Segoe UI Symbol"),
            FontStyles.Normal, FontWeights.Normal, FontStretches.Normal);

        public static readonly StaffGlyph GClef = new(0x1D11E, 7.0, 0.625);
        public static readonly StaffGlyph FClef = new(0x1D122, 2.96, 0.44);
        public static readonly StaffGlyph Brace = new(0x1D114, 1, 0);
        public static readonly StaffGlyph Sharp = new(0x266F, 2.5, 0.5, narrowing: 0.8);
        public static readonly StaffGlyph Flat = new(0x266D, 2.3, 0.75);
        public static readonly StaffGlyph Natural = new(0x266E, 2.5, 0.5);
        public static readonly StaffGlyph DoubleSharp = new(0x1D12A, 1.0, 0.5);
        public static readonly StaffGlyph DoubleFlat = new(0x1D12B, 2.3, 0.75);

        /// <summary>
        /// A whole notehead one space tall, centred on the origin, drawn rather than taken from
        /// the font: Segoe UI Symbol's is too round to read as a whole note at staff size.
        /// </summary>
        public static readonly Geometry WholeNote = CreateWholeNote();

        public static StaffGlyph ForAccidental(int accidental) => accidental switch
        {
            <= -2 => DoubleFlat,
            -1 => Flat,
            1 => Sharp,
            >= 2 => DoubleSharp,
            _ => Natural
        };

        private static Geometry CreateWholeNote()
        {
            var outer = new EllipseGeometry(new Point(0, 0), NoteheadWidth / 2, NoteheadHeight / 2);
            var hole = new EllipseGeometry(new Point(0, 0), 0.38, 0.24)
            {
                Transform = new RotateTransform(55)
            };
            var note = new CombinedGeometry(GeometryCombineMode.Exclude, outer, hole).GetFlattenedPathGeometry(0.001, ToleranceType.Absolute);
            note.Freeze();
            return note;
        }
    }

    // ----- Layout computation -----

    internal readonly record struct SignatureMark(StaffGlyph Glyph, double X, int TrebleStep);
    internal readonly record struct Notehead(double X, double Y, SpelledNote Note);
    internal readonly record struct Accidental(StaffGlyph Glyph, double X, double Y, SpelledNote Note);
    internal readonly record struct Ledger(double Left, double Right, double Y);

    /// <summary>Where everything goes, in staff spaces from the top-left corner.</summary>
    internal sealed class StaffLayout
    {
        public double Width { get; private init; }
        public double Height { get; private init; }
        public double TrebleTop { get; private init; }
        public double BassTop { get; private init; }
        public IReadOnlyList<SignatureMark> SignatureMarks { get; private init; } = [];
        public IReadOnlyList<Notehead> Noteheads { get; private init; } = [];
        public IReadOnlyList<Accidental> Accidentals { get; private init; } = [];
        public IReadOnlyList<Ledger> Ledgers { get; private init; } = [];

        /// <summary>A note's height on its staff, in spaces below that staff's top line.</summary>
        private static double OffsetBelowTop(SpelledNote note) =>
            IsOnTreble(note) ? (TrebleTopStep - note.StaffStep) / 2.0 : (BassTopStep - note.StaffStep) / 2.0;

        public static StaffLayout Compute(IReadOnlyList<SpelledNote> notes, KeySignature signature)
        {
            var chord = notes.Distinct().OrderBy(n => n.StaffStep).ThenBy(n => n.Accidental).ToList();

            // --- Vertical: grow past the default room only for what actually sticks out ---
            double above = DefaultAbove, below = DefaultBelow;
            foreach (var note in chord)
            {
                var glyph = signature.AccidentalToPrint(note) is { } a ? Glyphs.ForAccidental(a) : null;

                // How far the note, or its accidental, reaches past its staff's outer line.
                if (IsOnTreble(note))
                {
                    double reach = Math.Max(NoteheadHeight / 2, glyph?.ExtentAbove ?? 0) - OffsetBelowTop(note);
                    above = Math.Max(above, reach + EdgeClearance);
                }
                else
                {
                    double reach = OffsetBelowTop(note) - 4 + Math.Max(NoteheadHeight / 2, glyph?.ExtentBelow ?? 0);
                    below = Math.Max(below, reach + EdgeClearance);
                }
            }

            double trebleTop = above;
            double bassTop = trebleTop + 4 + StaffGap;
            double height = bassTop + 4 + below;
            double StaffY(SpelledNote n) => (IsOnTreble(n) ? trebleTop : bassTop) + OffsetBelowTop(n);

            // --- Key signature ---
            double x = ClefX + Math.Max(Glyphs.GClef.Width, Glyphs.FClef.Width) + ClefToSignature;
            var marks = new List<SignatureMark>();
            var steps = signature.Fifths >= 0 ? TrebleSharpSteps : TrebleFlatSteps;
            var markGlyph = signature.Fifths >= 0 ? Glyphs.Sharp : Glyphs.Flat;
            for (int i = 0; i < Math.Min(7, Math.Abs(signature.Fifths)); i++)
            {
                marks.Add(new SignatureMark(markGlyph, x, steps[i]));
                x += markGlyph.Width - SignatureOverlap;
            }
            if (marks.Count > 0) x += SignatureOverlap;

            double chordStart = x + SignatureToChord;

            // --- Seconds: within a staff, the upper note of a second steps right, as on an
            // up-stem chord. Alternating from the bottom handles clusters of any length. ---
            var displaced = new HashSet<SpelledNote>();
            foreach (var staff in chord.GroupBy(IsOnTreble))
            {
                SpelledNote? previous = null;
                foreach (var note in staff)
                {
                    if (previous is { } p && note.StaffStep - p.StaffStep <= 1 && !displaced.Contains(p))
                        displaced.Add(note);
                    previous = note;
                }
            }

            // --- Accidentals: stacked into columns leftward from the notes, zig-zagging from the
            // outside in (top, bottom, next top, ...) as engravers do; each takes the first column
            // where it clears everything already in it. Both staves share the columns, so a
            // sharp hanging below the treble cannot collide with a flat rising from the bass. ---
            var pending = chord
                .Where(n => signature.AccidentalToPrint(n) is not null)
                .Select(n => (Note: n, Glyph: Glyphs.ForAccidental(signature.AccidentalToPrint(n)!.Value), Y: StaffY(n)))
                .OrderBy(a => a.Y)
                .ToList();

            var order = new List<(SpelledNote Note, StaffGlyph Glyph, double Y)>();
            for (int lo = 0, hi = pending.Count - 1; lo <= hi; lo++, hi--)
            {
                order.Add(pending[lo]);
                if (hi != lo) order.Add(pending[hi]);
            }

            var columns = new List<List<(SpelledNote Note, StaffGlyph Glyph, double Y)>>();
            foreach (var accidental in order)
            {
                var column = columns.FirstOrDefault(c => c.All(other => Clears(accidental, other)));
                if (column is null)
                {
                    column = new List<(SpelledNote, StaffGlyph, double)>();
                    columns.Add(column);
                }
                column.Add(accidental);
            }

            double columnsWidth = columns.Sum(c => c.Max(a => a.Glyph.Width))
                                  + Math.Max(0, columns.Count - 1) * AccidentalColumnGap;
            double noteX = chordStart + Math.Max(ReservedAccidentalWidth, columnsWidth + AccidentalToNote);

            var accidentals = new List<Accidental>();
            double right = noteX - AccidentalToNote;
            foreach (var column in columns)
            {
                double columnWidth = column.Max(a => a.Glyph.Width);
                foreach (var (note, glyph, y) in column)
                    accidentals.Add(new Accidental(glyph, right - glyph.Width, y, note));   // right-aligned
                right -= columnWidth + AccidentalColumnGap;
            }

            // --- Noteheads and ledger lines ---
            var heads = chord
                .Select(n => new Notehead(displaced.Contains(n) ? noteX + NoteheadWidth : noteX, StaffY(n), n))
                .ToList();

            var ledgers = new List<Ledger>();
            AddLedgers(ledgers, heads.Where(h => IsOnTreble(h.Note)).ToList(), trebleTop, TrebleTopStep, TrebleBottomStep);
            AddLedgers(ledgers, heads.Where(h => !IsOnTreble(h.Note)).ToList(), bassTop, BassTopStep, BassBottomStep);

            return new StaffLayout
            {
                // A displaced second's column is always reserved, so a second never widens the staff.
                Width = noteX + 2 * NoteheadWidth + EndPadding,
                Height = height,
                TrebleTop = trebleTop,
                BassTop = bassTop,
                SignatureMarks = marks,
                Noteheads = heads,
                Accidentals = accidentals,
                Ledgers = ledgers
            };
        }

        /// <summary>Whether two accidentals can share a column without their outlines touching.</summary>
        private static bool Clears((SpelledNote Note, StaffGlyph Glyph, double Y) a,
                                   (SpelledNote Note, StaffGlyph Glyph, double Y) b)
        {
            const double padding = 0.15;
            var (upper, lower) = a.Y <= b.Y ? (a, b) : (b, a);
            return upper.Y + upper.Glyph.ExtentBelow + padding <= lower.Y - lower.Glyph.ExtentAbove;
        }

        /// <summary>
        /// One ledger line per line position outside the staff that a note reaches, as wide as
        /// the noteheads that need it (both columns when a displaced second is out there too).
        /// </summary>
        private static void AddLedgers(List<Ledger> ledgers, List<Notehead> heads, double staffTop, int topStep, int bottomStep)
        {
            if (heads.Count == 0) return;

            int highest = heads.Max(h => h.Note.StaffStep);
            for (int step = topStep + 2; step <= highest; step += 2)
                AddLedger(ledgers, heads.Where(h => h.Note.StaffStep >= step), staffTop + (topStep - step) / 2.0);

            int lowest = heads.Min(h => h.Note.StaffStep);
            for (int step = bottomStep - 2; step >= lowest; step -= 2)
                AddLedger(ledgers, heads.Where(h => h.Note.StaffStep <= step), staffTop + (topStep - step) / 2.0);
        }

        private static void AddLedger(List<Ledger> ledgers, IEnumerable<Notehead> heads, double y)
        {
            var list = heads.ToList();
            if (list.Count == 0) return;
            ledgers.Add(new Ledger(
                list.Min(h => h.X) - LedgerOverhang,
                list.Max(h => h.X) + NoteheadWidth + LedgerOverhang,
                y));
        }
    }
}
