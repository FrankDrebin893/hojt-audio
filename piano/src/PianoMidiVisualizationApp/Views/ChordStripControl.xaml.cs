using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// The diatonic chord strip. Hover is routed here rather than bound, since a Button has no
/// command for the pointer entering and leaving it.
/// </summary>
public partial class ChordStripControl : UserControl
{
    public ChordStripControl()
    {
        InitializeComponent();
    }

    private ChordStripViewModel? ViewModel => DataContext as ChordStripViewModel;

    private void Tile_MouseEnter(object sender, MouseEventArgs e)
    {
        if (sender is FrameworkElement { DataContext: ChordTileViewModel tile })
            ViewModel?.BeginHover(tile);
    }

    private void Tile_MouseLeave(object sender, MouseEventArgs e) => ViewModel?.EndHover();

    /// <summary>
    /// The strip can vanish from under the pointer (Shift+F7, clearing the key, zen mode); its
    /// hints must never outlive it on the keyboard.
    /// </summary>
    private void OnIsVisibleChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (!IsVisible)
            ViewModel?.EndHover();
    }
}
