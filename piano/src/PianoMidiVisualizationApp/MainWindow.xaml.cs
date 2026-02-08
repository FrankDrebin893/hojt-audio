using System.Collections.Specialized;
using System.Windows;
using System.Windows.Input;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Loaded += OnLoaded;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        // Auto-scroll the MIDI log when new items are added
        if (MidiLogList.ItemsSource is INotifyCollectionChanged collection)
        {
            collection.CollectionChanged += (_, _) =>
            {
                if (MidiLogList.Items.Count > 0)
                    MidiLogList.ScrollIntoView(MidiLogList.Items[^1]);
            };
        }
    }

    private void Window_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Space && DataContext is MainViewModel vm)
        {
            vm.SaveCurrentChordCommand.Execute(null);
            e.Handled = true;
        }
    }
}
