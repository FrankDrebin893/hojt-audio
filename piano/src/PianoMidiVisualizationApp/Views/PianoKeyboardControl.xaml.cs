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

    /// <summary>Where the label glyphs sit, measured from the top of the key.</summary>
    /// <remarks>
    /// Labels are placed by baseline rather than by top, so a held key's larger label sits on
    /// the same line as the idle one. The white-key baseline leaves the front lip clear for the
    /// scale mark below it.
    /// </remarks>
    private const double WhiteLabelBaseline = WhiteKeyHeight - 8;
    private const double BlackLabelBaseline = BlackKeyHeight - 6;

    private static readonly FontFamily LabelFont = new("Segoe UI");

    private readonly Dictionary<int, Rectangle> _keyRectangles = new();

    /// <summary>Where each key is drawn. Set when the keyboard is built; also read by the falling-notes view.</summary>
    public KeyboardLayout Layout { get; private set; } = KeyboardLayout.Default;

    /// <summary>Labels are tracked too, because the selected key re-spells their text.</summary>
    private readonly Dictionary<int, TextBlock> _keyLabels = new();

    /// <summary>Each key's hint overlay, collapsed until the key is hinted.</summary>
    private readonly Dictionary<int, Grid> _keyHints = new();

    /// <summary>Each key's scale mark: the edge that shows the key's role where its fill can't.</summary>
    private readonly Dictionary<int, Rectangle> _keyMarks = new();

    private PianoKeyboardViewModel? _viewModel;
    private KeyPalette _palette = null!;

    public PianoKeyboardControl()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (e.OldValue is PianoKeyboardViewModel oldVm)
        {
            oldVm.PropertyChanged -= OnKeyboardPropertyChanged;
            foreach (var key in oldVm.Keys)
                key.PropertyChanged -= OnKeyPropertyChanged;
        }

        _viewModel = e.NewValue as PianoKeyboardViewModel;
        if (_viewModel is { } vm)
        {
            vm.PropertyChanged += OnKeyboardPropertyChanged;
            BuildKeyboard(vm);
        }
    }

    private void BuildKeyboard(PianoKeyboardViewModel vm)
    {
        PianoCanvas.Children.Clear();
        _keyRectangles.Clear();
        _keyLabels.Clear();
        _keyHints.Clear();
        _keyMarks.Clear();

        if (vm.Keys.Count == 0) return;
        Layout = new KeyboardLayout(vm.Keys[0].NoteNumber, vm.Keys[^1].NoteNumber);
        _palette = new KeyPalette(this);

        // White keys first, with their marks, labels and hints; the black keys added after them
        // then sit on top.
        foreach (var key in vm.Keys.Where(k => !k.IsBlack))
            AddKey(key, CreateWhiteKey(Layout.KeyLeft(key.NoteNumber)));

        foreach (var key in vm.Keys.Where(k => k.IsBlack))
            AddKey(key, CreateBlackKey(Layout.KeyLeft(key.NoteNumber)));

        // Paint in the current state. Done after registration rather than inside the Create*
        // helpers so a key, role or hint set before this control existed still shows.
        foreach (var key in vm.Keys)
        {
            ApplyKeyFill(key);
            ApplyScaleMark(key);
            ApplyKeyLabel(key);
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

    /// <summary>
    /// Adds a key and its overlays in drawing order: body, label, hint, scale mark. The mark
    /// goes last so a hinted key still shows its role: it lies over the hint outline's edge
    /// and closes it off.
    /// </summary>
    private void AddKey(PianoKey key, Rectangle body)
    {
        double x = Layout.KeyLeft(key.NoteNumber);
        body.Tag = key.NoteNumber;

        var label = CreateLabel(x, key);
        var hint = CreateKeyHint(x, key);
        var mark = CreateScaleMark(x, key);

        PianoCanvas.Children.Add(body);
        PianoCanvas.Children.Add(label);
        PianoCanvas.Children.Add(hint);
        PianoCanvas.Children.Add(mark);

        _keyRectangles[key.NoteNumber] = body;
        _keyMarks[key.NoteNumber] = mark;
        _keyLabels[key.NoteNumber] = label;
        _keyHints[key.NoteNumber] = hint;
    }

    private Rectangle CreateWhiteKey(double x)
    {
        // Fill is deliberately left unset here; ApplyKeyFill assigns it once the key is
        // registered, so there is exactly one place that decides a key's colour.
        var rect = new Rectangle
        {
            Width = KeyboardLayout.WhiteKeyDrawnWidth,
            Height = WhiteKeyHeight,
            Stroke = _palette.WhiteBorder,
            StrokeThickness = 0.5,
            RadiusX = 0,
            RadiusY = 4,
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

    private Rectangle CreateBlackKey(double x)
    {
        var rect = new Rectangle
        {
            Width = BlackKeyWidth,
            Height = BlackKeyHeight,
            Stroke = _palette.BlackBorder,
            StrokeThickness = 0.5,
            RadiusX = 2,
            RadiusY = 2,
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

    /// <summary>Text, size, weight, colour and visibility are all set by <see cref="ApplyKeyLabel"/>.</summary>
    private static TextBlock CreateLabel(double x, PianoKey key)
    {
        var label = new TextBlock
        {
            FontFamily = LabelFont,
            TextAlignment = TextAlignment.Center,
            Width = KeyboardLayout.KeyWidth(key.NoteNumber),
            IsHitTestVisible = false
        };
        Canvas.SetLeft(label, x);
        Panel.SetZIndex(label, key.IsBlack ? 2 : 0);
        return label;
    }

    /// <summary>
    /// A bar along one edge of the key: the top of a black key, the front lip of a white one.
    /// Collapsed until <see cref="ApplyScaleMark"/> decides the key needs it.
    /// </summary>
    /// <remarks>
    /// Inset exactly as far as the hint outline (see <see cref="CreateKeyHint"/>), so on a
    /// hinted key the bar replaces that edge of the outline cleanly.
    /// </remarks>
    private static Rectangle CreateScaleMark(double x, PianoKey key)
    {
        double inset = HintInset(key);
        var mark = new Rectangle
        {
            Width = KeyboardLayout.KeyWidth(key.NoteNumber) - (2 * inset),
            RadiusX = 1,
            RadiusY = 1,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        Canvas.SetLeft(mark, x + inset);
        Panel.SetZIndex(mark, key.IsBlack ? 2 : 0);
        return mark;
    }

    /// <summary>
    /// An inset outline plus a dot above the label: the outline marks the whole key, the dot
    /// stays findable on a white key's lower half, where black keys never cover it. Both carry
    /// a deep keyline, so they still separate from a light fill as bright as the magenta.
    /// </summary>
    /// <remarks>
    /// A white key's hint shares its Z-index, so the black keys drawn later still cover it;
    /// a black key's sits above the key itself, level with its label.
    /// </remarks>
    private Grid CreateKeyHint(double x, PianoKey key)
    {
        double width = KeyboardLayout.KeyWidth(key.NoteNumber);
        double height = key.IsBlack ? BlackKeyHeight : WhiteKeyHeight;
        double dot = key.IsBlack ? 7 : 8;
        double inset = HintInset(key);
        double stroke = key.IsBlack ? 1.5 : 2;
        double keyline = key.IsBlack ? 0.75 : 1;

        var hint = new Grid
        {
            Width = width,
            Height = height,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        hint.Children.Add(new Rectangle
        {
            Margin = new Thickness(inset),
            Stroke = _palette.Hint,
            StrokeThickness = stroke,
            RadiusX = 2,
            RadiusY = 2
        });
        hint.Children.Add(new Rectangle
        {
            Margin = new Thickness(inset + stroke),
            Stroke = _palette.HintKeyline,
            StrokeThickness = keyline,
            RadiusX = 1,
            RadiusY = 1
        });
        hint.Children.Add(new Ellipse
        {
            Width = dot,
            Height = dot,
            Fill = _palette.Hint,
            Stroke = _palette.HintKeyline,
            StrokeThickness = keyline,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, 0, key.IsBlack ? 22 : 32)
        });

        Canvas.SetLeft(hint, x);
        Canvas.SetTop(hint, 0);
        Panel.SetZIndex(hint, key.IsBlack ? 2 : 0);
        return hint;
    }

    /// <summary>How far the hint outline, and the scale mark that can lie over it, sit inside the key.</summary>
    private static double HintInset(PianoKey key) => key.IsBlack ? 1.5 : 2;

    /// <summary>
    /// Resolves a key's fill from all of its states at once. Every fill change goes through
    /// here: deciding "pressed or not" in isolation is exactly what would make releasing a
    /// key wipe out its key-signature highlight.
    /// </summary>
    /// <remarks>
    /// Only white keys take the in-key wash. An in-key black key keeps its plain body and
    /// shows its role with the edge mark instead (see <see cref="ApplyScaleMark"/>).
    /// </remarks>
    private void ApplyKeyFill(PianoKey key)
    {
        if (!_keyRectangles.TryGetValue(key.NoteNumber, out var rect))
            return;

        var p = _palette;
        rect.Fill = key.IsPressed
            ? (key.IsBlack ? p.BlackHeld : p.WhiteHeld)
            : key.ScaleRole switch
            {
                KeyRole.Tonic => key.IsBlack ? p.BlackTonic : p.WhiteTonic,
                KeyRole.InKey => key.IsBlack ? p.Black : p.WhiteInKey,
                _ => key.IsBlack ? p.Black : p.White
            };
    }

    /// <summary>
    /// Shows the key's scale role as an edge mark wherever the fill can't: always on a black
    /// key (in-key black keys have no wash), and on a held white key, whose wash the held blue
    /// has replaced. So a held tonic still reads as the tonic.
    /// </summary>
    private void ApplyScaleMark(PianoKey key)
    {
        if (!_keyMarks.TryGetValue(key.NoteNumber, out var mark))
            return;

        bool show = key.ScaleRole != KeyRole.None && (key.IsBlack || key.IsPressed);
        mark.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;

        bool tonic = key.ScaleRole == KeyRole.Tonic;
        mark.Fill = tonic ? _palette.TonicMark : _palette.InKeyMark;
        mark.Height = tonic ? 4 : 2.5;
        double inset = HintInset(key);
        Canvas.SetTop(mark, key.IsBlack ? inset : WhiteKeyHeight - inset - mark.Height);
    }

    /// <summary>
    /// Three label roles. A held key's name is what you're reading, so it gets the high-contrast
    /// pair for its fill, a heavier weight and a step up in size. Idle, the Cs are landmarks
    /// (with the octave), and the other keys are labelled only with "All note names".
    /// </summary>
    private void ApplyKeyLabel(PianoKey key)
    {
        if (!_keyLabels.TryGetValue(key.NoteNumber, out var label))
            return;

        bool held = key.IsPressed;
        bool show = held || IsOctaveC(key) || _viewModel?.ShowAllNoteNames == true;
        label.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;

        var p = _palette;
        label.Text = LabelTextFor(key);
        label.FontWeight = held ? FontWeights.SemiBold : FontWeights.Medium;
        if (key.IsBlack)
        {
            label.Foreground = held ? p.BlackLabelHeld : p.BlackLabel;
            label.FontSize = held ? 9 : 8;
        }
        else
        {
            label.Foreground = held ? p.LabelHeld : p.Label;
            label.FontSize = held ? 11 : IsOctaveC(key) ? 10 : 9;
        }

        double baseline = key.IsBlack ? BlackLabelBaseline : WhiteLabelBaseline;
        Canvas.SetTop(label, baseline - (LabelFont.Baseline * label.FontSize));
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
                ApplyKeyFill(key);
                ApplyScaleMark(key);
                ApplyKeyLabel(key);
                break;

            case nameof(PianoKey.ScaleRole):
                ApplyKeyFill(key);
                ApplyScaleMark(key);
                break;

            case nameof(PianoKey.IsHinted):
                ApplyKeyHint(key);
                break;

            case nameof(PianoKey.NoteName):
                ApplyKeyLabel(key);
                break;
        }
    }

    private void OnKeyboardPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PianoKeyboardViewModel.ShowAllNoteNames) && _viewModel is { } vm)
        {
            foreach (var key in vm.Keys)
                ApplyKeyLabel(key);
        }
    }

    /// <summary>
    /// The keyboard's brushes, resolved once per build from the theme (Themes/Dark.xaml), where
    /// each one is named for what it means.
    /// </summary>
    private sealed class KeyPalette
    {
        public Brush White { get; }
        public Brush WhiteHeld { get; }
        public Brush WhiteInKey { get; }
        public Brush WhiteTonic { get; }
        public Brush WhiteBorder { get; }
        public Brush Black { get; }
        public Brush BlackHeld { get; }
        public Brush BlackTonic { get; }
        public Brush BlackBorder { get; }
        public Brush InKeyMark { get; }
        public Brush TonicMark { get; }
        public Brush Hint { get; }
        public Brush HintKeyline { get; }
        public Brush Label { get; }
        public Brush LabelHeld { get; }
        public Brush BlackLabel { get; }
        public Brush BlackLabelHeld { get; }

        public KeyPalette(FrameworkElement owner)
        {
            // Magenta for a missing key is loud on purpose: a mistyped name shows at once.
            Brush Find(string key) => owner.TryFindResource(key) as Brush ?? Brushes.Magenta;

            White = Find("Brush.Key.White");
            WhiteHeld = Find("Brush.Key.White.Held");
            WhiteInKey = Find("Brush.Key.White.InKey");
            WhiteTonic = Find("Brush.Key.White.Tonic");
            WhiteBorder = Find("Brush.Key.White.Border");
            Black = Find("Brush.Key.Black");
            BlackHeld = Find("Brush.Key.Black.Held");
            BlackTonic = Find("Brush.Key.Black.Tonic");
            BlackBorder = Find("Brush.Key.Black.Border");
            InKeyMark = Find("Brush.Key.InKey.Mark");
            TonicMark = Find("Brush.Key.Tonic.Mark");
            Hint = Find("Brush.Key.Hint");
            HintKeyline = Find("Brush.Key.Hint.Keyline");
            Label = Find("Brush.Key.Label");
            LabelHeld = Find("Brush.Key.Label.Held");
            BlackLabel = Find("Brush.Key.Black.Label");
            BlackLabelHeld = Find("Brush.Key.Black.Label.Held");
        }
    }
}
