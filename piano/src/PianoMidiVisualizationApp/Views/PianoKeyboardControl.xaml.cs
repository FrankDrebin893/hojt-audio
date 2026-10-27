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

    /// <summary>Height of the 3D vertical front skirt and overhanging bullnose lip at the bottom of each white key.</summary>
    private const double WhiteSkirtHeight = 13.5;
    private const double WhiteSkirtPressedHeight = 9.5;
    private const double WhiteBullnoseHeight = 3.0;

    /// <summary>Inset and sloped nose height for the 4-plane 3D sculpted ebony prism of each black key.</summary>
    private const double BlackTopInset = 2.2;
    private const double BlackNoseHeight = 15.5;
    private const double BlackNosePressedHeight = 12.0;

    /// <summary>Where the label glyphs sit, measured from the top of the key.</summary>
    /// <remarks>
    /// Labels are placed by baseline rather than by top, so a held key's larger label sits on
    /// the same line as the idle one. The white-key baseline sits on the keytop above the
    /// 3D bullnose overhang, vertical skirt, and scale mark; the black-key baseline sits on the
    /// raised satin playing surface above the sloped front nose crest.
    /// </remarks>
    private const double WhiteLabelBaseline = WhiteKeyHeight - 24;
    private const double BlackLabelBaseline = BlackKeyHeight - 19;

    /// <summary>The theme's body face (Font.Body), resolved when the keyboard is built.</summary>
    private FontFamily _labelFont = new("Segoe UI");

    private sealed class KeyVisual
    {
        public required Rectangle Body { get; init; }
        public DropShadowEffect? ShadowEffect { get; init; }
        public Rectangle? Chamfer { get; init; }
        public Rectangle? Skirt { get; init; }
        public Rectangle? Undercut { get; init; }
        public Rectangle? Bullnose { get; init; }
        public Line? WhiteRidgeLine { get; init; }
        public Rectangle? PressedRecess { get; init; }
        public Polygon? BlackContactShadow { get; init; }
        public Polygon? BlackLeftWall { get; init; }
        public Polygon? BlackRightWall { get; init; }
        public Rectangle? BlackTop { get; init; }
        public Polygon? BlackNose { get; init; }
        public Rectangle? BlackNoseCrest { get; init; }
        public Line? BlackRidgeLine { get; init; }
        public Line? BlackLeftFacetLine { get; init; }
        public Line? BlackRightFacetLine { get; init; }
        public required Rectangle LedHousing { get; init; }
        public required Rectangle LedGlow { get; init; }
        public required Rectangle LedBar { get; init; }
    }

    private readonly Dictionary<int, KeyVisual> _keyVisuals = new();

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
        _keyVisuals.Clear();
        _keyLabels.Clear();
        _keyHints.Clear();
        _keyMarks.Clear();

        if (vm.Keys.Count == 0) return;
        Layout = new KeyboardLayout(vm.Keys[0].NoteNumber, vm.Keys[^1].NoteNumber);
        _palette = new KeyPalette(this);
        if (TryFindResource("Font.Body") is FontFamily labelFont)
            _labelFont = labelFont;

        // White keys first, with their 3D skirts, chamfers, marks, labels and hints; the black keys
        // added after them then sit on top and cast shadows onto the white keys.
        foreach (var key in vm.Keys.Where(k => !k.IsBlack))
            AddKey(key, CreateWhiteKey(Layout.KeyLeft(key.NoteNumber)));

        foreach (var key in vm.Keys.Where(k => k.IsBlack))
            AddKey(key, CreateBlackKey(Layout.KeyLeft(key.NoteNumber)));

        // Soft top shadow cast across the back of the keybed by the overhanging front panel.
        var fallboardShadow = new Rectangle
        {
            Width = Layout.TotalWidth,
            Height = 10,
            Fill = _palette.FallboardShadow,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(fallboardShadow, 0);
        Canvas.SetTop(fallboardShadow, 0);
        Panel.SetZIndex(fallboardShadow, 3);
        PianoCanvas.Children.Add(fallboardShadow);

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
    /// Adds a key's 3D geometry and overlays in drawing order: body, 3D facets/skirt, key-light LED,
    /// label, hint, scale mark. The mark goes last so a hinted key still shows its role: it lies
    /// over the hint outline's edge and closes it off.
    /// </summary>
    private void AddKey(PianoKey key, KeyVisual visual)
    {
        double x = Layout.KeyLeft(key.NoteNumber);
        visual.Body.Tag = key.NoteNumber;

        var label = CreateLabel(x, key);
        var hint = CreateKeyHint(x, key);
        var mark = CreateScaleMark(x, key);

        if (visual.BlackContactShadow is { } contactShadow) PianoCanvas.Children.Add(contactShadow);
        PianoCanvas.Children.Add(visual.Body);
        if (visual.Chamfer is { } chamfer) PianoCanvas.Children.Add(chamfer);
        if (visual.PressedRecess is { } recess) PianoCanvas.Children.Add(recess);
        if (visual.Skirt is { } skirt) PianoCanvas.Children.Add(skirt);
        if (visual.Undercut is { } undercut) PianoCanvas.Children.Add(undercut);
        if (visual.Bullnose is { } bullnose) PianoCanvas.Children.Add(bullnose);
        if (visual.WhiteRidgeLine is { } whiteRidge) PianoCanvas.Children.Add(whiteRidge);
        if (visual.BlackLeftWall is { } leftWall) PianoCanvas.Children.Add(leftWall);
        if (visual.BlackRightWall is { } rightWall) PianoCanvas.Children.Add(rightWall);
        if (visual.BlackNose is { } nose) PianoCanvas.Children.Add(nose);
        if (visual.BlackTop is { } top) PianoCanvas.Children.Add(top);
        if (visual.BlackNoseCrest is { } noseCrest) PianoCanvas.Children.Add(noseCrest);
        if (visual.BlackLeftFacetLine is { } leftFacet) PianoCanvas.Children.Add(leftFacet);
        if (visual.BlackRightFacetLine is { } rightFacet) PianoCanvas.Children.Add(rightFacet);
        if (visual.BlackRidgeLine is { } ridge) PianoCanvas.Children.Add(ridge);
        PianoCanvas.Children.Add(visual.LedGlow);
        PianoCanvas.Children.Add(visual.LedHousing);
        PianoCanvas.Children.Add(visual.LedBar);
        PianoCanvas.Children.Add(label);
        PianoCanvas.Children.Add(hint);
        PianoCanvas.Children.Add(mark);

        _keyVisuals[key.NoteNumber] = visual;
        _keyMarks[key.NoteNumber] = mark;
        _keyLabels[key.NoteNumber] = label;
        _keyHints[key.NoteNumber] = hint;
    }

    private KeyVisual CreateWhiteKey(double x)
    {
        double w = KeyboardLayout.WhiteKeyDrawnWidth;
        var shadow = new DropShadowEffect
        {
            Color = _palette.Shadow,
            Direction = 270,
            ShadowDepth = 2.5,
            Opacity = 0.55,
            BlurRadius = 3.5
        };

        var body = new Rectangle
        {
            Width = w,
            Height = WhiteKeyHeight,
            Stroke = _palette.WhiteBorder,
            StrokeThickness = 0.8,
            RadiusX = 2.5,
            RadiusY = 2.5,
            Effect = shadow
        };
        Canvas.SetLeft(body, x);
        Canvas.SetTop(body, 0);
        Panel.SetZIndex(body, 0);

        // 3D cylindrical side-bevel sheen across the horizontal playing surface.
        var chamfer = new Rectangle
        {
            Width = w - 1.4,
            Height = WhiteKeyHeight - WhiteSkirtHeight,
            Fill = _palette.WhiteChamfer,
            RadiusX = 1.8,
            RadiusY = 1.8,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(chamfer, x + 0.7);
        Canvas.SetTop(chamfer, 0.5);
        Panel.SetZIndex(chamfer, 0);

        // Deep 3D vertical front skirt (lip) at the bottom of the white key.
        var skirt = new Rectangle
        {
            Width = w - 1.4,
            Height = WhiteSkirtHeight,
            Fill = _palette.WhiteSkirt,
            RadiusX = 2.0,
            RadiusY = 2.0,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(skirt, x + 0.7);
        Canvas.SetTop(skirt, WhiteKeyHeight - WhiteSkirtHeight - 0.5);
        Panel.SetZIndex(skirt, 0);

        // Crisp dark undercut shadow cast by the overhanging bullnose lip onto the vertical skirt.
        var undercut = new Rectangle
        {
            Width = w - 1.4,
            Height = 3.2,
            Fill = _palette.WhiteUndercut,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(undercut, x + 0.7);
        Canvas.SetTop(undercut, WhiteKeyHeight - WhiteSkirtHeight + 1.0);
        Panel.SetZIndex(undercut, 0);

        // Rounded 3D overhanging bullnose lip where the horizontal playing surface rolls over the front.
        var bullnose = new Rectangle
        {
            Width = w - 1.2,
            Height = WhiteBullnoseHeight,
            Fill = _palette.WhiteBullnose,
            RadiusX = 1.2,
            RadiusY = 1.2,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(bullnose, x + 0.6);
        Canvas.SetTop(bullnose, WhiteKeyHeight - WhiteSkirtHeight - 1.5);
        Panel.SetZIndex(bullnose, 0);

        // Fine hairline seam below the bullnose overhang.
        var whiteRidge = new Line
        {
            X1 = 0.8,
            X2 = w - 0.8,
            Stroke = _palette.WhiteRidge,
            StrokeThickness = 0.7,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(whiteRidge, x);
        Canvas.SetTop(whiteRidge, 0);
        Panel.SetZIndex(whiteRidge, 0);

        // Recessed side-wall shadows cast onto a depressed key by its higher unpressed neighbours.
        var pressedRecess = new Rectangle
        {
            Width = w - 1.2,
            Height = WhiteKeyHeight - WhiteSkirtPressedHeight - 1,
            Fill = _palette.PressedRecess,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        Canvas.SetLeft(pressedRecess, x + 0.6);
        Canvas.SetTop(pressedRecess, 1);
        Panel.SetZIndex(pressedRecess, 0);

        // Hardware Light Guide LED diffuser housing, radial glow & lens bar at the top of the key.
        var ledGlow = new Rectangle
        {
            Width = w - 1.4,
            Height = 28,
            Fill = _palette.KeyLedGlow,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        Canvas.SetLeft(ledGlow, x + 0.7);
        Canvas.SetTop(ledGlow, 1);
        Panel.SetZIndex(ledGlow, 0);

        var ledHousing = new Rectangle
        {
            Width = w - 4.0,
            Height = 5.4,
            Fill = _palette.KeyLedHousing,
            RadiusX = 1.4,
            RadiusY = 1.4,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(ledHousing, x + 2.0);
        Canvas.SetTop(ledHousing, 2.2);
        Panel.SetZIndex(ledHousing, 0);

        var ledBar = new Rectangle
        {
            Width = w - 5.2,
            Height = 4.2,
            Fill = _palette.KeyLedOff,
            RadiusX = 1.0,
            RadiusY = 1.0,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(ledBar, x + 2.6);
        Canvas.SetTop(ledBar, 2.8);
        Panel.SetZIndex(ledBar, 0);

        return new KeyVisual
        {
            Body = body,
            ShadowEffect = shadow,
            Chamfer = chamfer,
            Skirt = skirt,
            Undercut = undercut,
            Bullnose = bullnose,
            WhiteRidgeLine = whiteRidge,
            PressedRecess = pressedRecess,
            LedHousing = ledHousing,
            LedGlow = ledGlow,
            LedBar = ledBar
        };
    }

    private KeyVisual CreateBlackKey(double x)
    {
        // Tight, grounded contact shadow flush against the black key's right and front base on the
        // white key surface (zero offset gap, so the key is 100% planted in its slot).
        var contactShadow = new Polygon
        {
            Fill = _palette.BlackContactShadow,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(contactShadow, x);
        Canvas.SetTop(contactShadow, 0);
        Panel.SetZIndex(contactShadow, 1);

        // Outer ebony slot frame.
        var body = new Rectangle
        {
            Width = BlackKeyWidth,
            Height = BlackKeyHeight,
            Stroke = _palette.BlackBorder,
            StrokeThickness = 0.9,
            RadiusX = 2.0,
            RadiusY = 2.0
        };
        Canvas.SetLeft(body, x);
        Canvas.SetTop(body, 0);
        Panel.SetZIndex(body, 1);

        // Lit 3D left side-bevel wall sloping down from the narrow top surface to the base slot.
        var leftWall = new Polygon
        {
            Fill = _palette.BlackLeftWall,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(leftWall, x);
        Canvas.SetTop(leftWall, 0);
        Panel.SetZIndex(leftWall, 1);

        // Deep shaded 3D right side-bevel wall.
        var rightWall = new Polygon
        {
            Fill = _palette.BlackRightWall,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(rightWall, x);
        Canvas.SetTop(rightWall, 0);
        Panel.SetZIndex(rightWall, 1);

        // Trapezoidal front nose ramp sloping down from the raised playing surface to the front rim.
        var nose = new Polygon
        {
            Fill = _palette.BlackNose,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(nose, x);
        Canvas.SetTop(nose, 0);
        Panel.SetZIndex(nose, 1);

        // Narrow raised satin playing surface atop the 4-plane 3D ebony prism.
        var top = new Rectangle
        {
            Width = BlackKeyWidth - (2 * BlackTopInset),
            Height = BlackKeyHeight - BlackNoseHeight - 1,
            Fill = _palette.BlackTop,
            RadiusX = 1.2,
            RadiusY = 1.2,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(top, x + BlackTopInset);
        Canvas.SetTop(top, 1);
        Panel.SetZIndex(top, 1);

        // Glossy curved 3D specular crest roll where the horizontal top surface bends into the nose slope.
        var noseCrest = new Rectangle
        {
            Width = BlackKeyWidth - (2 * BlackTopInset),
            Height = 2.8,
            Fill = _palette.BlackNoseCrest,
            RadiusX = 0.8,
            RadiusY = 0.8,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(noseCrest, x + BlackTopInset);
        Canvas.SetTop(noseCrest, BlackKeyHeight - BlackNoseHeight - 1.0);
        Panel.SetZIndex(noseCrest, 1);

        // Diagonal left and right 3D facet ridges along the sloped front nose edges.
        var leftFacet = new Line
        {
            X1 = BlackTopInset,
            X2 = 0.8,
            Stroke = _palette.BlackRidge,
            StrokeThickness = 0.7,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(leftFacet, x);
        Canvas.SetTop(leftFacet, 0);
        Panel.SetZIndex(leftFacet, 1);

        var rightFacet = new Line
        {
            X1 = BlackKeyWidth - BlackTopInset,
            X2 = BlackKeyWidth - 0.8,
            Stroke = _palette.BlackBorder,
            StrokeThickness = 0.7,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(rightFacet, x);
        Canvas.SetTop(rightFacet, 0);
        Panel.SetZIndex(rightFacet, 1);

        // Bright horizontal specular highlight ridge along the top crest of the nose.
        var ridgeLine = new Line
        {
            X1 = BlackTopInset + 0.3,
            X2 = BlackKeyWidth - BlackTopInset - 0.3,
            Stroke = _palette.BlackRidge,
            StrokeThickness = 0.9,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(ridgeLine, x);
        Canvas.SetTop(ridgeLine, 0);
        Panel.SetZIndex(ridgeLine, 1);

        var ledGlow = new Rectangle
        {
            Width = BlackKeyWidth - (2 * BlackTopInset),
            Height = 22,
            Fill = _palette.KeyLedGlow,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed
        };
        Canvas.SetLeft(ledGlow, x + BlackTopInset);
        Canvas.SetTop(ledGlow, 1);
        Panel.SetZIndex(ledGlow, 1);

        var ledHousing = new Rectangle
        {
            Width = BlackKeyWidth - (2 * BlackTopInset) - 1.2,
            Height = 4.8,
            Fill = _palette.KeyLedHousing,
            RadiusX = 1.0,
            RadiusY = 1.0,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(ledHousing, x + BlackTopInset + 0.6);
        Canvas.SetTop(ledHousing, 2.0);
        Panel.SetZIndex(ledHousing, 1);

        var ledBar = new Rectangle
        {
            Width = BlackKeyWidth - (2 * BlackTopInset) - 2.2,
            Height = 3.8,
            Fill = _palette.KeyLedOff,
            RadiusX = 0.8,
            RadiusY = 0.8,
            IsHitTestVisible = false
        };
        Canvas.SetLeft(ledBar, x + BlackTopInset + 1.1);
        Canvas.SetTop(ledBar, 2.5);
        Panel.SetZIndex(ledBar, 1);

        return new KeyVisual
        {
            Body = body,
            BlackContactShadow = contactShadow,
            BlackLeftWall = leftWall,
            BlackRightWall = rightWall,
            BlackTop = top,
            BlackNose = nose,
            BlackNoseCrest = noseCrest,
            BlackLeftFacetLine = leftFacet,
            BlackRightFacetLine = rightFacet,
            BlackRidgeLine = ridgeLine,
            LedHousing = ledHousing,
            LedGlow = ledGlow,
            LedBar = ledBar
        };
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
    private TextBlock CreateLabel(double x, PianoKey key)
    {
        var label = new TextBlock
        {
            FontFamily = _labelFont,
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
        if (!_keyVisuals.TryGetValue(key.NoteNumber, out var v))
            return;

        var p = _palette;
        bool held = key.IsPressed;
        bool ringing = !held && key.IsSustained;
        bool outOfKey = !held && !ringing && key.ScaleRole == KeyRole.OutOfKey;

        v.Body.Fill = held
            ? (key.IsBlack ? p.BlackHeld : p.WhiteHeld)
            : ringing ? (key.IsBlack ? p.BlackRinging : p.WhiteRinging)
            : key.ScaleRole switch
            {
                KeyRole.Tonic => key.IsBlack ? p.BlackTonic : p.WhiteTonic,
                KeyRole.InKey => key.IsBlack ? p.BlackInKey : p.WhiteInKey,
                KeyRole.OutOfKey => key.IsBlack ? p.BlackOutOfKey : p.WhiteOutOfKey,
                _ => key.IsBlack ? p.Black : p.White
            };
        v.Body.Opacity = outOfKey ? (key.IsBlack ? 0.18 : 0.15) : 1.0;

        if (key.IsBlack)
        {
            // Depress the 4-plane 3D ebony prism when held: shift the top plane down and compress
            // its sloped front nose ramp within the key slot.
            double topY = held ? 2.6 : 1.0;
            double noseH = held ? BlackNosePressedHeight : BlackNoseHeight;
            double topH = BlackKeyHeight - noseH - topY;
            double noseTopY = topY + topH;
            double bottomY = BlackKeyHeight - 0.7;

            if (v.BlackContactShadow is { } contactShadow)
            {
                contactShadow.Opacity = outOfKey ? 0.0 : 1.0;
                double shadowRight = BlackKeyWidth + (held ? 1.1 : 1.8);
                double shadowBottom = BlackKeyHeight + (held ? 1.4 : 2.2);
                contactShadow.Points = new PointCollection
                {
                    new(BlackKeyWidth - 0.5, 1.0),
                    new(shadowRight, 2.0),
                    new(shadowRight, shadowBottom - 0.8),
                    new(shadowRight - 0.8, shadowBottom),
                    new(1.5, shadowBottom),
                    new(0.6, BlackKeyHeight - 0.5)
                };
            }

            if (v.BlackLeftWall is { } leftWall)
            {
                leftWall.Opacity = outOfKey ? 0.0 : 1.0;
                leftWall.Fill = held ? p.BlackLeftWallHeld : outOfKey ? p.BlackLeftWallOutOfKey : p.BlackLeftWall;
                leftWall.Points = new PointCollection
                {
                    new(0.6, 0.7),
                    new(BlackTopInset, topY),
                    new(BlackTopInset, noseTopY),
                    new(0.6, bottomY)
                };
            }

            if (v.BlackRightWall is { } rightWall)
            {
                rightWall.Opacity = outOfKey ? 0.0 : 1.0;
                rightWall.Fill = held ? p.BlackRightWallHeld : outOfKey ? p.BlackRightWallOutOfKey : p.BlackRightWall;
                rightWall.Points = new PointCollection
                {
                    new(BlackKeyWidth - BlackTopInset, topY),
                    new(BlackKeyWidth - 0.6, 0.7),
                    new(BlackKeyWidth - 0.6, bottomY),
                    new(BlackKeyWidth - BlackTopInset, noseTopY)
                };
            }

            if (v.BlackTop is { } top)
            {
                top.Opacity = outOfKey ? 0.10 : 1.0;
                Canvas.SetTop(top, topY);
                top.Height = topH;
                top.Fill = held
                    ? p.BlackTopHeld
                    : ringing
                        ? p.BlackTopRinging
                        : key.ScaleRole switch
                        {
                            KeyRole.Tonic => p.BlackTopTonic,
                            KeyRole.InKey => p.BlackTopInKey,
                            KeyRole.OutOfKey => p.BlackTopOutOfKey,
                            _ => p.BlackTop
                        };
            }

            if (v.BlackNose is { } nose)
            {
                nose.Opacity = outOfKey ? 0.0 : 1.0;
                nose.Fill = held
                    ? p.BlackNoseHeld
                    : outOfKey
                        ? p.BlackNoseOutOfKey
                        : key.ScaleRole is KeyRole.InKey or KeyRole.Tonic
                            ? p.BlackNoseInKey
                            : p.BlackNose;
                nose.Points = new PointCollection
                {
                    new(BlackTopInset, noseTopY),
                    new(BlackKeyWidth - BlackTopInset, noseTopY),
                    new(BlackKeyWidth - 0.6, bottomY),
                    new(0.6, bottomY)
                };
            }

            if (v.BlackNoseCrest is { } noseCrest)
            {
                noseCrest.Opacity = outOfKey ? 0.0 : 1.0;
                noseCrest.Fill = outOfKey ? p.BlackNoseCrestOutOfKey : p.BlackNoseCrest;
                Canvas.SetTop(noseCrest, noseTopY - 0.8);
            }

            if (v.BlackLeftFacetLine is { } leftFacet)
            {
                leftFacet.Opacity = outOfKey ? 0.15 : 1.0;
                leftFacet.Stroke = outOfKey ? p.BlackRidgeOutOfKey : p.BlackRidge;
                leftFacet.Y1 = noseTopY;
                leftFacet.Y2 = bottomY;
            }

            if (v.BlackRightFacetLine is { } rightFacet)
            {
                rightFacet.Opacity = outOfKey ? 0.15 : 1.0;
                rightFacet.Y1 = noseTopY;
                rightFacet.Y2 = bottomY;
            }

            if (v.BlackRidgeLine is { } ridge)
            {
                ridge.Opacity = outOfKey ? 0.15 : 1.0;
                ridge.Stroke = outOfKey ? p.BlackRidgeOutOfKey : p.BlackRidge;
                ridge.Y1 = noseTopY;
                ridge.Y2 = noseTopY;
            }
        }
        else
        {
            // Depress the white key when held: the 3D bullnose overhang and undercut shadow step
            // DOWN by 4px while the vertical front skirt compresses into the keybed well below.
            double skirtH = held ? WhiteSkirtPressedHeight : WhiteSkirtHeight;
            double skirtTop = WhiteKeyHeight - skirtH - 0.5;

            if (v.ShadowEffect is { } shadow)
            {
                shadow.ShadowDepth = held ? 1.0 : 2.5;
                shadow.Opacity = outOfKey ? 0.0 : (held ? 0.35 : 0.55);
            }

            if (v.Chamfer is { } chamfer)
            {
                chamfer.Height = skirtTop;
                chamfer.Opacity = outOfKey ? 0.0 : 1.0;
            }

            if (v.Skirt is { } skirt)
            {
                skirt.Height = skirtH;
                skirt.Opacity = outOfKey ? 0.12 : 1.0;
                skirt.Fill = held ? p.WhiteSkirtHeld : outOfKey ? p.WhiteSkirtOutOfKey : p.WhiteSkirt;
                Canvas.SetTop(skirt, skirtTop);
            }

            if (v.Undercut is { } undercut)
            {
                undercut.Opacity = outOfKey ? 0.0 : 1.0;
                Canvas.SetTop(undercut, skirtTop + 1.4);
            }

            if (v.Bullnose is { } bullnose)
            {
                bullnose.Opacity = outOfKey ? 0.12 : 1.0;
                bullnose.Fill = held ? p.WhiteBullnoseHeld : outOfKey ? p.WhiteBullnoseOutOfKey : p.WhiteBullnose;
                Canvas.SetTop(bullnose, skirtTop - 1.2);
            }

            if (v.WhiteRidgeLine is { } whiteRidge)
            {
                whiteRidge.Opacity = outOfKey ? 0.12 : 1.0;
                whiteRidge.Y1 = skirtTop + WhiteBullnoseHeight - 1.2;
                whiteRidge.Y2 = skirtTop + WhiteBullnoseHeight - 1.2;
            }

            if (v.PressedRecess is { } recess)
                recess.Visibility = held ? Visibility.Visible : Visibility.Collapsed;
        }

        // Physical hardware Light Guide LED window at the top edge of held or pedal-sustained keys.
        if (held || ringing)
        {
            v.LedBar.Fill = held ? p.KeyLedHeld : p.KeyLedRinging;
            v.LedBar.Height = held ? (key.IsBlack ? 4.4 : 5.0) : (key.IsBlack ? 3.8 : 4.2);
            v.LedHousing.Visibility = Visibility.Visible;
            v.LedBar.Visibility = Visibility.Visible;
            v.LedGlow.Visibility = held ? Visibility.Visible : Visibility.Collapsed;
        }
        else
        {
            v.LedHousing.Visibility = Visibility.Collapsed;
            v.LedBar.Visibility = Visibility.Collapsed;
            v.LedGlow.Visibility = Visibility.Collapsed;
        }
    }

    /// <summary>
    /// Shows the key's scale role as an edge mark on every key in the key, so the scale never
    /// rests on colour alone: the top edge of a black key (in-key black keys have no wash), the
    /// front edge of a white key's playing surface just above its 3D vertical skirt. A held or
    /// pedal-held white key keeps it too, so a held tonic still reads as the tonic once the
    /// blue has replaced its wash.
    /// </summary>
    /// <remarks>
    /// The light marks sit on ebony and on the held blue; an unheld white key takes the dark
    /// pair instead. Placing the white-key mark just above the 3D bullnose overhang keeps the
    /// sculpted key front visible in every key signature.
    /// </remarks>
    private void ApplyScaleMark(PianoKey key)
    {
        if (!_keyMarks.TryGetValue(key.NoteNumber, out var mark))
            return;

        bool show = key.ScaleRole is KeyRole.InKey or KeyRole.Tonic;
        mark.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;

        bool tonic = key.ScaleRole == KeyRole.Tonic;
        bool onIvory = !key.IsBlack && !key.IsPressed && !key.IsSustained;
        mark.Fill = onIvory
            ? (tonic ? _palette.TonicMarkIvory : _palette.InKeyMarkIvory)
            : (tonic ? _palette.TonicMark : _palette.InKeyMark);
        mark.Height = tonic ? 4.0 : 2.5;
        double inset = HintInset(key);
        double skirtH = key.IsPressed ? WhiteSkirtPressedHeight : WhiteSkirtHeight;
        Canvas.SetTop(mark, key.IsBlack ? inset : WhiteKeyHeight - skirtH - mark.Height - 1.5);
    }

    /// <summary>
    /// Three label roles. A sounding key's name is what you're reading, so it gets the
    /// high-contrast pair for its blue fill; one under a finger also gets a heavier weight and a
    /// step up in size, while one the pedal holds stays quieter. Idle, the Cs are landmarks (with
    /// the octave), and the other keys are labelled only with "All note names".
    /// </summary>
    private void ApplyKeyLabel(PianoKey key)
    {
        if (!_keyLabels.TryGetValue(key.NoteNumber, out var label))
            return;

        bool held = key.IsPressed;
        bool sounding = held || key.IsSustained;
        bool show = sounding || IsOctaveC(key) || _viewModel?.ShowAllNoteNames == true;
        label.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;

        var p = _palette;
        bool outOfKey = !sounding && key.ScaleRole == KeyRole.OutOfKey;
        label.Opacity = outOfKey ? 0.25 : 1.0;
        label.Text = LabelTextFor(key);
        label.FontWeight = held ? FontWeights.SemiBold : FontWeights.Medium;
        if (key.IsBlack)
        {
            label.Foreground = sounding ? p.BlackLabelHeld : outOfKey ? p.BlackLabelOutOfKey : p.BlackLabel;
            label.FontSize = held ? 9 : 8;
        }
        else
        {
            label.Foreground = sounding ? p.LabelHeld : outOfKey ? p.LabelOutOfKey : p.Label;
            label.FontSize = held ? 11 : IsOctaveC(key) ? 10 : 9;
        }

        double baseline = key.IsBlack ? BlackLabelBaseline : WhiteLabelBaseline;
        Canvas.SetTop(label, baseline - (_labelFont.Baseline * label.FontSize));
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
            case nameof(PianoKey.IsSustained):
                ApplyKeyFill(key);
                ApplyScaleMark(key);
                ApplyKeyLabel(key);
                break;

            case nameof(PianoKey.ScaleRole):
                ApplyKeyFill(key);
                ApplyScaleMark(key);
                ApplyKeyLabel(key);
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
        public Brush WhiteRinging { get; }
        public Brush WhiteInKey { get; }
        public Brush WhiteTonic { get; }
        public Brush WhiteOutOfKey { get; }
        public Brush WhiteBorder { get; }
        public Brush WhiteBullnose { get; }
        public Brush WhiteBullnoseOutOfKey { get; }
        public Brush WhiteBullnoseHeld { get; }
        public Brush WhiteUndercut { get; }
        public Brush WhiteSkirt { get; }
        public Brush WhiteSkirtOutOfKey { get; }
        public Brush WhiteSkirtHeld { get; }
        public Brush WhiteRidge { get; }
        public Brush WhiteChamfer { get; }
        public Brush PressedRecess { get; }
        public Brush KeyLedHousing { get; }
        public Brush KeyLedOff { get; }
        public Brush KeyLedHeld { get; }
        public Brush KeyLedRinging { get; }
        public Brush KeyLedGlow { get; }
        public Brush FallboardShadow { get; }
        public Brush BlackContactShadow { get; }
        public Brush Black { get; }
        public Brush BlackInKey { get; }
        public Brush BlackOutOfKey { get; }
        public Brush BlackLeftWall { get; }
        public Brush BlackLeftWallOutOfKey { get; }
        public Brush BlackLeftWallHeld { get; }
        public Brush BlackRightWall { get; }
        public Brush BlackRightWallOutOfKey { get; }
        public Brush BlackRightWallHeld { get; }
        public Brush BlackTop { get; }
        public Brush BlackTopInKey { get; }
        public Brush BlackTopTonic { get; }
        public Brush BlackTopOutOfKey { get; }
        public Brush BlackTopHeld { get; }
        public Brush BlackTopRinging { get; }
        public Brush BlackNose { get; }
        public Brush BlackNoseInKey { get; }
        public Brush BlackNoseOutOfKey { get; }
        public Brush BlackNoseHeld { get; }
        public Brush BlackNoseCrest { get; }
        public Brush BlackNoseCrestOutOfKey { get; }
        public Brush BlackRidge { get; }
        public Brush BlackRidgeOutOfKey { get; }
        public Brush BlackHeld { get; }
        public Brush BlackRinging { get; }
        public Brush BlackTonic { get; }
        public Brush BlackBorder { get; }
        public Brush InKeyMark { get; }
        public Brush TonicMark { get; }
        public Brush InKeyMarkIvory { get; }
        public Brush TonicMarkIvory { get; }
        public Brush Hint { get; }
        public Brush HintKeyline { get; }
        public Brush Label { get; }
        public Brush LabelOutOfKey { get; }
        public Brush LabelHeld { get; }
        public Brush BlackLabel { get; }
        public Brush BlackLabelOutOfKey { get; }
        public Brush BlackLabelHeld { get; }
        public Color Shadow { get; }

        public KeyPalette(FrameworkElement owner)
        {
            // Magenta for a missing key is loud on purpose: a mistyped name shows at once.
            Brush Find(string key) => owner.TryFindResource(key) as Brush ?? Brushes.Magenta;

            Shadow = owner.TryFindResource("Color.Shadow") as Color? ?? Colors.Magenta;

            White = Find("Brush.Key.White");
            WhiteHeld = Find("Brush.Key.White.Held");
            WhiteRinging = Find("Brush.Key.White.Ringing");
            WhiteInKey = Find("Brush.Key.White.InKey");
            WhiteTonic = Find("Brush.Key.White.Tonic");
            WhiteOutOfKey = Find("Brush.Key.White.OutOfKey");
            WhiteBorder = Find("Brush.Key.White.Border");
            WhiteBullnose = Find("Brush.Key.White.Bullnose");
            WhiteBullnoseOutOfKey = Find("Brush.Key.White.Bullnose.OutOfKey");
            WhiteBullnoseHeld = Find("Brush.Key.White.Bullnose.Held");
            WhiteUndercut = Find("Brush.Key.White.Undercut");
            WhiteSkirt = Find("Brush.Key.White.Skirt");
            WhiteSkirtOutOfKey = Find("Brush.Key.White.Skirt.OutOfKey");
            WhiteSkirtHeld = Find("Brush.Key.White.Skirt.Held");
            WhiteRidge = Find("Brush.Key.White.Ridge");
            WhiteChamfer = Find("Brush.Key.White.Chamfer");
            PressedRecess = Find("Brush.Key.PressedRecess");
            KeyLedHousing = Find("Brush.Key.Led.Housing");
            KeyLedOff = Find("Brush.Key.Led.Off");
            KeyLedHeld = Find("Brush.Key.Led.Held");
            KeyLedRinging = Find("Brush.Key.Led.Ringing");
            KeyLedGlow = Find("Brush.Key.Led.Glow");
            FallboardShadow = Find("Brush.Fallboard.Shadow");
            BlackContactShadow = Find("Brush.Key.Black.ContactShadow");
            Black = Find("Brush.Key.Black");
            BlackInKey = Find("Brush.Key.Black.InKey");
            BlackOutOfKey = Find("Brush.Key.Black.OutOfKey");
            BlackLeftWall = Find("Brush.Key.Black.LeftWall");
            BlackLeftWallOutOfKey = Find("Brush.Key.Black.LeftWall.OutOfKey");
            BlackLeftWallHeld = Find("Brush.Key.Black.LeftWall.Held");
            BlackRightWall = Find("Brush.Key.Black.RightWall");
            BlackRightWallOutOfKey = Find("Brush.Key.Black.RightWall.OutOfKey");
            BlackRightWallHeld = Find("Brush.Key.Black.RightWall.Held");
            BlackTop = Find("Brush.Key.Black.Top");
            BlackTopInKey = Find("Brush.Key.Black.Top.InKey");
            BlackTopTonic = Find("Brush.Key.Black.Top.Tonic");
            BlackTopOutOfKey = Find("Brush.Key.Black.Top.OutOfKey");
            BlackTopHeld = Find("Brush.Key.Black.Top.Held");
            BlackTopRinging = Find("Brush.Key.Black.Top.Ringing");
            BlackNose = Find("Brush.Key.Black.Nose");
            BlackNoseInKey = Find("Brush.Key.Black.Nose.InKey");
            BlackNoseOutOfKey = Find("Brush.Key.Black.Nose.OutOfKey");
            BlackNoseHeld = Find("Brush.Key.Black.Nose.Held");
            BlackNoseCrest = Find("Brush.Key.Black.NoseCrest");
            BlackNoseCrestOutOfKey = Find("Brush.Key.Black.NoseCrest.OutOfKey");
            BlackRidge = Find("Brush.Key.Black.Ridge");
            BlackRidgeOutOfKey = Find("Brush.Key.Black.Ridge.OutOfKey");
            BlackHeld = Find("Brush.Key.Black.Held");
            BlackRinging = Find("Brush.Key.Black.Ringing");
            BlackTonic = Find("Brush.Key.Black.Tonic");
            BlackBorder = Find("Brush.Key.Black.Border");
            InKeyMark = Find("Brush.Key.InKey.Mark");
            TonicMark = Find("Brush.Key.Tonic.Mark");
            InKeyMarkIvory = Find("Brush.Key.InKey.Mark.Ivory");
            TonicMarkIvory = Find("Brush.Key.Tonic.Mark.Ivory");
            Hint = Find("Brush.Key.Hint");
            HintKeyline = Find("Brush.Key.Hint.Keyline");
            Label = Find("Brush.Key.Label");
            LabelOutOfKey = Find("Brush.Key.Label.OutOfKey");
            LabelHeld = Find("Brush.Key.Label.Held");
            BlackLabel = Find("Brush.Key.Black.Label");
            BlackLabelOutOfKey = Find("Brush.Key.Black.Label.OutOfKey");
            BlackLabelHeld = Find("Brush.Key.Black.Label.Held");
        }
    }
}
