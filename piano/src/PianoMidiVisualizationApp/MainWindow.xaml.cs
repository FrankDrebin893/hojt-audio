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
        if (DataContext is not MainViewModel vm) return;

        if (e.Key == Key.Space)
        {
            vm.SaveCurrentChordCommand.Execute(null);
            e.Handled = true;
        }
        else if (e.Key == Key.F2 && Keyboard.Modifiers == ModifierKeys.Shift)
        {
            vm.ToggleChatPanelCommand.Execute(null);
            e.Handled = true;
        }
        else if (e.Key == Key.F3 && Keyboard.Modifiers == ModifierKeys.Shift)
        {
            vm.ToggleMidiLogCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void TitleBar_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ClickCount == 2)
        {
            MaximizeButton_Click(sender, e);
        }
        else
        {
            DragMove();
        }
    }

    private void MinimizeButton_Click(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState.Minimized;
    }

    private void MaximizeButton_Click(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Close();
    }
}
