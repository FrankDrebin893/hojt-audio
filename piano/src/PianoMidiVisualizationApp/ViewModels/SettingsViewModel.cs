using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.ViewModels;

public partial class SettingsViewModel : ObservableObject
{
    [ObservableProperty]
    private DeviceInfo? _selectedMidiDevice;

    [ObservableProperty]
    private string? _selectedAudioDriver;

    [ObservableProperty]
    private bool _useAsio = true;

    [ObservableProperty]
    private string _soundFontPath = "";

    [ObservableProperty]
    private float _volume = 0.8f;

    public ObservableCollection<DeviceInfo> MidiDevices { get; } = new();
    public ObservableCollection<string> AsioDriverNames { get; } = new();
    public ObservableCollection<string> WasapiDeviceNames { get; } = new();

    public void ApplyFrom(AppSettings settings)
    {
        UseAsio = settings.UseAsio;
        SoundFontPath = settings.SoundFontPath ?? "";
        Volume = settings.Volume;

        // Device selection will be applied after enumeration
        if (settings.LastMidiDevice != null)
        {
            var match = MidiDevices.FirstOrDefault(d => d.Name == settings.LastMidiDevice);
            if (match != null) SelectedMidiDevice = match;
        }

        if (settings.LastAudioDriver != null)
            SelectedAudioDriver = settings.LastAudioDriver;
    }

    public AppSettings ToAppSettings()
    {
        return new AppSettings
        {
            LastMidiDevice = SelectedMidiDevice?.Name,
            LastAudioDriver = SelectedAudioDriver,
            UseAsio = UseAsio,
            SoundFontPath = SoundFontPath,
            Volume = Volume
        };
    }
}
