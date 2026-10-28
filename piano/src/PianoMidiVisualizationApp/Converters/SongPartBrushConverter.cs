using System.Globalization;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;
using PianoMidiVisualizationApp.Views;

namespace PianoMidiVisualizationApp.Converters;

/// <summary>
/// A song part's colour index to its theme brush (Brush.Song.Part0..N-1), so the track list's
/// swatches match the falling notes, which resolve the same keys.
/// </summary>
public class SongPartBrushConverter : IValueConverter
{
    public object? Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is not int index) return null;
        int n = FallingNotesControl.PartColorCount;
        return Application.Current?.TryFindResource($"Brush.Song.Part{((index % n) + n) % n}") as Brush;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
