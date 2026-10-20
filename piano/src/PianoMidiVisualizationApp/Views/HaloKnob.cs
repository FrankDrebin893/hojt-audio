using System.Windows;
using System.Windows.Automation.Peers;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// A panel knob with a level halo: a ring of LEDs round a black cap that lights from the
/// bottom left, clockwise, as the value rises. Drag up or down, or scroll, to turn it.
/// </summary>
/// <remarks>
/// Never focusable, like the other stage controls, so Space stays "save chord"; the same
/// levels can be set from the keyboard with the sliders in Settings. Brushes come from the
/// theme (Brush.Halo.*, Brush.Knob.*) like the other renderers'.
/// </remarks>
public sealed class HaloKnob : RangeBase
{
    private const int Segments = 15;

    /// <summary>The sweep, from the bottom left (-225°) round through the top to the bottom right.</summary>
    private const double StartAngle = -225, Sweep = 270;

    /// <summary>DIPs of vertical drag for the full range.</summary>
    private const double DragRange = 200;

    private Point _dragStart;
    private double _dragStartValue;

    static HaloKnob()
    {
        FocusableProperty.OverrideMetadata(typeof(HaloKnob), new FrameworkPropertyMetadata(false));
        CursorProperty.OverrideMetadata(typeof(HaloKnob), new FrameworkPropertyMetadata(Cursors.SizeNS));
        ValueProperty.OverrideMetadata(typeof(HaloKnob), new FrameworkPropertyMetadata(0.0, FrameworkPropertyMetadataOptions.AffectsRender));
        MaximumProperty.OverrideMetadata(typeof(HaloKnob), new FrameworkPropertyMetadata(1.0, FrameworkPropertyMetadataOptions.AffectsRender));
    }

    public HaloKnob()
    {
        Width = 104;
        Height = 104;
        IsTabStop = false;
    }

    private double Fraction => Maximum > Minimum ? (Value - Minimum) / (Maximum - Minimum) : 0;

    /// <summary>A slider to assistive tech, so the knob's name and level are announced and settable.</summary>
    protected override AutomationPeer OnCreateAutomationPeer() => new HaloKnobAutomationPeer(this);

    private sealed class HaloKnobAutomationPeer(HaloKnob owner) : RangeBaseAutomationPeer(owner)
    {
        protected override string GetClassNameCore() => nameof(HaloKnob);
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Slider;
    }

    protected override void OnRender(DrawingContext dc)
    {
        double size = Math.Min(ActualWidth, ActualHeight);
        if (size <= 0) return;
        var c = new Point(ActualWidth / 2, ActualHeight / 2);
        double outer = size / 2;

        // The 3D recessed well the knob sits in, with a lit bottom rim.
        dc.DrawEllipse(Find("Brush.Knob.Recess"), new Pen(Find("Brush.Knob.RecessRim"), 1.2), c, outer - 0.6, outer - 0.6);

        int lit = (int)Math.Round(Fraction * Segments);
        if (lit == 0 && Value > Minimum) lit = 1;
        double ledRadius = outer - (size * 0.088);
        double dot = size * 0.033;
        var offBrush = Find("Brush.Halo.Off");
        var offPen = new Pen(Find("Brush.Halo.OffRim"), 0.8);
        var coreBrush = Find("Brush.Halo.Core");

        for (int i = 0; i < Segments; i++)
        {
            double angle = (StartAngle + (Sweep * i / (Segments - 1))) * Math.PI / 180;
            var at = new Point(c.X + (ledRadius * Math.Cos(angle)), c.Y + (ledRadius * Math.Sin(angle)));
            if (i >= lit)
            {
                dc.DrawEllipse(offBrush, offPen, at, dot, dot);
            }
            else
            {
                Brush fill = i < 9 ? Find("Brush.Halo.Low")
                    : i < 12 ? Find("Brush.Halo.Mid")
                    : Find("Brush.Halo.High");
                Brush glow = i < 9 ? Find("Brush.Halo.LowGlow")
                    : i < 12 ? Find("Brush.Halo.MidGlow")
                    : Find("Brush.Halo.HighGlow");

                dc.DrawEllipse(glow, null, at, dot * 2.5, dot * 2.5);
                dc.DrawEllipse(fill, null, at, dot * 1.05, dot * 1.05);
                dc.DrawEllipse(coreBrush, null, new Point(at.X - (dot * 0.18), at.Y - (dot * 0.18)), dot * 0.42, dot * 0.42);
            }
        }

        // The 3D rotary cap: cast shadow, knurled outer body, machined bevel top, and lit pointer.
        double cap = outer * 0.66;
        dc.DrawEllipse(Find("Brush.Knob.CapShadow"), null, new Point(c.X, c.Y + (size * 0.035)), cap * 1.16, cap * 1.16);
        dc.DrawEllipse(Find("Brush.Knob.Cap"), new Pen(Find("Brush.Knob.CapRim"), 1.5), c, cap, cap);

        // Knurled grip flutes around the perimeter skirt of the rotary cap.
        const int Flutes = 28;
        var flutePen = new Pen(Find("Brush.Knob.Flute"), Math.Max(1.0, size * 0.014));
        for (int i = 0; i < Flutes; i++)
        {
            double a = 2 * Math.PI * i / Flutes;
            dc.DrawLine(flutePen,
                new Point(c.X + (cap * 0.80 * Math.Cos(a)), c.Y + (cap * 0.80 * Math.Sin(a))),
                new Point(c.X + (cap * 0.96 * Math.Cos(a)), c.Y + (cap * 0.96 * Math.Sin(a))));
        }

        // Machined top face with specular rim bevel.
        double topCap = cap * 0.78;
        dc.DrawEllipse(Find("Brush.Knob.CapTop"), new Pen(Find("Brush.Knob.CapBevel"), 1.1), c, topCap, topCap);

        double pointer = (StartAngle + (Sweep * Fraction)) * Math.PI / 180;
        var p0 = new Point(c.X + (cap * 0.36 * Math.Cos(pointer)), c.Y + (cap * 0.36 * Math.Sin(pointer)));
        var p1 = new Point(c.X + (cap * 0.74 * Math.Cos(pointer)), c.Y + (cap * 0.74 * Math.Sin(pointer)));

        var glowPen = new Pen(Find("Brush.Knob.PointerGlow"), Math.Max(3.6, size * 0.052))
        {
            StartLineCap = PenLineCap.Round,
            EndLineCap = PenLineCap.Round
        };
        var pen = new Pen(Find("Brush.Knob.Pointer"), Math.Max(2.0, size * 0.028))
        {
            StartLineCap = PenLineCap.Round,
            EndLineCap = PenLineCap.Round
        };
        dc.DrawLine(glowPen, p0, p1);
        dc.DrawLine(pen, p0, p1);
    }

    protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonDown(e);
        if (!IsEnabled) return;
        _dragStart = e.GetPosition(this);
        _dragStartValue = Value;
        CaptureMouse();
        e.Handled = true;
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        if (!IsMouseCaptured) return;
        double dy = _dragStart.Y - e.GetPosition(this).Y;
        Value = Math.Clamp(_dragStartValue + (dy / DragRange * (Maximum - Minimum)), Minimum, Maximum);
    }

    protected override void OnMouseLeftButtonUp(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonUp(e);
        if (IsMouseCaptured) ReleaseMouseCapture();
    }

    protected override void OnMouseWheel(MouseWheelEventArgs e)
    {
        base.OnMouseWheel(e);
        if (!IsEnabled) return;
        double step = (Maximum - Minimum) / 20;
        Value = Math.Clamp(Value + (Math.Sign(e.Delta) * step), Minimum, Maximum);
        e.Handled = true;
    }

    // Magenta for a missing key is loud on purpose, as in the other renderers.
    private Brush Find(string key) => TryFindResource(key) as Brush ?? Brushes.Magenta;
}
