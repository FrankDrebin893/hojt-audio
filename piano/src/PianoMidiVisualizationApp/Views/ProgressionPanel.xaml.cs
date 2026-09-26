using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class ProgressionPanel : UserControl
{
    public ProgressionPanel()
    {
        InitializeComponent();

        // Hiding the sidebar (Shift+F4, zen) under the pointer may never raise MouseLeave, and
        // a hint left behind would sit on the keyboard with nothing to explain it.
        IsVisibleChanged += (_, e) =>
        {
            if (e.NewValue is false) Tools?.EndSuggestionPreview();
        };
    }

    private ProgressionToolsViewModel? Tools => (DataContext as MainViewModel)?.ProgressionTools;

    private void Suggestion_MouseEnter(object sender, MouseEventArgs e)
    {
        if (sender is FrameworkElement { DataContext: ChordSuggestionItem item })
            Tools?.PreviewSuggestion(item);
    }

    private void Suggestion_MouseLeave(object sender, MouseEventArgs e) =>
        Tools?.EndSuggestionPreview();

    /// <summary>Keeps the chord being played in view when eight of them overflow the sidebar.</summary>
    private void ChordCard_TargetUpdated(object? sender, DataTransferEventArgs e)
    {
        if (sender is FrameworkElement { Tag: true } card)
            card.BringIntoView();
    }
}
