using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
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

    private static readonly SolidColorBrush WhiteDefault = new(Colors.White);
    private static readonly SolidColorBrush WhitePressed = new(Color.FromRgb(100, 180, 255));
    private static readonly SolidColorBrush BlackDefault = new(Color.FromRgb(30, 30, 30));
    private static readonly SolidColorBrush BlackPressed = new(Color.FromRgb(30, 120, 255));
    private static readonly SolidColorBrush KeyBorder = new(Color.FromRgb(80, 80, 80));

    private readonly Dictionary<int, Rectangle> _keyRectangles = new();

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

        // First pass: draw white keys
        int whiteKeyIndex = 0;
        foreach (var key in vm.Keys)
        {
            if (!key.IsBlack)
            {
                var rect = CreateWhiteKey(whiteKeyIndex, key);
                PianoCanvas.Children.Add(rect);
                _keyRectangles[key.NoteNumber] = rect;
                whiteKeyIndex++;
            }
        }

        // Second pass: draw black keys on top
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
            Fill = WhiteDefault,
            Stroke = KeyBorder,
            StrokeThickness = 0.5,
            RadiusX = 0,
            RadiusY = 3,
            Tag = key.NoteNumber
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
            Fill = BlackDefault,
            Stroke = KeyBorder,
            StrokeThickness = 0.5,
            RadiusX = 0,
            RadiusY = 3,
            Tag = key.NoteNumber
        };
        Canvas.SetLeft(rect, x);
        Canvas.SetTop(rect, 0);
        Panel.SetZIndex(rect, 1);
        return rect;
    }

    private double GetBlackKeyX(int noteNumber, PianoKeyboardViewModel vm)
    {
        // Find the white key index of the white key just before this black key
        int whiteKeyIndex = 0;
        foreach (var k in vm.Keys)
        {
            if (k.NoteNumber >= noteNumber) break;
            if (!k.IsBlack) whiteKeyIndex++;
        }

        // Black key offsets within an octave (relative to the left white key)
        // C#: between C and D -> offset ~0.6 of white key width
        // D#: between D and E -> offset ~0.6
        // F#: between F and G -> offset ~0.6
        // G#: between G and A -> offset ~0.6
        // A#: between A and B -> offset ~0.6
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

        return (whiteKeyIndex * WhiteKeyWidth) + (WhiteKeyWidth * offset) - (BlackKeyWidth / 2.0);
    }

    private void OnKeyPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PianoKey.IsPressed) && sender is PianoKey key)
        {
            if (_keyRectangles.TryGetValue(key.NoteNumber, out var rect))
            {
                if (key.IsBlack)
                    rect.Fill = key.IsPressed ? BlackPressed : BlackDefault;
                else
                    rect.Fill = key.IsPressed ? WhitePressed : WhiteDefault;
            }
        }
    }
}
