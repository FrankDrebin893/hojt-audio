using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Shapes;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class PianoKeyboardControl : UserControl
{
    // Key sizes live in KeyboardLayout, which the falling-notes view shares so its notes land
    // exactly on these keys. Aliased here to keep the drawing code below readable.
    private const double WhiteKeyWidth = KeyboardLayout.WhiteKeyWidth;
    private const double WhiteKeyHeight = KeyboardLayout.WhiteKeyHeight;
    private const double BlackKeyWidth = KeyboardLayout.BlackKeyWidth;
    private const double BlackKeyHeight = KeyboardLayout.BlackKeyHeight;

    /// <summary>Drawing height, leaving a little headroom below the keys for their shadows.</summary>
    public const double CanvasHeight = WhiteKeyHeight + 6;

    // White key gradients - ivory to light gray for 3D effect
    private static readonly LinearGradientBrush WhiteKeyGradient = new(
        Color.FromRgb(255, 255, 253), // Warm white at top
        Color.FromRgb(235, 235, 230), // Slightly darker at bottom
        new Point(0, 0), new Point(0, 1));

    private static readonly LinearGradientBrush WhiteKeyPressedGradient = new(
        Color.FromRgb(140, 200, 255), // Lighter blue at top
        Color.FromRgb(80, 160, 235),  // Darker blue at bottom
        new Point(0, 0), new Point(0, 1));

    // Key-signature highlights. Green reads as "belongs here", amber marks the tonic so the
    // key centre is findable at a glance; both stay pale enough to leave the labels legible.
    private static readonly LinearGradientBrush WhiteKeyInKeyGradient = new(
        Color.FromRgb(232, 244, 228),
        Color.FromRgb(198, 226, 192),
        new Point(0, 0), new Point(0, 1));

    private static readonly LinearGradientBrush WhiteKeyTonicGradient = new(
        Color.FromRgb(255, 233, 184),
        Color.FromRgb(242, 201, 120),
        new Point(0, 0), new Point(0, 1));

    // Black key gradients - creates beveled top effect
    private static readonly LinearGradientBrush BlackKeyGradient;
    private static readonly LinearGradientBrush BlackKeyPressedGradient;
    private static readonly LinearGradientBrush BlackKeyInKeyGradient;
    private static readonly LinearGradientBrush BlackKeyTonicGradient;

    // Hints (the app suggesting a key) are an overlay, not a fill, so they can sit on top of
    // any fill. Magenta is the one hue clear of all of them: blue pressed, green in-key, amber
    // tonic, ivory and black.
    private static readonly SolidColorBrush HintBrush = new(Color.FromRgb(232, 62, 156));

    private static readonly SolidColorBrush KeyBorder = new(Color.FromRgb(60, 60, 60));
    private static readonly SolidColorBrush WhiteKeyLabel = new(Color.FromRgb(130, 130, 125));
    private static readonly SolidColorBrush BlackKeyLabel = new(Color.FromRgb(140, 140, 140));

    private readonly Dictionary<int, Rectangle> _keyRectangles = new();

    /// <summary>Where each key is drawn. Set when the keyboard is built; also read by the falling-notes view.</summary>
    public KeyboardLayout Layout { get; private set; } = KeyboardLayout.Default;

    /// <summary>Labels are tracked too, because the selected key re-spells their text.</summary>
    private readonly Dictionary<int, TextBlock> _keyLabels = new();

    /// <summary>Each key's hint overlay, collapsed until the key is hinted.</summary>
    private readonly Dictionary<int, Grid> _keyHints = new();

    static PianoKeyboardControl()
    {
        // Black key gradient with highlight at top for 3D bevel
        BlackKeyGradient = BeveledBlackKeyBrush(
            Color.FromRgb(70, 70, 70),   // Lighter top edge
            Color.FromRgb(35, 35, 35),   // Quick transition
            Color.FromRgb(25, 25, 25),   // Dark middle
            Color.FromRgb(15, 15, 15));  // Darker bottom

        BlackKeyPressedGradient = BeveledBlackKeyBrush(
            Color.FromRgb(80, 150, 220),
            Color.FromRgb(40, 120, 200),
            Color.FromRgb(25, 100, 180),
            Color.FromRgb(20, 80, 160));

        // The same bevel structure, tinted: a flat fill here would read as a dead rectangle
        // next to the unhighlighted black keys.
        BlackKeyInKeyGradient = BeveledBlackKeyBrush(
            Color.FromRgb(58, 90, 56),
            Color.FromRgb(40, 62, 39),
            Color.FromRgb(32, 50, 31),
            Color.FromRgb(28, 46, 27));

        BlackKeyTonicGradient = BeveledBlackKeyBrush(
            Color.FromRgb(106, 83, 38),
            Color.FromRgb(78, 60, 26),
            Color.FromRgb(66, 50, 22),
            Color.FromRgb(58, 44, 18));

        WhiteKeyGradient.Freeze();
        WhiteKeyPressedGradient.Freeze();
        WhiteKeyInKeyGradient.Freeze();
        WhiteKeyTonicGradient.Freeze();
        HintBrush.Freeze();
    }

    /// <summary>
    /// The four-stop vertical bevel every black key shares: a lit top edge, a fast falloff,
    /// then a slow darkening to the bottom.
    /// </summary>
    private static LinearGradientBrush BeveledBlackKeyBrush(Color top, Color shoulder, Color middle, Color bottom)
    {
        var brush = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = new Point(0, 1),
            GradientStops = new GradientStopCollection
            {
                new(top, 0.0),
                new(shoulder, 0.08),
                new(middle, 0.5),
                new(bottom, 1.0)
            }
        };
        brush.Freeze();
        return brush;
    }

    public PianoKeyboardControl()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (e.OldValue is PianoKeyboardViewModel oldVm)
        {
            foreach (var key in oldVm.Keys)
                key.PropertyChanged -= OnKeyPropertyChanged;
        }

        if (e.NewValue is PianoKeyboardViewModel vm)
        {
            BuildKeyboard(vm);
        }
    }

    private void BuildKeyboard(PianoKeyboardViewModel vm)
    {
        PianoCanvas.Children.Clear();
        _keyRectangles.Clear();
        _keyLabels.Clear();
        _keyHints.Clear();

        if (vm.Keys.Count == 0) return;
        Layout = new KeyboardLayout(vm.Keys[0].NoteNumber, vm.Keys[^1].NoteNumber);

        // First pass: draw white keys and their labels
        foreach (var key in vm.Keys)
        {
            if (!key.IsBlack)
            {
                double x = Layout.KeyLeft(key.NoteNumber);
                var rect = CreateWhiteKey(x, key);
                PianoCanvas.Children.Add(rect);
                _keyRectangles[key.NoteNumber] = rect;

                var label = CreateWhiteKeyLabel(x, key);
                PianoCanvas.Children.Add(label);
                _keyLabels[key.NoteNumber] = label;

                var hint = CreateKeyHint(x, key);
                PianoCanvas.Children.Add(hint);
                _keyHints[key.NoteNumber] = hint;
            }
        }

        // Second pass: draw black keys and their labels on top
        foreach (var key in vm.Keys)
        {
            if (key.IsBlack)
            {
                double x = Layout.KeyLeft(key.NoteNumber);
                var rect = CreateBlackKey(x, key);
                PianoCanvas.Children.Add(rect);
                _keyRectangles[key.NoteNumber] = rect;

                var label = CreateBlackKeyLabel(x, key);
                PianoCanvas.Children.Add(label);
                _keyLabels[key.NoteNumber] = label;

                var hint = CreateKeyHint(x, key);
                PianoCanvas.Children.Add(hint);
                _keyHints[key.NoteNumber] = hint;
            }
        }

        // Paint in the current scale roles and hints. Done after registration rather than inside
        // the Create* helpers so a key or hint set before this control existed still shows.
        foreach (var key in vm.Keys)
        {
            ApplyKeyFill(key);
            ApplyKeyHint(key);
        }

        // Size the canvas to the keys actually drawn. The Viewbox divides by this, so it is what
        // makes a narrower range render larger rather than leaving a gap.
        PianoCanvas.Width = Layout.TotalWidth;
        PianoCanvas.Height = CanvasHeight;

        // Subscribe to property changes
        foreach (var key in vm.Keys)
            key.PropertyChanged += OnKeyPropertyChanged;
    }

    private Rectangle CreateWhiteKey(double x, PianoKey key)
    {
        // Fill is deliberately left unset here; ApplyKeyFill assigns it once the key is
        // registered, so there is exactly one place that decides a key's colour.
        var rect = new Rectangle
        {
            Width = KeyboardLayout.WhiteKeyDrawnWidth,
            Height = WhiteKeyHeight,
            Stroke = KeyBorder,
            StrokeThickness = 0.5,
            RadiusX = 0,
            RadiusY = 4,
            Tag = key.NoteNumber,
            Effect = new DropShadowEffect
            {
                Color = Colors.Black,
                Direction = 270,
                ShadowDepth = 1,
                Opacity = 0.15,
                BlurRadius = 2
            }
        };
        Canvas.SetLeft(rect, x);
        Canvas.SetTop(rect, 0);
        Panel.SetZIndex(rect, 0);
        return rect;
    }

    private Rectangle CreateBlackKey(double x, PianoKey key)
    {
        var rect = new Rectangle
        {
            Width = BlackKeyWidth,
            Height = BlackKeyHeight,
            Stroke = new SolidColorBrush(Color.FromRgb(20, 20, 20)),
            StrokeThickness = 0.5,
            RadiusX = 2,
            RadiusY = 2,
            Tag = key.NoteNumber,
            Effect = new DropShadowEffect
            {
                Color = Colors.Black,
                Direction = 315,
                ShadowDepth = 3,
                Opacity = 0.5,
                BlurRadius = 5
            }
        };
        Canvas.SetLeft(rect, x);
        Canvas.SetTop(rect, 0);
        Panel.SetZIndex(rect, 1);
        return rect;
    }

    /// <summary>C notes show the full name with octave, everything else just the letter.</summary>
    /// <remarks>
    /// The C test is on the note number, not the text: in a flat key "Db" contains no "C".
    /// </remarks>
    private static string LabelTextFor(PianoKey key) =>
        IsOctaveC(key)
            ? key.NoteName
            : key.NoteName.TrimEnd('0', '1', '2', '3', '4', '5', '6', '7', '8', '9');

    private static bool IsOctaveC(PianoKey key) => key.NoteNumber % 12 == 0;

    private TextBlock CreateWhiteKeyLabel(double x, PianoKey key)
    {
        bool isC = IsOctaveC(key);

        var label = new TextBlock
        {
            Text = LabelTextFor(key),
            FontSize = isC ? 10 : 9,
            FontWeight = FontWeights.Medium,
            FontFamily = new FontFamily("Segoe UI"),
            Foreground = WhiteKeyLabel,
            TextAlignment = TextAlignment.Center,
            Width = KeyboardLayout.WhiteKeyDrawnWidth
        };
        Canvas.SetLeft(label, x);
        Canvas.SetTop(label, WhiteKeyHeight - (isC ? 18 : 16));
        Panel.SetZIndex(label, 0);
        return label;
    }

    private TextBlock CreateBlackKeyLabel(double x, PianoKey key)
    {
        var label = new TextBlock
        {
            Text = LabelTextFor(key),
            FontSize = 8,
            FontWeight = FontWeights.Medium,
            FontFamily = new FontFamily("Segoe UI"),
            Foreground = BlackKeyLabel,
            TextAlignment = TextAlignment.Center,
            Width = BlackKeyWidth
        };
        Canvas.SetLeft(label, x);
        Canvas.SetTop(label, BlackKeyHeight - 14);
        Panel.SetZIndex(label, 2);
        return label;
    }

    /// <summary>
    /// An inset outline plus a dot above the label: the outline marks the whole key, the dot
    /// stays findable on a white key's lower half, where black keys never cover it.
    /// </summary>
    /// <remarks>
    /// A white key's hint shares its Z-index, so the black keys drawn later still cover it;
    /// a black key's sits above the key itself, level with its label.
    /// </remarks>
    private static Grid CreateKeyHint(double x, PianoKey key)
    {
        double width = key.IsBlack ? BlackKeyWidth : KeyboardLayout.WhiteKeyDrawnWidth;
        double height = key.IsBlack ? BlackKeyHeight : WhiteKeyHeight;
        double dot = key.IsBlack ? 7 : 8;

        var hint = new Grid
        {
            Width = width,
            Height = height,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        hint.Children.Add(new Rectangle
        {
            Margin = new Thickness(key.IsBlack ? 1.5 : 2),
            Stroke = HintBrush,
            StrokeThickness = key.IsBlack ? 1.5 : 2,
            RadiusX = 2,
            RadiusY = 2
        });
        hint.Children.Add(new Ellipse
        {
            Width = dot,
            Height = dot,
            Fill = HintBrush,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, 0, key.IsBlack ? 22 : 32)
        });

        Canvas.SetLeft(hint, x);
        Canvas.SetTop(hint, 0);
        Panel.SetZIndex(hint, key.IsBlack ? 2 : 0);
        return hint;
    }

    /// <summary>
    /// Resolves a key's fill from all of its states at once. Every fill change goes through
    /// here: deciding "pressed or not" in isolation is exactly what would make releasing a
    /// key wipe out its key-signature highlight.
    /// </summary>
    private void ApplyKeyFill(PianoKey key)
    {
        if (!_keyRectangles.TryGetValue(key.NoteNumber, out var rect))
            return;

        rect.Fill = key.IsPressed
            ? (key.IsBlack ? BlackKeyPressedGradient : WhiteKeyPressedGradient)
            : key.ScaleRole switch
            {
                KeyRole.Tonic => key.IsBlack ? BlackKeyTonicGradient : WhiteKeyTonicGradient,
                KeyRole.InKey => key.IsBlack ? BlackKeyInKeyGradient : WhiteKeyInKeyGradient,
                _ => key.IsBlack ? BlackKeyGradient : WhiteKeyGradient
            };
    }

    private void ApplyKeyHint(PianoKey key)
    {
        if (_keyHints.TryGetValue(key.NoteNumber, out var hint))
            hint.Visibility = key.IsHinted ? Visibility.Visible : Visibility.Collapsed;
    }

    private void OnKeyPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (sender is not PianoKey key) return;

        switch (e.PropertyName)
        {
            case nameof(PianoKey.IsPressed):
            case nameof(PianoKey.ScaleRole):
                ApplyKeyFill(key);
                break;

            case nameof(PianoKey.IsHinted):
                ApplyKeyHint(key);
                break;

            case nameof(PianoKey.NoteName):
                if (_keyLabels.TryGetValue(key.NoteNumber, out var label))
                    label.Text = LabelTextFor(key);
                break;
        }
    }
}
