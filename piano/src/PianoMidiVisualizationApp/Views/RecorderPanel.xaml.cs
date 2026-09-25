using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class RecorderPanel : UserControl
{
    public RecorderPanel()
    {
        InitializeComponent();
    }

    /// <summary>
    /// Selects the clicked take. A ListBoxItem only selects itself on click if it can take focus,
    /// and these deliberately cannot. Not handled, so the row's export and delete buttons still
    /// get the click.
    /// </summary>
    private void TakeList_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        for (var node = e.OriginalSource as DependencyObject; node != null && node != TakeList; node = ParentOf(node))
        {
            if (node is ListBoxItem item)
            {
                item.IsSelected = true;
                return;
            }
        }
    }

    // A click on the take's name lands on a Run, which has a logical parent but no visual one.
    private static DependencyObject? ParentOf(DependencyObject node) =>
        node is Visual ? VisualTreeHelper.GetParent(node) : LogicalTreeHelper.GetParent(node);

    private void Timeline_RegionSelected(object? sender, (TimeSpan Start, TimeSpan End) region)
    {
        if (DataContext is RecorderViewModel vm)
            vm.SetLoopRegion(region.Start, region.End);
    }

    private void Timeline_SeekRequested(object? sender, TimeSpan position)
    {
        if (DataContext is RecorderViewModel vm)
            vm.Seek(position);
    }
}
