using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class SongPracticePanel : UserControl
{
    public static readonly DependencyProperty KeyboardControlProperty = DependencyProperty.Register(
        nameof(KeyboardControl), typeof(PianoKeyboardControl), typeof(SongPracticePanel));

    /// <summary>The keyboard the notes fall onto; the falling-notes view aligns its columns to it.</summary>
    public PianoKeyboardControl? KeyboardControl
    {
        get => (PianoKeyboardControl?)GetValue(KeyboardControlProperty);
        set => SetValue(KeyboardControlProperty, value);
    }

    /// <summary>The falling-notes view, for tests that check it against the keyboard.</summary>
    public FallingNotesControl FallingNotes => Notes;

    public SongPracticePanel()
    {
        InitializeComponent();

        // A popup is a window of its own: it would stay up over whatever replaces the song view.
        IsVisibleChanged += (_, e) =>
        {
            if (e.NewValue is false) LoopRangePopup.IsOpen = false;
        };
    }

    private SongPracticeViewModel? Session => DataContext as SongPracticeViewModel;

    // ------------------------------------------------------------------ narrow stage

    /// <summary>
    /// How much of the song bar is left out to fit a narrow stage, least needed first. Each step
    /// keeps what the ones before it dropped. Scaling the whole bar down instead made its text
    /// unreadable: at the 900x420 minimum with the sidebar open it came out at 65%, 7px labels.
    /// </summary>
    public enum BarDensity
    {
        Full,
        TightSeparators,
        NoSongDetails,     // BPM and bar count, which the title's tooltip still shows
        IconButtons,       // Open, Loop and Tracks as glyphs, which have tooltips
        NoLabels,          // "Speed" and "Bars"
        LoopRangeInPopup,  // the two bar steppers behind a "1–4" dropdown
        NoBarLabel,
    }

    private const BarDensity Sparsest = BarDensity.NoBarLabel;

    /// <summary>
    /// Width the chord readout beside the bar is guaranteed, margins included: room for most
    /// triads and sevenths ("Dm7  ii7") at full size. Reserved rather than measured, so the bar
    /// keeps its layout as chords come and go; a longer name shrinks into whatever the bar leaves.
    /// </summary>
    private const double ChordReserve = 90;

    private static readonly Thickness SeparatorMargin = new(10, 3, 10, 3);
    private static readonly Thickness TightSeparatorMargin = new(5, 3, 5, 3);

    private static readonly Size Unbounded = new(double.PositiveInfinity, double.PositiveInfinity);

    private bool _fittingBar;

    /// <summary>The density the bar was last fitted at, for tests.</summary>
    public BarDensity Density { get; private set; }

    private void BarGrid_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (e.WidthChanged) FitBar();
    }

    // The bar's own width changes with its content: a song loaded or closed, a new title.
    private void BarContent_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (e.WidthChanged) FitBar();
    }

    /// <summary>
    /// Picks the fullest density whose natural width fits beside the chord readout. Measured
    /// rather than set by window width, since the title's length varies. The Viewbox still
    /// scales down whatever overflows the sparsest one, so nothing is ever cut off.
    /// </summary>
    private void FitBar()
    {
        if (_fittingBar) return;
        double available = BarGrid.ActualWidth - ChordReserve;
        if (available <= 0) return;

        _fittingBar = true;
        try
        {
            var density = BarDensity.Full;
            while (true)
            {
                ApplyDensity(density);
                BarContent.Measure(Unbounded);
                if (BarContent.DesiredSize.Width <= available || density == Sparsest) break;
                density++;
            }
            Density = density;
            BarViewbox.MaxWidth = available;

            if (density < BarDensity.LoopRangeInPopup) LoopRangePopup.IsOpen = false;
        }
        finally
        {
            _fittingBar = false;
        }
    }

    private void ApplyDensity(BarDensity density)
    {
        var separatorMargin = density >= BarDensity.TightSeparators ? TightSeparatorMargin : SeparatorMargin;
        foreach (var separator in SongControls.Children.OfType<Rectangle>())
            Change(separator, MarginProperty, separatorMargin);

        Show(SongDetailsText, density < BarDensity.NoSongDetails);
        Change(TitleBlock, MaxWidthProperty,
               density >= BarDensity.NoBarLabel ? 84.0 : density >= BarDensity.NoSongDetails ? 160.0 : 220.0);

        bool labelled = density < BarDensity.IconButtons;
        Show(OpenLabel, labelled);
        Show(LoopLabel, labelled);
        Show(TracksLabel, labelled);

        Show(SpeedLabel, density < BarDensity.NoLabels);
        Show(BarsLabel, density < BarDensity.NoLabels);

        Show(InlineLoopRange, density < BarDensity.LoopRangeInPopup);
        Show(LoopRangeButton, density >= BarDensity.LoopRangeInPopup);

        Show(BarLabel, density < BarDensity.NoBarLabel);
    }

    private void Show(UIElement part, bool visible) =>
        Change(part, VisibilityProperty, visible ? Visibility.Visible : Visibility.Collapsed);

    /// <summary>
    /// Sets a property on part of the bar, and marks every element from it up to the bar as
    /// needing a measure. A change only marks its own parent, and layout walks up from there as
    /// it goes; FitBar's direct Measure of the bar would otherwise get its last size back.
    /// </summary>
    private void Change(UIElement part, DependencyProperty property, object value)
    {
        part.SetValue(property, value);
        for (DependencyObject? element = part; element != null && element != BarViewbox;
             element = VisualTreeHelper.GetParent(element))
        {
            (element as UIElement)?.InvalidateMeasure();
        }
    }

    // Scrolling over a value nudges it, like the metronome's tempo box.

    private void Speed_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (Session is { } session)
            session.Speed += e.Delta > 0 ? SongPracticeViewModel.SpeedStep : -SongPracticeViewModel.SpeedStep;
        e.Handled = true;
    }

    private void LoopStart_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (Session is { } session)
            session.LoopStartBar += e.Delta > 0 ? 1 : -1;
        e.Handled = true;
    }

    private void LoopEnd_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (Session is { } session)
            session.LoopEndBar += e.Delta > 0 ? 1 : -1;
        e.Handled = true;
    }
}
