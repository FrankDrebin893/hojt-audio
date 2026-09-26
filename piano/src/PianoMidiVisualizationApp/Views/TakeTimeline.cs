using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using PianoMidiVisualizationApp.Services.Recording;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// A take drawn as a small piano roll: its notes, the A–B loop region and the playhead.
/// Dragging across it selects a loop region; a click while playing jumps the playhead there.
/// Drawn in OnRender rather than as elements, since the playhead moves 30 times a second.
/// </summary>
public class TakeTimeline : FrameworkElement
{
    /// <summary>Below this a mouse-down/up pair is a click, not a drag.</summary>
    private const double DragThreshold = 4;

    /// <summary>Pitch rows shown at the least, so a take of one note is not one fat bar.</summary>
    private const int MinimumPitchSpan = 12;

    private static readonly Brush Background = Frozen(new SolidColorBrush(Color.FromRgb(0x11, 0x11, 0x11)));
    private static readonly Brush NoteFill = Frozen(new SolidColorBrush(Color.FromRgb(80, 160, 235)));
    private static readonly Brush RegionFill = Frozen(new SolidColorBrush(Color.FromArgb(0x2E, 242, 201, 120)));
    private static readonly Pen RegionEdge = Frozen(new Pen(Frozen(new SolidColorBrush(Color.FromRgb(242, 201, 120))), 1));
    private static readonly Pen Playhead = Frozen(new Pen(Brushes.White, 1.5));
    private static readonly Pen SecondLine = Frozen(new Pen(Frozen(new SolidColorBrush(Color.FromRgb(0x2A, 0x2A, 0x2A))), 1));
    private static readonly Brush HintText = Frozen(new SolidColorBrush(Color.FromRgb(0x66, 0x66, 0x66)));
    private static readonly Typeface HintTypeface = new("Segoe UI");

    public static readonly DependencyProperty TakeProperty = DependencyProperty.Register(
        nameof(Take), typeof(Take), typeof(TakeTimeline),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty PositionProperty = DependencyProperty.Register(
        nameof(Position), typeof(TimeSpan), typeof(TakeTimeline),
        new FrameworkPropertyMetadata(TimeSpan.Zero, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty ShowPlayheadProperty = DependencyProperty.Register(
        nameof(ShowPlayhead), typeof(bool), typeof(TakeTimeline),
        new FrameworkPropertyMetadata(false, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty LoopStartProperty = DependencyProperty.Register(
        nameof(LoopStart), typeof(TimeSpan?), typeof(TakeTimeline),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty LoopEndProperty = DependencyProperty.Register(
        nameof(LoopEnd), typeof(TimeSpan?), typeof(TakeTimeline),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public Take? Take
    {
        get => (Take?)GetValue(TakeProperty);
        set => SetValue(TakeProperty, value);
    }

    public TimeSpan Position
    {
        get => (TimeSpan)GetValue(PositionProperty);
        set => SetValue(PositionProperty, value);
    }

    public bool ShowPlayhead
    {
        get => (bool)GetValue(ShowPlayheadProperty);
        set => SetValue(ShowPlayheadProperty, value);
    }

    public TimeSpan? LoopStart
    {
        get => (TimeSpan?)GetValue(LoopStartProperty);
        set => SetValue(LoopStartProperty, value);
    }

    public TimeSpan? LoopEnd
    {
        get => (TimeSpan?)GetValue(LoopEndProperty);
        set => SetValue(LoopEndProperty, value);
    }

    /// <summary>A drag finished, selecting [start, end) in take time.</summary>
    public event EventHandler<(TimeSpan Start, TimeSpan End)>? RegionSelected;

    /// <summary>A click (not a drag) at this point in take time.</summary>
    public event EventHandler<TimeSpan>? SeekRequested;

    private double? _dragFromX;
    private double _dragToX;

    public TakeTimeline()
    {
        Cursor = Cursors.IBeam;
        Focusable = false;
        ClipToBounds = true;
    }

    private static T Frozen<T>(T freezable) where T : Freezable
    {
        freezable.Freeze();
        return freezable;
    }

    protected override void OnRender(DrawingContext dc)
    {
        double width = ActualWidth, height = ActualHeight;
        var bounds = new Rect(0, 0, width, height);
        dc.DrawRoundedRectangle(Background, null, bounds, 3, 3);

        var take = Take;
        if (take == null || take.Length <= TimeSpan.Zero || width <= 0 || height <= 0)
        {
            DrawHint(dc, take == null ? "No take selected" : "Empty take", bounds);
            return;
        }

        double secondsWide = take.Length.TotalSeconds;
        double X(TimeSpan t) => Math.Clamp(t.TotalSeconds / secondsWide, 0, 1) * width;

        // One faint line per second, or per five on a long take, so the lines stay sparse.
        int step = secondsWide > 40 ? 5 : 1;
        for (int s = step; s < secondsWide; s += step)
        {
            double x = Math.Round(X(TimeSpan.FromSeconds(s))) + 0.5;
            dc.DrawLine(SecondLine, new Point(x, 0), new Point(x, height));
        }

        // Region under the notes, so the notes stay readable through it.
        if (_dragFromX is { } from)
        {
            DrawRegion(dc, Math.Min(from, _dragToX), Math.Max(from, _dragToX), height);
        }
        else if (LoopStart.HasValue || LoopEnd.HasValue)
        {
            DrawRegion(dc, X(LoopStart ?? TimeSpan.Zero), X(LoopEnd ?? take.Length), height);
        }

        int low = take.LowestNote, high = take.HighestNote;
        int span = high - low + 1;
        if (span < MinimumPitchSpan)
        {
            low -= (MinimumPitchSpan - span) / 2;
            span = MinimumPitchSpan;
        }

        const double verticalPadding = 4;
        double rowHeight = (height - 2 * verticalPadding) / span;
        double barHeight = Math.Max(1.5, rowHeight - 1);

        foreach (var note in take.Notes)
        {
            double x = X(note.Start);
            double w = Math.Max(2, X(note.End) - x);
            double y = verticalPadding + (low + span - 1 - note.Note) * rowHeight;
            dc.PushOpacity(0.45 + 0.55 * Math.Clamp(note.Velocity, 0, 127) / 127.0);
            dc.DrawRectangle(NoteFill, null, new Rect(x, y, w, barHeight));
            dc.Pop();
        }

        if (ShowPlayhead)
        {
            double x = X(Position);
            dc.DrawLine(Playhead, new Point(x, 0), new Point(x, height));
        }
    }

    private static void DrawRegion(DrawingContext dc, double left, double right, double height)
    {
        dc.DrawRectangle(RegionFill, null, new Rect(left, 0, Math.Max(0, right - left), height));
        dc.DrawLine(RegionEdge, new Point(Math.Round(left) + 0.5, 0), new Point(Math.Round(left) + 0.5, height));
        dc.DrawLine(RegionEdge, new Point(Math.Round(right) - 0.5, 0), new Point(Math.Round(right) - 0.5, height));
    }

    private void DrawHint(DrawingContext dc, string text, Rect bounds)
    {
        var formatted = new FormattedText(text, CultureInfo.CurrentUICulture, FlowDirection.LeftToRight,
                                          HintTypeface, 11, HintText, VisualTreeHelper.GetDpi(this).PixelsPerDip);
        dc.DrawText(formatted, new Point((bounds.Width - formatted.Width) / 2, (bounds.Height - formatted.Height) / 2));
    }

    private TimeSpan TimeAt(double x) =>
        Take is { } take && ActualWidth > 0
            ? take.Length * Math.Clamp(x / ActualWidth, 0, 1)
            : TimeSpan.Zero;

    protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonDown(e);
        if (Take == null) return;

        _dragFromX = _dragToX = e.GetPosition(this).X;
        CaptureMouse();
        e.Handled = true;
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        if (_dragFromX == null) return;

        _dragToX = Math.Clamp(e.GetPosition(this).X, 0, ActualWidth);
        InvalidateVisual();
    }

    protected override void OnMouseLeftButtonUp(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonUp(e);
        if (_dragFromX is not { } from) return;

        double to = Math.Clamp(e.GetPosition(this).X, 0, ActualWidth);
        _dragFromX = null;
        ReleaseMouseCapture();
        InvalidateVisual();
        e.Handled = true;

        if (Math.Abs(to - from) < DragThreshold)
            SeekRequested?.Invoke(this, TimeAt(to));
        else
            RegionSelected?.Invoke(this, (TimeAt(Math.Min(from, to)), TimeAt(Math.Max(from, to))));
    }

    protected override void OnLostMouseCapture(MouseEventArgs e)
    {
        base.OnLostMouseCapture(e);
        if (_dragFromX == null) return;

        // Capture taken away mid-drag (Alt+Tab, a dialog): abandon the selection.
        _dragFromX = null;
        InvalidateVisual();
    }
}
