using System.Collections.Specialized;
using System.Windows;

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
}
