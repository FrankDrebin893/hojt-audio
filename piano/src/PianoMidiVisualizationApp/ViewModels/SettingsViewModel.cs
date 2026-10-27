using System.Collections.ObjectModel;
using System.IO;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
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
    private bool _isSyncingSoundBank;

    [ObservableProperty]
    private DeviceInfo? _selectedMidiDevice;

    [ObservableProperty]
    private string? _selectedAudioDriver;

    [ObservableProperty]
    private bool _useAsio = true;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SoundFontFileName))]
    private string _soundFontPath = "";

    /// <summary>What Settings shows for the sound: its file name, the folder being noise at that width.</summary>
    public string SoundFontFileName => string.IsNullOrEmpty(SoundFontPath) ? "" : Path.GetFileName(SoundFontPath);

    /// <summary>Managed library of imported .sf2 and .sfz piano soundbanks.</summary>
    public ObservableCollection<SoundBankEntry> SoundBanks { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasSelectedSoundBank))]
    private SoundBankEntry? _selectedSoundBank;

    public bool HasSoundBanks => SoundBanks.Count > 0;

    public bool HasSelectedSoundBank => SelectedSoundBank is not null;

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
    [NotifyPropertyChangedFor(nameof(KeyLabel))]
    private int? _keyTonicPitchClass;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentKey))]
    [NotifyPropertyChangedFor(nameof(KeyLabel))]
    private ScaleType _keyScale = ScaleType.Major;

    public bool IsKeyHighlightEnabled => KeyTonicPitchClass.HasValue;

    /// <summary>The key as the OLED's header line reads it: "C Major", or "No key".</summary>
    public string KeyLabel => CurrentKey?.DisplayName ?? "No key";

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
        SoundBanks.CollectionChanged += (_, _) => OnPropertyChanged(nameof(HasSoundBanks));

        for (int pc = 0; pc < 12; pc++)
            KeyRoots.Add(new KeyRootOption { PitchClass = pc, Name = MusicKey.RootName(pc, KeyScale) });

        ScanLocalSoundbanksDirectory();
    }

    /// <summary>Renames the existing roots; deliberately does not replace them.</summary>
    private void RespellKeyRoots()
    {
        foreach (var root in KeyRoots)
            root.Name = MusicKey.RootName(root.PitchClass, KeyScale);
    }

    partial void OnKeyScaleChanged(ScaleType value) => RespellKeyRoots();

    partial void OnSelectedSoundBankChanged(SoundBankEntry? value)
    {
        if (_isSyncingSoundBank) return;

        _isSyncingSoundBank = true;
        try
        {
            if (value is not null && !string.IsNullOrWhiteSpace(value.FilePath))
            {
                SoundFontPath = value.FilePath;
            }
            else if (value is null && SoundBanks.Count == 0)
            {
                SoundFontPath = "";
            }
        }
        finally
        {
            _isSyncingSoundBank = false;
        }
    }

    partial void OnSoundFontPathChanged(string value)
    {
        if (_isSyncingSoundBank) return;

        _isSyncingSoundBank = true;
        try
        {
            if (string.IsNullOrWhiteSpace(value))
            {
                SelectedSoundBank = null;
            }
            else
            {
                var entry = AddOrFindSoundBank(value);
                SelectedSoundBank = entry;
            }
        }
        finally
        {
            _isSyncingSoundBank = false;
        }
    }

    private SoundBankEntry AddOrFindSoundBank(string filePath, string? preferredName = null)
    {
        var existing = SoundBanks.FirstOrDefault(b =>
            string.Equals(b.FilePath, filePath, StringComparison.OrdinalIgnoreCase));
        if (existing is not null)
        {
            if (string.IsNullOrWhiteSpace(existing.Name))
                existing.Name = Path.GetFileNameWithoutExtension(filePath);
            return existing;
        }

        var entry = new SoundBankEntry
        {
            Name = !string.IsNullOrWhiteSpace(preferredName)
                ? preferredName
                : Path.GetFileNameWithoutExtension(filePath),
            FilePath = filePath
        };
        SoundBanks.Add(entry);
        return entry;
    }

    private void ScanLocalSoundbanksDirectory()
    {
        try
        {
            string dir = AppSettings.SoundbanksDirectoryPath;
            Directory.CreateDirectory(dir);

            var files = Directory.EnumerateFiles(dir)
                .Where(f =>
                {
                    string ext = Path.GetExtension(f);
                    return ext.Equals(".sf2", StringComparison.OrdinalIgnoreCase)
                        || ext.Equals(".sfz", StringComparison.OrdinalIgnoreCase);
                })
                .OrderBy(Path.GetFileName, StringComparer.OrdinalIgnoreCase);

            foreach (string file in files)
                AddOrFindSoundBank(file);
        }
        catch
        {
            // Ignore directory access issues on restricted environments
        }
    }

    /// <summary>
    /// Imports a .sf2 or .sfz soundbank file into the managed library and makes it active.
    /// Standalone .sf2 files are copied into the local Soundbanks folder when possible; .sfz
    /// files are referenced in place so their relative sample folders remain intact.
    /// </summary>
    public SoundBankEntry? ImportSoundBankFile(string sourcePath)
    {
        if (string.IsNullOrWhiteSpace(sourcePath))
            return null;

        string targetPath = sourcePath;
        string ext = Path.GetExtension(sourcePath);

        if (ext.Equals(".sf2", StringComparison.OrdinalIgnoreCase))
        {
            try
            {
                if (File.Exists(sourcePath))
                {
                    string dir = AppSettings.SoundbanksDirectoryPath;
                    Directory.CreateDirectory(dir);
                    string destPath = Path.Combine(dir, Path.GetFileName(sourcePath));

                    if (!string.Equals(Path.GetFullPath(sourcePath), Path.GetFullPath(destPath), StringComparison.OrdinalIgnoreCase))
                    {
                        File.Copy(sourcePath, destPath, overwrite: true);
                    }
                    targetPath = destPath;
                }
            }
            catch
            {
                // If copy fails or file is locked, reference the original path directly
                targetPath = sourcePath;
            }
        }

        var entry = AddOrFindSoundBank(targetPath);
        SelectedSoundBank = entry;
        if (!string.Equals(SoundFontPath, entry.FilePath, StringComparison.Ordinal))
            SoundFontPath = entry.FilePath;

        PersistSoundBanks();
        return entry;
    }

    [RelayCommand]
    private void ImportSoundBanks()
    {
        var dialog = new OpenFileDialog
        {
            Multiselect = true,
            Filter = "Piano sound banks (*.sf2;*.sfz)|*.sf2;*.sfz|SoundFont 2 (*.sf2)|*.sf2|SFZ instrument (*.sfz)|*.sfz|All files (*.*)|*.*",
            Title = "Import Piano Soundbanks (.sf2 or .sfz)"
        };

        if (dialog.ShowDialog() == true)
        {
            foreach (string file in dialog.FileNames)
                ImportSoundBankFile(file);
        }
    }

    [RelayCommand]
    private void BrowseSoundFont() => ImportSoundBanks();

    [RelayCommand]
    private void SelectSoundBank(SoundBankEntry? entry)
    {
        if (entry is not null)
            SelectedSoundBank = entry;
    }

    [RelayCommand]
    private void RemoveSoundBank(SoundBankEntry? entry)
    {
        var target = entry ?? SelectedSoundBank;
        if (target is null) return;

        int index = SoundBanks.IndexOf(target);
        bool wasSelected = ReferenceEquals(target, SelectedSoundBank)
                           || string.Equals(target.FilePath, SoundFontPath, StringComparison.OrdinalIgnoreCase);

        SoundBanks.Remove(target);

        if (wasSelected)
        {
            if (SoundBanks.Count > 0)
            {
                int nextIndex = Math.Clamp(index, 0, SoundBanks.Count - 1);
                SelectedSoundBank = SoundBanks[nextIndex];
            }
            else
            {
                SelectedSoundBank = null;
                SoundFontPath = "";
            }
        }

        PersistSoundBanks();
    }

    private void PersistSoundBanks()
    {
        try
        {
            var saved = AppSettings.Load();
            saved.SoundFontPath = SoundFontPath;
            saved.SoundBanks = SoundBanks.ToList();
            saved.Save();
        }
        catch
        {
            // Ignore persistence errors
        }
    }

    public void ApplyFrom(AppSettings settings)
    {
        UseAsio = settings.UseAsio;

        if (settings.SoundBanks is { Count: > 0 })
        {
            foreach (var bank in settings.SoundBanks)
            {
                if (!string.IsNullOrWhiteSpace(bank.FilePath))
                    AddOrFindSoundBank(bank.FilePath, bank.Name);
            }
        }

        ScanLocalSoundbanksDirectory();

        SoundFontPath = settings.SoundFontPath ?? "";
        if (string.IsNullOrEmpty(SoundFontPath) && SoundBanks.Count > 0 && SelectedSoundBank is not null)
        {
            SoundFontPath = SelectedSoundBank.FilePath;
        }

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
            SoundBanks = SoundBanks.ToList(),
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
