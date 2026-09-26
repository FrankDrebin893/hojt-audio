using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Audio;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>
/// One entry in the key-root dropdown. The name depends on the selected scale (Ab Major
/// but G# Minor), so it is observable and re-spelled in place: swapping the collection's
/// contents instead would clear the ComboBox's selection along with the old items.
/// </summary>
public partial class KeyRootOption : ObservableObject
{
    public int PitchClass { get; init; }

    [ObservableProperty]
    private string _name = "";
}

/// <summary>One entry in the scale dropdown. <see cref="Group"/> is its header in the list.</summary>
public record ScaleOption(ScaleType Scale, string Name, string Group);

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

    /// <summary>
    /// The selected key centre, or null for no key at all. One nullable field rather than a
    /// root plus a separate "enabled" flag: with two, re-picking the root the dropdown was
    /// already showing raised no change, so the highlight could never be switched on that way.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    [NotifyPropertyChangedFor(nameof(IsKeyHighlightEnabled))]
    private int? _keyTonicPitchClass;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    private ScaleType _keyScale = ScaleType.Major;

    public bool IsKeyHighlightEnabled => KeyTonicPitchClass.HasValue;

    /// <summary>
    /// Silence notes outside the selected key. Inert while no key is selected, which is why
    /// the checkbox is disabled rather than hidden then.
    /// </summary>
    [ObservableProperty]
    private bool _muteOutOfKeyNotes;

    // ----- Metronome -----
    // Clamped in the setters rather than validated, so every writer (the BPM field, the -/+
    // buttons, tap tempo, a hand-edited settings file) lands in range with one notification.

    private int _metronomeBpm = 90;
    private int _metronomeBeatsPerBar = 4;
    private float _metronomeVolume = 0.7f;

    /// <summary>
    /// The practice tempo, 30-240. Other features read it as their tempo too, so the name and
    /// type are part of a contract: keep them.
    /// </summary>
    public int MetronomeBpm
    {
        get => _metronomeBpm;
        set => SetProperty(ref _metronomeBpm,
            Math.Clamp(value, MetronomeSampleProvider.MinBpm, MetronomeSampleProvider.MaxBpm));
    }

    /// <summary>1-12. 1 means no accent: every click is the same.</summary>
    public int MetronomeBeatsPerBar
    {
        get => _metronomeBeatsPerBar;
        set => SetProperty(ref _metronomeBeatsPerBar,
            Math.Clamp(value, MetronomeSampleProvider.MinBeatsPerBar, MetronomeSampleProvider.MaxBeatsPerBar));
    }

    /// <summary>The click's own level, 0-1. The master volume applies on top of it.</summary>
    public float MetronomeVolume
    {
        get => _metronomeVolume;
        set => SetProperty(ref _metronomeVolume, Math.Clamp(value, 0f, 1f));
    }

    public IReadOnlyList<int> MetronomeBeatsPerBarOptions { get; } = Enumerable.Range(
        MetronomeSampleProvider.MinBeatsPerBar,
        MetronomeSampleProvider.MaxBeatsPerBar - MetronomeSampleProvider.MinBeatsPerBar + 1).ToList();

    public ObservableCollection<DeviceInfo> MidiDevices { get; } = new();
    public ObservableCollection<string> AsioDriverNames { get; } = new();
    public ObservableCollection<string> WasapiDeviceNames { get; } = new();

    /// <summary>The twelve roots, spelled for the selected scale — Eb Minor, not D# Minor.</summary>
    public ObservableCollection<KeyRootOption> KeyRoots { get; } = new();

    /// <summary>
    /// Every scale in picker order: the major and minor family first, then the modes in the
    /// order they sit on the major scale's degrees, then the five- and six-note scales. Static
    /// because it never changes, which also lets the view group it without a DataContext.
    /// </summary>
    public static IReadOnlyList<ScaleOption> ScaleOptions { get; } =
        new (ScaleType Scale, string Group)[]
        {
            (ScaleType.Major, "Major & minor"),
            (ScaleType.NaturalMinor, "Major & minor"),
            (ScaleType.HarmonicMinor, "Major & minor"),
            (ScaleType.MelodicMinor, "Major & minor"),
            (ScaleType.Dorian, "Modes"),
            (ScaleType.Phrygian, "Modes"),
            (ScaleType.Lydian, "Modes"),
            (ScaleType.Mixolydian, "Modes"),
            (ScaleType.Locrian, "Modes"),
            (ScaleType.MajorPentatonic, "Pentatonic & blues"),
            (ScaleType.MinorPentatonic, "Pentatonic & blues"),
            (ScaleType.Blues, "Pentatonic & blues"),
        }
        .Select(o => new ScaleOption(o.Scale, o.Scale.DisplayName(), o.Group))
        .ToArray();

    /// <summary>The selected key, or null when no key is selected.</summary>
    public MusicKey? CurrentKey =>
        KeyTonicPitchClass is { } pitchClass ? new MusicKey(pitchClass, KeyScale) : null;

    public SettingsViewModel()
    {
        for (int pc = 0; pc < 12; pc++)
            KeyRoots.Add(new KeyRootOption { PitchClass = pc, Name = MusicKey.RootName(pc, KeyScale) });
    }

    /// <summary>Renames the existing roots; deliberately does not replace them.</summary>
    private void RespellKeyRoots()
    {
        foreach (var root in KeyRoots)
            root.Name = MusicKey.RootName(root.PitchClass, KeyScale);
    }

    partial void OnKeyScaleChanged(ScaleType value) => RespellKeyRoots();

    public void ApplyFrom(AppSettings settings)
    {
        UseAsio = settings.UseAsio;
        SoundFontPath = settings.SoundFontPath ?? "";
        Volume = settings.Volume;
        AnthropicApiKey = settings.AnthropicApiKey ?? "";

        KeyScale = settings.ResolveKeyScale();
        KeyTonicPitchClass = settings.KeyHighlightEnabled
            ? ((settings.KeyTonicPitchClass % 12) + 12) % 12
            : null;
        MuteOutOfKeyNotes = settings.MuteOutOfKeyNotes;
        MetronomeBpm = settings.MetronomeBpm;
        MetronomeBeatsPerBar = settings.MetronomeBeatsPerBar;
        MetronomeVolume = settings.MetronomeVolume;

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
            KeyHighlightEnabled = KeyTonicPitchClass.HasValue,
            KeyTonicPitchClass = KeyTonicPitchClass ?? 0,
            KeyScale = KeyScale.ToString(),
            MuteOutOfKeyNotes = MuteOutOfKeyNotes,
            MetronomeBpm = MetronomeBpm,
            MetronomeBeatsPerBar = MetronomeBeatsPerBar,
            MetronomeVolume = MetronomeVolume
        };
    }
}
