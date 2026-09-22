using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>
/// One entry in the key-root dropdown. The name depends on the selected quality (Ab Major
/// but G# Minor), so it is observable and re-spelled in place: swapping the collection's
/// contents instead would clear the ComboBox's selection along with the old items.
/// </summary>
public partial class KeyRootOption : ObservableObject
{
    public int PitchClass { get; init; }

    [ObservableProperty]
    private string _name = "";
}

/// <summary>One entry in the key-quality dropdown.</summary>
public record KeyQualityOption(KeyQuality Quality, string Name);

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

    [ObservableProperty]
    private string _anthropicApiKey = "";

    // ----- Key signature -----

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    private bool _isKeyHighlightEnabled;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    private int _keyTonicPitchClass;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    private KeyQuality _keyQuality = KeyQuality.Major;

    public ObservableCollection<DeviceInfo> MidiDevices { get; } = new();
    public ObservableCollection<string> AsioDriverNames { get; } = new();
    public ObservableCollection<string> WasapiDeviceNames { get; } = new();

/// <summary>The twelve roots, spelled for the selected quality — Eb Minor, not D# Minor.</summary>
    public ObservableCollection<KeyRootOption> KeyRoots { get; } = new();

    public ObservableCollection<KeyQualityOption> KeyQualities { get; } = new()
    {
        new(KeyQuality.Major, "Major"),
        new(KeyQuality.Minor, "Minor")
    };

    /// <summary>The selected key signature, or null when highlighting is off.</summary>
    public MusicKey? CurrentKey => IsKeyHighlightEnabled
        ? new MusicKey(KeyTonicPitchClass, KeyQuality)
        : null;

    public SettingsViewModel()
    {
        for (int pc = 0; pc < 12; pc++)
            KeyRoots.Add(new KeyRootOption { PitchClass = pc, Name = MusicKey.RootName(pc, KeyQuality) });
    }

    /// <summary>Renames the existing roots; deliberately does not replace them.</summary>
    private void RespellKeyRoots()
    {
        foreach (var root in KeyRoots)
            root.Name = MusicKey.RootName(root.PitchClass, KeyQuality);
    }

    // Touching either dropdown is itself the act of choosing a key, so it turns highlighting
    // on. The explicit clear button is the only way back to "no key".
    partial void OnKeyQualityChanged(KeyQuality value)
    {
        RespellKeyRoots();
        IsKeyHighlightEnabled = true;
    }

    partial void OnKeyTonicPitchClassChanged(int value) => IsKeyHighlightEnabled = true;

    public void ApplyFrom(AppSettings settings)
    {
        UseAsio = settings.UseAsio;
        SoundFontPath = settings.SoundFontPath ?? "";
        Volume = settings.Volume;
        AnthropicApiKey = settings.AnthropicApiKey ?? "";

        KeyQuality = Enum.TryParse<KeyQuality>(settings.KeyQuality, out var quality)
            ? quality
            : KeyQuality.Major;
        KeyTonicPitchClass = ((settings.KeyTonicPitchClass % 12) + 12) % 12;
        // Last, because setting either of the two above flips it on.
        IsKeyHighlightEnabled = settings.KeyHighlightEnabled;

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
            Volume = Volume,
            AnthropicApiKey = AnthropicApiKey,
            KeyHighlightEnabled = IsKeyHighlightEnabled,
            KeyTonicPitchClass = KeyTonicPitchClass,
            KeyQuality = KeyQuality.ToString()
        };
    }
}
