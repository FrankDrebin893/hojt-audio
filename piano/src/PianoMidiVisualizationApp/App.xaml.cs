using System.Windows;
using PianoMidiVisualizationApp.Audio;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp;

public partial class App : Application
{
    private MainViewModel? _mainViewModel;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        var midiInput = new MidiInputService();
        var audioEngine = new AudioEngine();

        _mainViewModel = new MainViewModel(midiInput, audioEngine, Current.Dispatcher);

        // Enumerate devices
        _mainViewModel.RefreshDevices();

        // Load saved settings and auto-connect. Must run after RefreshDevices() so the saved
        // MIDI device can be matched by name against the enumerated list.
        var saved = AppSettings.Load();
        _mainViewModel.ApplySettings(saved);

        // Configure chat service with saved API key
        if (!string.IsNullOrEmpty(saved.AnthropicApiKey))
        {
            _mainViewModel.Chat.Configure(saved.AnthropicApiKey);
        }

        _mainViewModel.AutoConnect();

        var window = new MainWindow { DataContext = _mainViewModel };
        RestoreWindowPlacement(window, saved);

        window.Closing += (_, _) =>
        {
            var settings = _mainViewModel.CaptureSettings();

            // RestoreBounds, not Left/Top/Width/Height: closing while maximized or minimized
            // would otherwise persist that geometry as the window's normal size.
            var bounds = window.WindowState == WindowState.Normal
                ? new Rect(window.Left, window.Top, window.Width, window.Height)
                : window.RestoreBounds;

            settings.WindowLeft = bounds.Left;
            settings.WindowTop = bounds.Top;
            settings.WindowWidth = bounds.Width;
            settings.WindowHeight = bounds.Height;
            settings.Save();

            _mainViewModel.Dispose();
        };

        MainWindow = window;
        window.Show();
    }

    private static void RestoreWindowPlacement(Window window, AppSettings saved)
    {
        if (saved.WindowWidth is not > 0 || saved.WindowHeight is not > 0)
            return;

        window.Width = saved.WindowWidth.Value;
        window.Height = saved.WindowHeight.Value;

        // Only restore the position if the window would still land somewhere visible —
        // otherwise a monitor that has since been unplugged would strand it off-screen.
        if (saved.WindowLeft is { } left && saved.WindowTop is { } top
            && IsOnVirtualScreen(new Rect(left, top, window.Width, window.Height)))
        {
            window.WindowStartupLocation = WindowStartupLocation.Manual;
            window.Left = left;
            window.Top = top;
        }
    }

    private static bool IsOnVirtualScreen(Rect bounds)
    {
        var virtualScreen = new Rect(
            SystemParameters.VirtualScreenLeft, SystemParameters.VirtualScreenTop,
            SystemParameters.VirtualScreenWidth, SystemParameters.VirtualScreenHeight);

        var visible = Rect.Intersect(bounds, virtualScreen);

        // Enough of the title bar must be reachable to drag the window back.
        return !visible.IsEmpty && visible.Width >= 120 && visible.Height >= 60;
    }
}
