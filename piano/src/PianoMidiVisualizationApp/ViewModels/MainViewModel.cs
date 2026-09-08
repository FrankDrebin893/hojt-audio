using System.Collections.ObjectModel;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using PianoMidiVisualizationApp.Audio;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

public partial class MainViewModel : ObservableObject, IDisposable
{
    private readonly IMidiInputService _midiInput;
    private readonly IAudioEngine _audioEngine;
    private readonly Dispatcher _dispatcher;
    private readonly ChordAnalyzer _analyzer = new();
    private System.Threading.Timer? _activityTimer;

    public PianoKeyboardViewModel PianoKeyboard { get; }
    public SettingsViewModel Settings { get; }
    public ChatViewModel Chat { get; }

    [ObservableProperty]
    private string _statusText = "Ready";

    [ObservableProperty]
    private bool _isMidiConnected;

    [ObservableProperty]
    private bool _isAudioRunning;

    [ObservableProperty]
    private bool _midiActivity;

    [ObservableProperty]
    private string _lastMidiMessage = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentChord))]
    [NotifyPropertyChangedFor(nameof(HasChord))]
    private ChordAnalysis _analysis = ChordAnalysis.Empty;

    /// <summary>The chord name alone — what gets saved to the progression and sent to the AI.</summary>
    public string CurrentChord => Analysis.Name;

    public bool HasChord => !Analysis.IsEmpty;

    // ----- Panel visibility. Defaults are the zen layout; AppSettings overrides them on load. -----

    /// <summary>A transient surface rather than a layout panel, so it is excluded from zen mode.</summary>
    [ObservableProperty]
    private bool _isSettingsOverlayVisible;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isMidiLogVisible;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isChatPanelVisible;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isProgressionVisible = true;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isStatusBarVisible = true;

    /// <summary>
    /// Derived rather than stored, so it can never desync: turning any panel back on
    /// manually leaves zen mode with no extra bookkeeping.
    /// </summary>
    public bool IsZenMode => !IsChatPanelVisible && !IsMidiLogVisible
                          && !IsProgressionVisible && !IsStatusBarVisible;

    private const int MaxLogLines = 100;
    private const int MaxSavedChords = 8;
    public ObservableCollection<string> MidiLog { get; } = new();
    public ObservableCollection<SavedChord> SavedChords { get; } = new();

    public MainViewModel(IMidiInputService midiInput, IAudioEngine audioEngine, Dispatcher dispatcher)
    {
        _midiInput = midiInput;
        _audioEngine = audioEngine;
        _dispatcher = dispatcher;

        PianoKeyboard = new PianoKeyboardViewModel();
        Settings = new SettingsViewModel();

        var chatService = new ChatService();
        Chat = new ChatViewModel(chatService, GetMusicContext);

        _midiInput.NoteOn += OnMidiNoteOn;
        _midiInput.NoteOff += OnMidiNoteOff;
        _midiInput.MessageReceived += OnRawMidiMessage;

        Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(Settings.Volume))
                _audioEngine.Volume = Settings.Volume;
            else if (e.PropertyName == nameof(Settings.AnthropicApiKey))
                Chat.Configure(Settings.AnthropicApiKey);
        };
    }

    private MusicContext GetMusicContext()
    {
        return new MusicContext(
            CurrentChord,
            SavedChords.Select(c => c.ChordName).ToList(),
            MidiLog.TakeLast(10).ToList());
    }

    public void RefreshDevices()
    {
        Settings.MidiDevices.Clear();
        foreach (var device in _midiInput.GetAvailableDevices())
            Settings.MidiDevices.Add(device);

        Settings.AsioDriverNames.Clear();
        foreach (var name in _audioEngine.GetAsioDriverNames())
            Settings.AsioDriverNames.Add(name);

        Settings.WasapiDeviceNames.Clear();
        foreach (var name in _audioEngine.GetWasapiDeviceNames())
            Settings.WasapiDeviceNames.Add(name);

        // Auto-select first available
        if (Settings.SelectedMidiDevice == null && Settings.MidiDevices.Count > 0)
            Settings.SelectedMidiDevice = Settings.MidiDevices[0];

        if (Settings.SelectedAudioDriver == null)
        {
            if (Settings.UseAsio && Settings.AsioDriverNames.Count > 0)
                Settings.SelectedAudioDriver = Settings.AsioDriverNames[0];
            else if (Settings.WasapiDeviceNames.Count > 0)
                Settings.SelectedAudioDriver = Settings.WasapiDeviceNames[0];
        }
    }

    [RelayCommand]
    private void ConnectMidi()
    {
        if (Settings.SelectedMidiDevice == null)
        {
            StatusText = "No MIDI device selected";
            return;
        }

        try
        {
            _midiInput.Open(Settings.SelectedMidiDevice.Index);
            IsMidiConnected = true;
            StatusText = $"MIDI connected: {Settings.SelectedMidiDevice.Name}";
        }
        catch (Exception ex)
        {
            IsMidiConnected = false;
            StatusText = $"MIDI error: {ex.Message}";
        }
    }

    [RelayCommand]
    private void DisconnectMidi()
    {
        _midiInput.Close();
        IsMidiConnected = false;
        StatusText = "MIDI disconnected";
    }

    [RelayCommand]
    private void StartAudio()
    {
        if (string.IsNullOrEmpty(Settings.SoundFontPath))
        {
            StatusText = "Please select a SoundFont file first";
            return;
        }

        if (string.IsNullOrEmpty(Settings.SelectedAudioDriver))
        {
            StatusText = "No audio driver selected";
            return;
        }

        try
        {
            _audioEngine.Initialize(Settings.SelectedAudioDriver, Settings.UseAsio, Settings.SoundFontPath);
            _audioEngine.Volume = Settings.Volume;
            _audioEngine.Start();
            IsAudioRunning = true;
            StatusText = $"Audio started: {Settings.SelectedAudioDriver} ({(Settings.UseAsio ? "ASIO" : "WASAPI")})";
        }
        catch (Exception ex)
        {
            IsAudioRunning = false;
            StatusText = $"Audio error: {ex.Message}";
        }
    }

    [RelayCommand]
    private void StopAudio()
    {
        _audioEngine.Stop();
        IsAudioRunning = false;
        StatusText = "Audio stopped";
    }

    [RelayCommand]
    private void BrowseSoundFont()
    {
        var dialog = new OpenFileDialog
        {
            Filter = "SoundFont/SFZ files (*.sf2;*.sfz)|*.sf2;*.sfz|SoundFont files (*.sf2)|*.sf2|SFZ files (*.sfz)|*.sfz|All files (*.*)|*.*",
            Title = "Select SoundFont/SFZ File"
        };

        if (dialog.ShowDialog() == true)
        {
            Settings.SoundFontPath = dialog.FileName;
        }
    }

    [RelayCommand]
    private void RefreshDeviceList()
    {
        RefreshDevices();
        StatusText = "Devices refreshed";
    }

    [RelayCommand]
    private void SaveCurrentChord()
    {
        if (string.IsNullOrEmpty(CurrentChord))
        {
            StatusText = "No chord to save";
            return;
        }

        if (SavedChords.Count >= MaxSavedChords)
        {
            StatusText = "Maximum 8 chords saved - remove one first";
            return;
        }

        var pressedNotes = PianoKeyboard.GetPressedNotes().OrderBy(n => n).ToList();
        if (pressedNotes.Count == 0)
        {
            StatusText = "No notes held";
            return;
        }

        var noteNames = pressedNotes.Select(MusicNaming.WithOctave).ToList();

        var savedChord = new SavedChord
        {
            ChordName = CurrentChord,
            NoteNumbers = pressedNotes,
            NoteNames = noteNames
        };

        SavedChords.Add(savedChord);
        StatusText = $"Saved chord: {CurrentChord}";
    }

    [RelayCommand]
    private void RemoveChord(SavedChord? chord)
    {
        if (chord != null && SavedChords.Contains(chord))
        {
            SavedChords.Remove(chord);
            StatusText = $"Removed chord: {chord.ChordName}";
        }
    }

    [RelayCommand]
    private void ToggleSettingsOverlay() => IsSettingsOverlayVisible = !IsSettingsOverlayVisible;

    [RelayCommand]
    private void CloseSettingsOverlay() => IsSettingsOverlayVisible = false;

    [RelayCommand]
    private void ToggleMidiLog() => IsMidiLogVisible = !IsMidiLogVisible;

    [RelayCommand]
    private void ToggleChatPanel() => IsChatPanelVisible = !IsChatPanelVisible;

    [RelayCommand]
    private void ToggleProgression() => IsProgressionVisible = !IsProgressionVisible;

    [RelayCommand]
    private void ToggleStatusBar() => IsStatusBarVisible = !IsStatusBarVisible;

    private readonly record struct PanelLayout(bool Chat, bool MidiLog, bool Progression, bool StatusBar);

    private PanelLayout? _preZenLayout;

    [RelayCommand]
    private void ToggleZenMode()
    {
        if (IsZenMode)
        {
            // Nothing was saved if the app started in zen — restore a sensible layout instead.
            var restore = _preZenLayout ?? new PanelLayout(false, false, true, true);
            IsChatPanelVisible = restore.Chat;
            IsMidiLogVisible = restore.MidiLog;
            IsProgressionVisible = restore.Progression;
            IsStatusBarVisible = restore.StatusBar;
        }
        else
        {
            _preZenLayout = new PanelLayout(
                IsChatPanelVisible, IsMidiLogVisible, IsProgressionVisible, IsStatusBarVisible);
            IsChatPanelVisible = IsMidiLogVisible = IsProgressionVisible = IsStatusBarVisible = false;
            IsSettingsOverlayVisible = false;
        }
    }

    /// <summary>
    /// Applies saved settings, including the panel flags that <see cref="SettingsViewModel"/>
    /// cannot see. The overlay's visibility is deliberately not restored.
    /// </summary>
    public void ApplySettings(AppSettings saved)
    {
        Settings.ApplyFrom(saved);
        IsMidiLogVisible = saved.ShowMidiLog;
        IsChatPanelVisible = saved.ShowChatPanel;
        IsProgressionVisible = saved.ShowProgression;
        IsStatusBarVisible = saved.ShowStatusBar;
    }

    public AppSettings CaptureSettings()
    {
        var saved = Settings.ToAppSettings();
        saved.ShowMidiLog = IsMidiLogVisible;
        saved.ShowChatPanel = IsChatPanelVisible;
        saved.ShowProgression = IsProgressionVisible;
        saved.ShowStatusBar = IsStatusBarVisible;
        return saved;
    }

    public void AutoConnect()
    {
        // Auto-connect MIDI if a device is selected
        if (Settings.SelectedMidiDevice != null && !IsMidiConnected)
        {
            try
            {
                _midiInput.Open(Settings.SelectedMidiDevice.Index);
                IsMidiConnected = true;
                StatusText = $"MIDI auto-connected: {Settings.SelectedMidiDevice.Name}";
            }
            catch (Exception ex)
            {
                StatusText = $"MIDI auto-connect failed: {ex.Message}";
            }
        }

        // Auto-start audio if we have a SoundFont and audio driver
        if (!string.IsNullOrEmpty(Settings.SoundFontPath)
            && System.IO.File.Exists(Settings.SoundFontPath)
            && !string.IsNullOrEmpty(Settings.SelectedAudioDriver)
            && !IsAudioRunning)
        {
            try
            {
                _audioEngine.Initialize(Settings.SelectedAudioDriver, Settings.UseAsio, Settings.SoundFontPath);
                _audioEngine.Volume = Settings.Volume;
                _audioEngine.Start();
                IsAudioRunning = true;
                StatusText = $"Audio auto-started: {Settings.SelectedAudioDriver} ({(Settings.UseAsio ? "ASIO" : "WASAPI")})";
            }
            catch (Exception ex)
            {
                IsAudioRunning = false;
                StatusText = $"Audio auto-start failed: {ex.Message}";
            }
        }
    }

    private void OnRawMidiMessage(object? sender, RawMidiMessageEventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            MidiActivity = true;
            LastMidiMessage = e.Description;
            StatusText = e.Description;

            MidiLog.Add($"[{DateTime.Now:HH:mm:ss.fff}] {e.Description}");
            while (MidiLog.Count > MaxLogLines)
                MidiLog.RemoveAt(0);

            // Turn off the activity light after 80ms
            _activityTimer?.Dispose();
            _activityTimer = new System.Threading.Timer(_ =>
            {
                _dispatcher.BeginInvoke(() => MidiActivity = false);
            }, null, 80, System.Threading.Timeout.Infinite);
        });
    }

    private void OnMidiNoteOn(object? sender, NoteEventArgs e)
    {
        _audioEngine.NoteOn(e.Channel, e.NoteNumber, e.Velocity);

        _dispatcher.BeginInvoke(() =>
        {
            PianoKeyboard.SetKeyPressed(e.NoteNumber, e.Velocity);
            Analysis = _analyzer.Analyze(PianoKeyboard.GetPressedNotes());
        });
    }

    private void OnMidiNoteOff(object? sender, NoteEventArgs e)
    {
        _audioEngine.NoteOff(e.Channel, e.NoteNumber);

        _dispatcher.BeginInvoke(() =>
        {
            PianoKeyboard.SetKeyReleased(e.NoteNumber);
            Analysis = _analyzer.Analyze(PianoKeyboard.GetPressedNotes());
        });
    }

    public void Dispose()
    {
        _activityTimer?.Dispose();
        _midiInput.NoteOn -= OnMidiNoteOn;
        _midiInput.NoteOff -= OnMidiNoteOff;
        _midiInput.MessageReceived -= OnRawMidiMessage;
        _audioEngine.Stop();
        _audioEngine.Dispose();
        _midiInput.Close();
        _midiInput.Dispose();
    }
}
