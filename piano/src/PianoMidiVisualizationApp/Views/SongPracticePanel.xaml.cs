using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
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
    }

    private SongPracticeViewModel? Session => DataContext as SongPracticeViewModel;

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
