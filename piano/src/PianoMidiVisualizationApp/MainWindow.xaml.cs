using System.Collections.Specialized;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media.Animation;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp;

public partial class MainWindow : Window
{
    /// <summary>Full brightness on the click, gone well before the next one (240 BPM is 250 ms).</summary>
    private static readonly DoubleAnimation BeatFlashFade = CreateBeatFlashFade();

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

        if (DataContext is MainViewModel vm)
            vm.MetronomeBeat += OnMetronomeBeat;
    }

    private static DoubleAnimation CreateBeatFlashFade()
    {
        var fade = new DoubleAnimation(1, 0, TimeSpan.FromMilliseconds(180));
        fade.Freeze();
        return fade;
    }

    /// <summary>
    /// Flashes the beat light. Raised per click from the audio thread's beat counter, so the
    /// light follows the audio clock. Code rather than a XAML trigger because a binding-driven
    /// EventTrigger also fires once when the binding first attaches, flashing at startup.
    /// </summary>
    private void OnMetronomeBeat(object? sender, EventArgs e) =>
        BeatFlash.BeginAnimation(OpacityProperty, BeatFlashFade);

    // ----- Metronome BPM field -----

    private void MetronomeBpmBox_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        switch (e.Key)
        {
            case Key.Enter:
                CommitBpm();
                // Hand the keyboard back, so Space and Ctrl+M work again without a click.
                Keyboard.Focus(this);
                break;
            case Key.Escape:
                MetronomeBpmBox.GetBindingExpression(TextBox.TextProperty)?.UpdateTarget();
                Keyboard.Focus(this);
                break;
            case Key.Up:
                NudgeBpm(+1);
                break;
            case Key.Down:
                NudgeBpm(-1);
                break;
            default:
                return;
        }

        e.Handled = true;
    }

    private void MetronomeBpmBox_LostKeyboardFocus(object sender, KeyboardFocusChangedEventArgs e) =>
        CommitBpm();

    private void MetronomeBpmBox_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        NudgeBpm(e.Delta > 0 ? +1 : -1);
        e.Handled = true;
    }

    /// <summary>Writes the typed tempo, then reads it back: shows the clamped value, or restores the old one if the text was not a number.</summary>
    private void CommitBpm()
    {
        var binding = MetronomeBpmBox.GetBindingExpression(TextBox.TextProperty);
        binding?.UpdateSource();
        binding?.UpdateTarget();
    }

    /// <summary>Nudges from whatever is typed, so a half-entered tempo is not lost.</summary>
    private void NudgeBpm(int delta)
    {
        if (DataContext is not MainViewModel vm) return;
        CommitBpm();
        vm.Settings.MetronomeBpm += delta;
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
        bool ctrlShift = Keyboard.Modifiers == (ModifierKeys.Control | ModifierKeys.Shift);

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
            case Key.R when ctrl && !IsTextEntryFocused():
                vm.Recorder.ToggleRecordCommand.Execute(null);
                break;
            case Key.R when ctrlShift && !IsTextEntryFocused():
                vm.Recorder.TogglePlaybackCommand.Execute(null);
                break;
            case Key.F5 when shift && !IsTextEntryFocused():
                vm.ToggleRecorderCommand.Execute(null);
                break;
            case Key.M when ctrl && !IsTextEntryFocused():
                vm.ToggleMetronomeCommand.Execute(null);
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
