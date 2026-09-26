using System.Globalization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// An interactive circle of fifths: the twelve major keys on the outer ring, their relative
/// minors inside. Highlights the selected key with its neighbours and relative, outlines the
/// current chord's root, and selects a key when a segment is clicked.
/// </summary>
/// <remarks>
/// Drawn in code at the largest size that fits rather than scaled by a Viewbox. A Viewbox
/// would shrink the labels along with the rings until they were unreadable at the 900×420
/// minimum window; here the geometry scales and the type sizes stop at a floor.
/// </remarks>
public partial class CircleOfFifthsControl : UserControl
{
    // Radii as fractions of the outer radius: an empty hub, the minor ring, the major ring. The
    // hub is kept small because the minor ring needs the depth: its bottom segment stacks two
    // spellings, and at the 900×420 minimum there is no other room for them.
    private const double HubFraction = 0.25;
    private const double RingSplitFraction = 0.62;

    /// <summary>Largest diameter drawn; any bigger and the circle outweighs the chord readout.</summary>
    private const double MaxDiameter = 300;

    /// <summary>Width of the gap between neighbouring segments, in pixels at any size.</summary>
    private const double SegmentGap = 1.5;

    /// <summary>Half a segment's angular width, in degrees.</summary>
    private const double HalfSegmentDegrees = 360.0 / CircleOfFifths.SegmentCount / 2;

    /// <summary>
    /// The selected key's tonic, or null for no key. Two-way by default: clicking a segment
    /// writes it, which is how a click reaches the settings and moves the key pickers.
    /// </summary>
    public static readonly DependencyProperty TonicPitchClassProperty = DependencyProperty.Register(
        nameof(TonicPitchClass), typeof(int?), typeof(CircleOfFifthsControl),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.BindsTwoWayByDefault,
            (d, _) => ((CircleOfFifthsControl)d).UpdateKey()));

    /// <summary>The selected key's scale. Two-way by default, like <see cref="TonicPitchClass"/>.</summary>
    public static readonly DependencyProperty ScaleProperty = DependencyProperty.Register(
        nameof(Scale), typeof(ScaleType), typeof(CircleOfFifthsControl),
        new FrameworkPropertyMetadata(ScaleType.Major, FrameworkPropertyMetadataOptions.BindsTwoWayByDefault,
            (d, _) => ((CircleOfFifthsControl)d).UpdateKey()));

    /// <summary>The current chord's root pitch class, or null when nothing is sounding.</summary>
    public static readonly DependencyProperty RootPitchClassProperty = DependencyProperty.Register(
        nameof(RootPitchClass), typeof(int?), typeof(CircleOfFifthsControl),
        new PropertyMetadata(null, (d, _) => ((CircleOfFifthsControl)d).UpdateRootMarker()));

    /// <summary>
    /// Where the circle sits across the width it is given. Aligning it to an edge keeps it
    /// still while a neighbouring column changes width, e.g. with the chord name's length.
    /// A caption wider than the circle grows away from that edge.
    /// </summary>
    public static readonly DependencyProperty CircleAlignmentProperty = DependencyProperty.Register(
        nameof(CircleAlignment), typeof(HorizontalAlignment), typeof(CircleOfFifthsControl),
        new PropertyMetadata(HorizontalAlignment.Center,
            (d, e) => ((CircleOfFifthsControl)d).ApplyAlignment((HorizontalAlignment)e.NewValue)));

    public int? TonicPitchClass
    {
        get => (int?)GetValue(TonicPitchClassProperty);
        set => SetValue(TonicPitchClassProperty, value);
    }

    public ScaleType Scale
    {
        get => (ScaleType)GetValue(ScaleProperty);
        set => SetValue(ScaleProperty, value);
    }

    public int? RootPitchClass
    {
        get => (int?)GetValue(RootPitchClassProperty);
        set => SetValue(RootPitchClassProperty, value);
    }

    public HorizontalAlignment CircleAlignment
    {
        get => (HorizontalAlignment)GetValue(CircleAlignmentProperty);
        set => SetValue(CircleAlignmentProperty, value);
    }

    /// <summary>One ring segment and the text drawn on it.</summary>
    private sealed class Segment(CircleRing ring, int index)
    {
        public CircleRing Ring { get; } = ring;
        public int Index { get; } = index;
        public Path Shape { get; } = new();

        /// <summary>
        /// The key's name, one block per line. Only the bottom minor segment has two: it is the
        /// narrowest slot with the longest name, so it stacks "d♯" over "e♭".
        /// </summary>
        public required TextBlock[] Names { get; init; }

        /// <summary>The key-signature count; major ring only, as the wedge's two keys share it.</summary>
        public TextBlock? Count { get; init; }

        public CircleSegmentRole Role { get; set; }
    }

    private readonly List<Segment> _segments = new();

    /// <summary>Outline laid over the root's segment. Not hit-testable, so clicks pass through.</summary>
    private readonly Path _rootMarker = new() { IsHitTestVisible = false, Visibility = Visibility.Collapsed };

    private double _diameter;

    public CircleOfFifthsControl()
    {
        InitializeComponent();
        ApplyAlignment(CircleAlignment);
        BuildSegments();
        UpdateKey();
    }

    private void ApplyAlignment(HorizontalAlignment alignment)
    {
        Stack.HorizontalAlignment = alignment;
        CircleCanvas.HorizontalAlignment = alignment;
        CaptionText.HorizontalAlignment = alignment;
    }

    private MusicKey? CurrentKey =>
        TonicPitchClass is { } pitchClass ? new MusicKey(pitchClass, Scale) : null;

    /// <summary>
    /// Creates every shape and label once. Resizing only moves them, and selection only
    /// restyles them, so neither rebuilds the tree or loses hover state.
    /// </summary>
    private void BuildSegments()
    {
        foreach (var ring in new[] { CircleRing.Major, CircleRing.Minor })
        {
            for (int index = 0; index < CircleOfFifths.SegmentCount; index++)
            {
                string label = CircleOfFifths.LabelAt(ring, index);
                var segment = new Segment(ring, index)
                {
                    Names = (ring == CircleRing.Minor ? label.Split('/') : new[] { label }).Select(MakeLabel).ToArray(),
                    Count = ring == CircleRing.Major ? MakeLabel(CircleOfFifths.SignatureLabelAt(index)) : null
                };

                var shape = segment.Shape;
                shape.Cursor = Cursors.Hand;
                shape.ToolTip = CircleOfFifths.TooltipAt(ring, index);
                AutomationProperties.SetName(shape, AutomationNameFor(ring, index));
                shape.MouseEnter += (_, _) => ApplyStyle(segment);
                shape.MouseLeave += (_, _) => ApplyStyle(segment);
                shape.MouseLeftButtonDown += (_, e) =>
                {
                    SelectKeyAt(segment);
                    e.Handled = true;
                };

                CircleCanvas.Children.Add(shape);
                foreach (var block in TextOf(segment))
                    CircleCanvas.Children.Add(block);

                _segments.Add(segment);
            }
        }

        _rootMarker.StrokeLineJoin = PenLineJoin.Round;
        _rootMarker.SetResourceReference(Shape.StrokeProperty, "Brush.Circle.RootMarker");
        Panel.SetZIndex(_rootMarker, 2);
        CircleCanvas.Children.Add(_rootMarker);
    }

    private static IEnumerable<TextBlock> TextOf(Segment segment) =>
        segment.Count is { } count ? segment.Names.Append(count) : segment.Names;

    private static TextBlock MakeLabel(string text)
    {
        var label = new TextBlock
        {
            Text = text,
            IsHitTestVisible = false,   // clicks and hover belong to the segment underneath
            TextAlignment = TextAlignment.Center
        };
        Panel.SetZIndex(label, 1);
        return label;
    }

    /// <summary>"A major", "F#/Gb major", "D#/Eb minor" — what the tooltip and a screen reader call it.</summary>
    private static string AutomationNameFor(CircleRing ring, int index)
    {
        string tonic = index == CircleOfFifths.EnharmonicSegment
            ? $"{CircleOfFifths.TonicNameAt(ring, index)}/{CircleOfFifths.TonicNameAt(ring, index, flatsAtBottom: true)}"
            : CircleOfFifths.TonicNameAt(ring, index);
        return $"{tonic} {(ring == CircleRing.Major ? "major" : "minor")}";
    }

    /// <summary>
    /// Writes the clicked key through the two-way properties, and so into the settings: an
    /// outer segment selects its major key, an inner one its natural minor.
    /// </summary>
    /// <remarks>
    /// SetCurrentValue rather than SetValue: a local value would replace a one-way binding
    /// outright and leave the circle deaf to the key pickers from then on.
    /// </remarks>
    private void SelectKeyAt(Segment segment)
    {
        SetCurrentValue(ScaleProperty, segment.Ring == CircleRing.Major ? ScaleType.Major : ScaleType.NaturalMinor);
        SetCurrentValue(TonicPitchClassProperty, CircleOfFifths.TonicAt(segment.Ring, segment.Index));
    }

    private void UpdateKey()
    {
        var key = CurrentKey;
        foreach (var segment in _segments)
        {
            segment.Role = CircleOfFifths.RoleOf(key, segment.Ring, segment.Index);
            ApplyStyle(segment);
        }

        CaptionText.Text = CircleOfFifths.CaptionFor(key);
        CaptionText.SetResourceReference(TextBlock.ForegroundProperty,
            key is null ? "Brush.Text.Faint" : "Brush.Text.Tertiary");
    }

    /// <summary>
    /// Resolves a segment's look from its role and hover state together, so that neither can
    /// overwrite the other. Colours are resource references into the theme.
    /// </summary>
    private static void ApplyStyle(Segment segment)
    {
        bool isMajor = segment.Ring == CircleRing.Major;
        bool isSelected = segment.Role == CircleSegmentRole.Selected;

        segment.Shape.SetResourceReference(Shape.FillProperty, segment.Role switch
        {
            CircleSegmentRole.Selected => "Brush.Circle.Selected",
            CircleSegmentRole.Related => "Brush.Circle.Related",
            _ => isMajor ? "Brush.Circle.Major" : "Brush.Circle.Minor"
        });

        if (segment.Shape.IsMouseOver)
            segment.Shape.SetResourceReference(Shape.StrokeProperty, "Brush.Circle.Hover");
        else
            segment.Shape.ClearValue(Shape.StrokeProperty);

        foreach (var name in segment.Names)
        {
            name.SetResourceReference(TextBlock.ForegroundProperty,
                isSelected ? "Brush.Text.Primary" : isMajor ? "Brush.Text.Secondary" : "Brush.Text.Tertiary");
            name.FontWeight = isSelected ? FontWeights.Bold : isMajor ? FontWeights.SemiBold : FontWeights.Normal;
        }

        segment.Count?.SetResourceReference(TextBlock.ForegroundProperty,
            isSelected ? "Brush.Text.Secondary" : "Brush.Text.Muted");
    }

    private void UpdateRootMarker()
    {
        if (RootPitchClass is { } pitchClass && _diameter > 0)
        {
            // The root sits on the major ring, which is the ring of pitch classes in fifths
            // order; the minor ring is a relabelling of the same wedges.
            int index = CircleOfFifths.SegmentOfMajor(pitchClass);
            _rootMarker.Data = _segments.First(s => s.Ring == CircleRing.Major && s.Index == index).Shape.Data;
            _rootMarker.Visibility = Visibility.Visible;
        }
        else
        {
            _rootMarker.Visibility = Visibility.Collapsed;
        }
    }

    private void Host_SizeChanged(object sender, SizeChangedEventArgs e) => Relayout();

    /// <summary>
    /// Fits the circle and its one-line caption into the host. The caption's height does not
    /// depend on its text, so the circle's size depends only on the space, never on the key.
    /// </summary>
    private void Relayout()
    {
        double width = Host.ActualWidth, height = Host.ActualHeight;
        if (width <= 0 || height <= 0) return;

        CaptionText.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        double captionHeight = CaptionText.DesiredSize.Height;   // includes its margin

        double diameter = Math.Floor(Math.Max(0, Math.Min(Math.Min(width, height - captionHeight), MaxDiameter)));

        // At least as wide as the circle so a short caption centres under it; a long one may
        // run wider, but never past the space the control was given.
        CaptionText.MinWidth = diameter;
        CaptionText.MaxWidth = width;

        if (Math.Abs(diameter - _diameter) >= 1)
            Layout(diameter);
    }

    /// <summary>Positions every shape and label for a circle of the given diameter.</summary>
    private void Layout(double diameter)
    {
        _diameter = diameter;
        CircleCanvas.Width = CircleCanvas.Height = diameter;

        double markerThickness = Math.Clamp(diameter * 0.012, 1.75, 3);
        _rootMarker.StrokeThickness = markerThickness;

        var centre = new Point(diameter / 2, diameter / 2);
        double outer = diameter / 2 - markerThickness / 2;   // leave the marker's stroke inside the canvas
        double split = outer * RingSplitFraction;
        double hub = outer * HubFraction;

        // Proportional type with a floor: below it the labels would stop being readable, above
        // the ceiling they would crowd the segment edges.
        double labelSize = Math.Clamp(outer * 0.16, 10, 17);
        double countSize = Math.Clamp(outer * 0.11, 8, 12);
        double minorSize = Math.Clamp(outer * 0.15, 9.5, 15);
        double gap = Math.Max(1, countSize * 0.2);

        foreach (var segment in _segments)
        {
            bool isMajor = segment.Ring == CircleRing.Major;
            double inner = isMajor ? split : hub;
            double rim = isMajor ? outer : split;
            double middle = (inner + rim) / 2;

            segment.Shape.Data = SectorGeometry(centre, inner, rim, segment.Index);

            foreach (var name in segment.Names)
                name.FontSize = isMajor ? labelSize : minorSize;

            if (segment.Count is { } count)
                count.FontSize = countSize;

            var blocks = StackOrder(segment).ToList();
            double radius = segment.Index == CircleOfFifths.EnharmonicSegment
                ? FitBottomStack(segment, blocks, middle, rim, gap)
                : middle;

            PlaceStack(centre, radius, segment.Index, outer, gap, blocks);
        }

        UpdateRootMarker();
    }

    /// <summary>
    /// The bottom segment carries the only two-spelling names, "F♯/G♭" and "d♯" over "e♭", each
    /// about as wide as the segment where it sits. The segment widens outward, so the stack is
    /// pushed out against the rim, and a name still too wide is set smaller, sized for its bold
    /// form. Returns the radius to centre the stack on.
    /// </summary>
    private double FitBottomStack(Segment segment, List<TextBlock> blocks, double middle, double rim, double gap)
    {
        double radius = middle;
        double halfAngle = HalfSegmentDegrees * Math.PI / 180;

        // Shrinking a name shortens the stack, which moves it again: the second pass settles it.
        for (int pass = 0; pass < 2; pass++)
        {
            var spans = blocks.Select(InkSpan).ToList();
            double total = spans.Sum(s => s.Height) + gap * (spans.Count - 1);
            radius = Math.Max(middle, rim - SegmentGap - 2 - total / 2);

            // At the bottom of the circle, down the screen is outward from the centre.
            double edge = radius - total / 2;
            for (int i = 0; i < blocks.Count; i++)
            {
                double blockRadius = edge + spans[i].Height / 2;
                edge += spans[i].Height + gap;

                if (segment.Names.Contains(blocks[i]))
                    blocks[i].FontSize = FitBold(blocks[i], 2 * blockRadius * Math.Sin(halfAngle) - SegmentGap);
            }
        }

        return radius;
    }

    /// <summary>
    /// The segment's text blocks top to bottom. The name always sits nearer the rim than the
    /// count — above it on the top half of the circle, below it on the bottom half — as it
    /// would on a printed circle, and there it gets the wider part of the segment.
    /// </summary>
    private static IEnumerable<TextBlock> StackOrder(Segment segment)
    {
        if (segment.Count is not { } count) return segment.Names;

        double degrees = segment.Index * HalfSegmentDegrees * 2;
        bool bottomHalf = Math.Cos(degrees * Math.PI / 180) < -1e-9;
        return bottomHalf ? segment.Names.Prepend(count) : segment.Names.Append(count);
    }

    /// <summary>
    /// Stacks text blocks on the segment's midpoint by the extent of their ink, not their line
    /// boxes: a line box carries empty leading, and centring by it would push a descender
    /// ("g") or a tall ♭ out of a narrow segment. Labels stay upright, each in a box as wide
    /// as the circle's radius so a weight change on selection cannot shift it sideways.
    /// </summary>
    private void PlaceStack(Point centre, double radius, int index, double boxWidth, double gap, List<TextBlock> blocks)
    {
        var mid = PointAt(centre, radius, index * HalfSegmentDegrees * 2);

        var spans = blocks.Select(block => (block, ink: InkSpan(block))).ToList();
        double total = spans.Sum(s => s.ink.Height) + gap * (spans.Count - 1);

        double inkTop = mid.Y - total / 2;
        foreach (var (block, ink) in spans)
        {
            block.Width = boxWidth;
            Canvas.SetLeft(block, mid.X - boxWidth / 2);
            Canvas.SetTop(block, inkTop - ink.Top);
            inkTop += ink.Height + gap;
        }
    }

    /// <summary>Where a block's ink starts and ends, measured down from the top of its line box.</summary>
    private InkExtent InkSpan(TextBlock block)
    {
        var text = Format(block.Text, new Typeface(block.FontFamily, block.FontStyle, block.FontWeight, block.FontStretch), block.FontSize);
        double bottom = text.Height + text.OverhangAfter;
        return new InkExtent(bottom - text.Extent, text.Extent);
    }

    private readonly record struct InkExtent(double Top, double Height);

    /// <summary>
    /// The largest size up to the block's current one at which its text, set bold as it is
    /// when its key is selected, fits the width. Selecting a key must never push its label out.
    /// </summary>
    private double FitBold(TextBlock block, double width)
    {
        var bold = new Typeface(block.FontFamily, block.FontStyle, FontWeights.Bold, block.FontStretch);
        double textWidth = Format(block.Text, bold, block.FontSize).Width;
        return textWidth <= width ? block.FontSize : block.FontSize * width / textWidth;
    }

    private FormattedText Format(string text, Typeface typeface, double size) =>
        new(text, CultureInfo.CurrentUICulture, FlowDirection.LeftToRight, typeface, size,
            Brushes.White, VisualTreeHelper.GetDpi(this).PixelsPerDip);

    /// <summary>
    /// An annular sector for one segment, inset on both sides so neighbours are separated by
    /// a gap of constant width rather than one that widens towards the rim.
    /// </summary>
    private static Geometry SectorGeometry(Point centre, double inner, double outer, int index)
    {
        double middle = index * HalfSegmentDegrees * 2;
        double start = middle - HalfSegmentDegrees;
        double end = middle + HalfSegmentDegrees;

        double outerInset = InsetDegrees(outer);
        double innerInset = InsetDegrees(inner);

        var figure = new PathFigure { StartPoint = PointAt(centre, outer, start + outerInset), IsClosed = true };
        figure.Segments.Add(new ArcSegment(PointAt(centre, outer, end - outerInset),
            new Size(outer, outer), 0, false, SweepDirection.Clockwise, true));
        figure.Segments.Add(new LineSegment(PointAt(centre, inner, end - innerInset), true));
        figure.Segments.Add(new ArcSegment(PointAt(centre, inner, start + innerInset),
            new Size(inner, inner), 0, false, SweepDirection.Counterclockwise, true));

        var geometry = new PathGeometry { Figures = { figure } };
        geometry.Freeze();
        return geometry;
    }

    /// <summary>The angle, at this radius, that half the segment gap spans.</summary>
    private static double InsetDegrees(double radius) =>
        radius <= SegmentGap ? 0 : Math.Asin(SegmentGap / 2 / radius) * 180 / Math.PI;

    /// <summary>A point at the given radius, clockwise from 12 o'clock by the given angle.</summary>
    private static Point PointAt(Point centre, double radius, double degrees)
    {
        double radians = degrees * Math.PI / 180;
        return new Point(centre.X + radius * Math.Sin(radians), centre.Y - radius * Math.Cos(radians));
    }
}
