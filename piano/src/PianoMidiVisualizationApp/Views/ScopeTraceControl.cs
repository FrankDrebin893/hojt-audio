using System.Windows;
using System.Windows.Media;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// The scope's trace: the held chord as one waveform, the sum of a sine per note at its
/// equal-tempered pitch, drawn across the full width like channel 1 of an oscilloscope. With
/// nothing held it is the idle baseline. It shows what the chord's intervals do to the wave
/// (a fifth repeats sooner than a tritone), not the sound itself: no timbre, no envelope.
/// </summary>
/// <remarks>
/// Decorative and behind the readout, so it never takes input and has no size of its own:
/// it fills whatever cell it is given. The time window is fixed, so a chord always draws
/// the same trace and a render of it is reproducible.
/// </remarks>
public sealed class ScopeTraceControl : FrameworkElement
{
    /// <summary>Seconds across the full width.</summary>
    private const double Window = 0.03;

    /// <summary>Points per DIP of width: enough that the highest key's cycles stay smooth.</summary>
    private const double SamplesPerDip = 0.75;

    public static readonly DependencyProperty NotesProperty = DependencyProperty.Register(
        nameof(Notes), typeof(IReadOnlyList<int>), typeof(ScopeTraceControl),
        new FrameworkPropertyMetadata(Array.Empty<int>(), FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty TraceBrushProperty = DependencyProperty.Register(
        nameof(TraceBrush), typeof(Brush), typeof(ScopeTraceControl),
        new FrameworkPropertyMetadata(Brushes.LimeGreen, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty IdleBrushProperty = DependencyProperty.Register(
        nameof(IdleBrush), typeof(Brush), typeof(ScopeTraceControl),
        new FrameworkPropertyMetadata(Brushes.DarkGreen, FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>MIDI note numbers; any order.</summary>
    public IReadOnlyList<int> Notes
    {
        get => (IReadOnlyList<int>)GetValue(NotesProperty);
        set => SetValue(NotesProperty, value);
    }

    public Brush TraceBrush
    {
        get => (Brush)GetValue(TraceBrushProperty);
        set => SetValue(TraceBrushProperty, value);
    }

    /// <summary>The flat line drawn while nothing is held.</summary>
    public Brush IdleBrush
    {
        get => (Brush)GetValue(IdleBrushProperty);
        set => SetValue(IdleBrushProperty, value);
    }

    public ScopeTraceControl()
    {
        IsHitTestVisible = false;
        Focusable = false;
    }

    protected override Size MeasureOverride(Size availableSize) => new(0, 0);

    protected override void OnRender(DrawingContext dc)
    {
        double width = ActualWidth, height = ActualHeight;
        if (width <= 0 || height <= 0) return;

        double mid = height / 2;
        var notes = Notes ?? Array.Empty<int>();
        if (notes.Count == 0)
        {
            dc.DrawLine(new Pen(IdleBrush, 1), new Point(0, mid), new Point(width, mid));
            return;
        }

        var frequencies = notes.Select(n => 440.0 * Math.Pow(2, (n - 69) / 12.0)).ToArray();

        // Each voice gets the same amplitude and a fixed phase offset, so the sum rarely lines
        // up into one tall spike. Scaled by the voice count so a big chord fills the same band.
        double amplitude = height * 0.42 / Math.Max(1.6, Math.Sqrt(frequencies.Length) * 1.25);
        int samples = Math.Max(2, (int)(width * SamplesPerDip));

        var geometry = new StreamGeometry();
        using (var ctx = geometry.Open())
        {
            for (int i = 0; i <= samples; i++)
            {
                double x = width * i / samples;
                double t = Window * i / samples;
                double sum = 0;
                for (int v = 0; v < frequencies.Length; v++)
                    sum += Math.Sin((2 * Math.PI * frequencies[v] * t) + (v * 0.7));

                double y = Math.Clamp(mid - (sum * amplitude), 1, height - 1);
                if (i == 0) ctx.BeginFigure(new Point(x, y), isFilled: false, isClosed: false);
                else ctx.LineTo(new Point(x, y), isStroked: true, isSmoothJoin: true);
            }
        }
        geometry.Freeze();

        dc.DrawGeometry(null, new Pen(TraceBrush, 1.6) { LineJoin = PenLineJoin.Round }, geometry);
    }
}
