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

        // Load saved settings
        var saved = AppSettings.Load();
        _mainViewModel.Settings.ApplyFrom(saved);

        var window = new MainWindow { DataContext = _mainViewModel };
        window.Closing += (_, _) =>
        {
            _mainViewModel.Settings.ToAppSettings().Save();
            _mainViewModel.Dispose();
        };

        MainWindow = window;
        window.Show();
    }
}
