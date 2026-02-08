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
    private const double WhiteKeyWidth = 26;
    private const double WhiteKeyHeight = 155;
    private const double BlackKeyWidth = 16;
    private const double BlackKeyHeight = 100;

    // White key gradients - ivory to light gray for 3D effect
    private static readonly LinearGradientBrush WhiteKeyGradient = new(
        Color.FromRgb(255, 255, 253), // Warm white at top
        Color.FromRgb(235, 235, 230), // Slightly darker at bottom
        new Point(0, 0), new Point(0, 1));

    private static readonly LinearGradientBrush WhiteKeyPressedGradient = new(
        Color.FromRgb(140, 200, 255), // Lighter blue at top
        Color.FromRgb(80, 160, 235),  // Darker blue at bottom
        new Point(0, 0), new Point(0, 1));

    // Black key gradients - creates beveled top effect
    private static readonly LinearGradientBrush BlackKeyGradient;
    private static readonly LinearGradientBrush BlackKeyPressedGradient;

    private static readonly SolidColorBrush KeyBorder = new(Color.FromRgb(60, 60, 60));
    private static readonly SolidColorBrush WhiteKeyLabel = new(Color.FromRgb(170, 170, 165));
    private static readonly SolidColorBrush BlackKeyLabel = new(Color.FromRgb(100, 100, 100));

    private readonly Dictionary<int, Rectangle> _keyRectangles = new();

    static PianoKeyboardControl()
    {
        // Black key gradient with highlight at top for 3D bevel
        BlackKeyGradient = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = new Point(0, 1),
            GradientStops = new GradientStopCollection
            {
                new(Color.FromRgb(70, 70, 70), 0.0),    // Lighter top edge
                new(Color.FromRgb(35, 35, 35), 0.08),   // Quick transition
                new(Color.FromRgb(25, 25, 25), 0.5),    // Dark middle
                new(Color.FromRgb(15, 15, 15), 1.0)     // Darker bottom
            }
        };
        BlackKeyGradient.Freeze();

        BlackKeyPressedGradient = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = new Point(0, 1),
            GradientStops = new GradientStopCollection
            {
                new(Color.FromRgb(80, 150, 220), 0.0),
                new(Color.FromRgb(40, 120, 200), 0.08),
                new(Color.FromRgb(25, 100, 180), 0.5),
                new(Color.FromRgb(20, 80, 160), 1.0)
            }
        };
        BlackKeyPressedGradient.Freeze();

        WhiteKeyGradient.Freeze();
        WhiteKeyPressedGradient.Freeze();
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

        // First pass: draw white keys and their labels
        int whiteKeyIndex = 0;
        foreach (var key in vm.Keys)
        {
            if (!key.IsBlack)
            {
                var rect = CreateWhiteKey(whiteKeyIndex, key);
                PianoCanvas.Children.Add(rect);
                _keyRectangles[key.NoteNumber] = rect;

                var label = CreateWhiteKeyLabel(whiteKeyIndex, key);
                PianoCanvas.Children.Add(label);

                whiteKeyIndex++;
            }
        }

        // Second pass: draw black keys and their labels on top
        whiteKeyIndex = 0;
        for (int i = 0; i < vm.Keys.Count; i++)
        {
            var key = vm.Keys[i];
            if (key.IsBlack)
            {
                double x = GetBlackKeyX(key.NoteNumber, vm);
                var rect = CreateBlackKey(x, key);
                PianoCanvas.Children.Add(rect);
                _keyRectangles[key.NoteNumber] = rect;

                var label = CreateBlackKeyLabel(x, key);
                PianoCanvas.Children.Add(label);
            }
            else
            {
                whiteKeyIndex++;
            }
        }

        // Set canvas width
        int totalWhiteKeys = vm.Keys.Count(k => !k.IsBlack);
        PianoCanvas.Width = totalWhiteKeys * WhiteKeyWidth;

        // Subscribe to property changes
        foreach (var key in vm.Keys)
            key.PropertyChanged += OnKeyPropertyChanged;
    }

    private Rectangle CreateWhiteKey(int index, PianoKey key)
    {
        var rect = new Rectangle
        {
            Width = WhiteKeyWidth - 1,
            Height = WhiteKeyHeight,
            Fill = WhiteKeyGradient,
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
        Canvas.SetLeft(rect, index * WhiteKeyWidth);
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
            Fill = BlackKeyGradient,
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

    private TextBlock CreateWhiteKeyLabel(int index, PianoKey key)
    {
        // Show full name (with octave) for C notes, just letter for others
        bool isC = key.NoteNumber % 12 == 0;
        string labelText = isC ? key.NoteName : key.NoteName.TrimEnd('0', '1', '2', '3', '4', '5', '6', '7', '8', '9');

        var label = new TextBlock
        {
            Text = labelText,
            FontSize = isC ? 9 : 8,
            FontWeight = isC ? FontWeights.SemiBold : FontWeights.Normal,
            Foreground = WhiteKeyLabel,
            TextAlignment = TextAlignment.Center,
            Width = WhiteKeyWidth - 1
        };
        Canvas.SetLeft(label, index * WhiteKeyWidth);
        Canvas.SetTop(label, WhiteKeyHeight - (isC ? 16 : 14));
        Panel.SetZIndex(label, 0);
        return label;
    }

    private TextBlock CreateBlackKeyLabel(double x, PianoKey key)
    {
        string labelText = key.NoteName.TrimEnd('0', '1', '2', '3', '4', '5', '6', '7', '8', '9');

        var label = new TextBlock
        {
            Text = labelText,
            FontSize = 7,
            Foreground = BlackKeyLabel,
            TextAlignment = TextAlignment.Center,
            Width = BlackKeyWidth
        };
        Canvas.SetLeft(label, x);
        Canvas.SetTop(label, BlackKeyHeight - 12);
        Panel.SetZIndex(label, 2);
        return label;
    }

    private double GetBlackKeyX(int noteNumber, PianoKeyboardViewModel vm)
    {
        // Count white keys before this black key
        int whiteKeysBefore = 0;
        foreach (var k in vm.Keys)
        {
            if (k.NoteNumber >= noteNumber) break;
            if (!k.IsBlack) whiteKeysBefore++;
        }

        // The black key sits after the white key at index (whiteKeysBefore - 1)
        int leftWhiteKeyIndex = whiteKeysBefore - 1;

        // Black key offsets within an octave (relative to the left white key)
        int noteInOctave = noteNumber % 12;
        double offset = noteInOctave switch
        {
            1 => 0.6,   // C#
            3 => 0.7,   // D#
            6 => 0.6,   // F#
            8 => 0.65,  // G#
            10 => 0.7,  // A#
            _ => 0.6
        };

        return (leftWhiteKeyIndex * WhiteKeyWidth) + (WhiteKeyWidth * offset) - (BlackKeyWidth / 2.0);
    }

    private void OnKeyPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PianoKey.IsPressed) && sender is PianoKey key)
        {
            if (_keyRectangles.TryGetValue(key.NoteNumber, out var rect))
            {
                if (key.IsBlack)
                    rect.Fill = key.IsPressed ? BlackKeyPressedGradient : BlackKeyGradient;
                else
                    rect.Fill = key.IsPressed ? WhiteKeyPressedGradient : WhiteKeyGradient;
            }
        }
    }
}
