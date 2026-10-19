using System.Windows;
using System.Windows.Media;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// The glass in front of the stage: scanlines, a vignette that darkens toward the rim, a soft
/// reflection, and the case's bezel with rounded corners framing the tube. Laid over the stage
/// and never hit-testable, so everything under it still takes clicks.
/// </summary>
/// <remarks>
/// Brushes come from the theme (Brush.Crt.*) like the other renderers'. The bezel is a ring:
/// the whole rectangle with the rounded screen cut out of it, so the corners of whatever is
/// drawn under it are hidden as a real tube's would be.
/// </remarks>
public sealed class CrtOverlay : FrameworkElement
{
    /// <summary>How far the bezel reaches in from each edge.</summary>
    public const double BezelThickness = 6;

    private const double ScreenRadius = 18;

    /// <summary>One scanline every this many DIPs.</summary>
    private const double ScanlinePitch = 3;

    private Brush? _scanlines;

    public CrtOverlay()
    {
        IsHitTestVisible = false;
        Focusable = false;
    }

    protected override Size MeasureOverride(Size availableSize) => new(0, 0);

    protected override void OnRender(DrawingContext dc)
    {
        double width = ActualWidth, height = ActualHeight;
        if (width <= BezelThickness * 2 || height <= BezelThickness * 2) return;

        var outer = new Rect(0, 0, width, height);
        var screen = new Rect(BezelThickness, BezelThickness,
                              width - (2 * BezelThickness), height - (2 * BezelThickness));
        var screenShape = new RectangleGeometry(screen, ScreenRadius, ScreenRadius);
        screenShape.Freeze();

        dc.PushClip(screenShape);
        dc.DrawRectangle(Scanlines(), null, screen);
        dc.DrawRectangle(Vignette, null, screen);
        dc.DrawRectangle(Reflection, null, screen);
        dc.Pop();

        var bezel = new CombinedGeometry(GeometryCombineMode.Exclude, new RectangleGeometry(outer), screenShape);
        bezel.Freeze();
        dc.DrawGeometry(Find("Brush.Crt.Bezel"), null, bezel);
        dc.DrawGeometry(null, new Pen(Find("Brush.Crt.Rim"), 1), screenShape);
    }

    private Brush Scanlines()
    {
        if (_scanlines is not null) return _scanlines;

        var line = new GeometryDrawing(Find("Brush.Crt.Scanline"), null, new RectangleGeometry(new Rect(0, 0, 1, 1)));
        var gap = new GeometryDrawing(Brushes.Transparent, null, new RectangleGeometry(new Rect(0, 0, 1, ScanlinePitch)));
        var tile = new DrawingGroup();
        tile.Children.Add(gap);
        tile.Children.Add(line);
        _scanlines = new DrawingBrush(tile)
        {
            TileMode = TileMode.Tile,
            Viewport = new Rect(0, 0, 1, ScanlinePitch),
            ViewportUnits = BrushMappingMode.Absolute,
            Viewbox = new Rect(0, 0, 1, ScanlinePitch),
            ViewboxUnits = BrushMappingMode.Absolute,
            Stretch = Stretch.None
        };
        _scanlines.Freeze();
        return _scanlines;
    }

    private static readonly Brush Vignette = Frozen(new RadialGradientBrush
    {
        GradientOrigin = new Point(0.5, 0.48),
        Center = new Point(0.5, 0.48),
        RadiusX = 0.72,
        RadiusY = 0.78,
        GradientStops =
        {
            new GradientStop(Color.FromArgb(0, 0, 0, 0), 0.55),
            new GradientStop(Color.FromArgb(150, 0, 0, 0), 1)
        }
    });

    private static readonly Brush Reflection = Frozen(new RadialGradientBrush
    {
        GradientOrigin = new Point(0.28, 0.12),
        Center = new Point(0.28, 0.12),
        RadiusX = 0.45,
        RadiusY = 0.4,
        GradientStops =
        {
            new GradientStop(Color.FromArgb(14, 255, 255, 255), 0),
            new GradientStop(Color.FromArgb(0, 255, 255, 255), 1)
        }
    });

    private static Brush Frozen(Brush brush)
    {
        brush.Freeze();
        return brush;
    }

    // Magenta for a missing key is loud on purpose, as in the other renderers.
    private Brush Find(string key) => TryFindResource(key) as Brush ?? Brushes.Magenta;
}
