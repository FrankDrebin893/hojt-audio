using System.Collections.Specialized;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
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
            collection.CollectionChanged += (_, _) => ScrollLogToEnd();
        }

        // The log is hidden by default, so it can be stale by the time it is shown.
        MidiLogList.IsVisibleChanged += (_, _) => ScrollLogToEnd();
    }

    private void ScrollLogToEnd()
    {
        // Skip the work entirely while the log is collapsed — this runs per MIDI message.
        if (!MidiLogList.IsVisible || MidiLogList.Items.Count == 0) return;

        MidiLogList.ScrollIntoView(MidiLogList.Items[^1]);
    }

    private void SettingsScrim_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (DataContext is MainViewModel vm)
            vm.CloseSettingsOverlayCommand.Execute(null);
    }

    // KeyDown rather than PreviewKeyDown, so focused controls get first refusal: an open
    // ComboBox consumes Esc to close its dropdown before the overlay ever sees it.
    private void Window_KeyDown(object sender, KeyEventArgs e)
    {
        if (DataContext is not MainViewModel vm) return;

        bool shift = Keyboard.Modifiers == ModifierKeys.Shift;
        bool ctrl = Keyboard.Modifiers == ModifierKeys.Control;

        switch (e.Key)
        {
            case Key.Space when !IsTextEntryFocused():
                vm.SaveCurrentChordCommand.Execute(null);
                break;
            case Key.OemComma when ctrl:
                vm.ToggleSettingsOverlayCommand.Execute(null);
                break;
            // Only handled while the overlay is open, so Esc stays available to everything else.
            case Key.Escape when vm.IsSettingsOverlayVisible:
                vm.CloseSettingsOverlayCommand.Execute(null);
                break;
            case Key.F11:
                vm.ToggleZenModeCommand.Execute(null);
                break;
            case Key.F1 when shift:
                vm.ToggleStatusBarCommand.Execute(null);
                break;
            case Key.F2 when shift:
                vm.ToggleChatPanelCommand.Execute(null);
                break;
            case Key.F3 when shift:
                vm.ToggleMidiLogCommand.Execute(null);
                break;
            case Key.F4 when shift:
                vm.ToggleProgressionCommand.Execute(null);
                break;
            default:
                return;
        }

        e.Handled = true;
    }

    /// <summary>Space is a text character, so it must not trigger shortcuts while typing.</summary>
    private static bool IsTextEntryFocused() =>
        Keyboard.FocusedElement is TextBoxBase or PasswordBox;

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
