using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using PianoMidiVisualizationApp.Audio;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;
using PianoMidiVisualizationApp.Services.Recording;

namespace PianoMidiVisualizationApp.ViewModels;

public partial class MainViewModel : ObservableObject, IDisposable
{
    private readonly IMidiInputService _midiInput;
    private readonly IAudioEngine _audioEngine;
    private readonly Dispatcher _dispatcher;
    private readonly ChordAnalyzer _analyzer = new();
    private System.Threading.Timer? _activityTimer;

    /// <summary>Every pitch class, i.e. nothing muted.</summary>
    private const int AllPitchClasses = 0xFFF;

    /// <summary>
    /// The pitch classes the audio engine is allowed to sound, as a 12-bit mask. Read on the
    /// MIDI callback thread, so it is one volatile int rather than three settings properties
    /// that thread could catch mid-update and combine into a key that was never selected.
    /// </summary>
    private volatile int _audiblePitchClasses = AllPitchClasses;

    // ----- Live input and app-generated notes share keys, voices and the readout -----

    /// <summary>App-generated notes all sound on MIDI channel 1 (0-based 0).</summary>
    private const int AppChannel = 0;

    /// <summary>
    /// Guards the audio bookkeeping below. Live notes arrive on the MIDI callback thread and app
    /// notes on a player's timing thread, and both engines release every voice on a key with a
    /// single NoteOff, so each side has to know whether the other still holds that key.
    /// </summary>
    private readonly object _soundingLock = new();

    /// <summary>Live notes the engine is sounding, indexed channel * 128 + note.</summary>
    private readonly bool[] _liveSounding = new bool[16 * 128];

    /// <summary>App note-ons not yet released, per note (all on <see cref="AppChannel"/>).</summary>
    private readonly int[] _appSounding = new int[128];

    // The same split for the keyboard lights, touched only on the UI thread: a key stays lit
    // while either source holds it, so one letting go never darkens a key the other still holds.
    private readonly bool[] _liveHeld = new bool[128];
    private readonly int[] _appHeld = new int[128];

    public PianoKeyboardViewModel PianoKeyboard { get; }
    public SettingsViewModel Settings { get; }
    public ChatViewModel Chat { get; }
    public RecorderViewModel Recorder { get; }

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

    // ----- Metronome -----

    /// <summary>Starts off on every launch and is never persisted.</summary>
    [ObservableProperty]
    private bool _isMetronomeOn;

    /// <summary>Whether the most recent beat was a downbeat. Set just before <see cref="MetronomeBeat"/>.</summary>
    [ObservableProperty]
    private bool _isMetronomeAccentBeat;

    /// <summary>
    /// Raised on the UI thread once per click, marshalled from the audio thread that scheduled
    /// it. The beat light flashes from this rather than from a UI timer of its own, so it can
    /// never drift away from what is heard.
    /// </summary>
    public event EventHandler? MetronomeBeat;

    private readonly TapTempo _tapTempo = new();
    private readonly Stopwatch _tapClock = Stopwatch.StartNew();

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

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isRecorderVisible;

    /// <summary>
    /// Derived rather than stored, so it can never desync: turning any panel back on
    /// manually leaves zen mode with no extra bookkeeping.
    /// </summary>
    public bool IsZenMode => !IsChatPanelVisible && !IsMidiLogVisible
                          && !IsProgressionVisible && !IsStatusBarVisible
                          && !IsRecorderVisible;

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

        Recorder = new RecorderViewModel(dispatcher, PlayNoteOn, PlayNoteOff,
                                         () => ExportTempo, status => StatusText = status);

        _midiInput.NoteOn += OnMidiNoteOn;
        _midiInput.NoteOff += OnMidiNoteOff;
        _midiInput.MessageReceived += OnRawMidiMessage;

        if (_audioEngine.Metronome is { } metronome)
            metronome.BeatStarted += OnMetronomeBeat;
        ApplyMetronomeSettings();

        Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(Settings.Volume))
                _audioEngine.Volume = Settings.Volume;
            else if (e.PropertyName == nameof(Settings.AnthropicApiKey))
                Chat.Configure(Settings.AnthropicApiKey);
            else if (e.PropertyName is nameof(Settings.KeyTonicPitchClass)
                                    or nameof(Settings.KeyScale))
            {
                PianoKeyboard.SetKey(Settings.CurrentKey);
                UpdateAudiblePitchClasses();
                RefreshAnalysis();   // the readout re-spells with the new key
            }
            else if (e.PropertyName == nameof(Settings.MuteOutOfKeyNotes))
                UpdateAudiblePitchClasses();
            else if (e.PropertyName is nameof(Settings.MetronomeBpm)
                                    or nameof(Settings.MetronomeBeatsPerBar)
                                    or nameof(Settings.MetronomeVolume))
                ApplyMetronomeSettings();
        };
    }

    private MusicContext GetMusicContext()
    {
        return new MusicContext(
            CurrentChord,
            SavedChords.Select(c => c.ChordName).ToList(),
            MidiLog.TakeLast(10).ToList());
    }

    /// <summary>Whether note names should read as flats, per the selected key signature.</summary>
    private bool UseFlats => Settings.CurrentKey?.UsesFlats ?? false;

    /// <summary>The tempo written into exported takes.</summary>
    private int ExportTempo => TakeMidiExporter.DefaultBpm;

    private void UpdateAudiblePitchClasses() =>
        _audiblePitchClasses = Settings.MuteOutOfKeyNotes && Settings.CurrentKey is { } key
            ? key.PitchClassMask
            : AllPitchClasses;

    private void ApplyMetronomeSettings()
    {
        if (_audioEngine.Metronome is not { } metronome) return;
        metronome.Bpm = Settings.MetronomeBpm;
        metronome.BeatsPerBar = Settings.MetronomeBeatsPerBar;
        metronome.Volume = Settings.MetronomeVolume;
    }

    partial void OnIsMetronomeOnChanged(bool value)
    {
        if (_audioEngine.Metronome is { } metronome)
            metronome.IsEnabled = value;

        StatusText = !value ? "Metronome off"
            : IsAudioRunning ? $"Metronome on: {Settings.MetronomeBpm} BPM"
            : "Metronome on - start audio to hear it";
    }

    private void OnMetronomeBeat(object? sender, MetronomeBeatEventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            IsMetronomeAccentBeat = e.IsAccent;
            MetronomeBeat?.Invoke(this, EventArgs.Empty);
        });
    }

    private void RefreshAnalysis() =>
        Analysis = _analyzer.Analyze(PianoKeyboard.GetPressedNotes(), UseFlats);

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

        var noteNames = pressedNotes.Select(n => MusicNaming.WithOctave(n, UseFlats)).ToList();

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

    /// <summary>Turns the key highlight off. Picking from either dropdown turns it back on.</summary>
    [RelayCommand]
    private void ClearKey() => Settings.KeyTonicPitchClass = null;

    [RelayCommand]
    private void ToggleMetronome() => IsMetronomeOn = !IsMetronomeOn;

    [RelayCommand]
    private void IncreaseMetronomeBpm() => Settings.MetronomeBpm++;

    [RelayCommand]
    private void DecreaseMetronomeBpm() => Settings.MetronomeBpm--;

    /// <summary>Sets the tempo from the average of the last few taps; the first tap only starts the count.</summary>
    [RelayCommand]
    private void TapMetronome()
    {
        if (_tapTempo.Tap(_tapClock.Elapsed) is { } bpm)
            Settings.MetronomeBpm = bpm;
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

    [RelayCommand]
    private void ToggleRecorder() => IsRecorderVisible = !IsRecorderVisible;

    private readonly record struct PanelLayout(bool Chat, bool MidiLog, bool Progression, bool StatusBar,
                                               bool Recorder);

    private PanelLayout? _preZenLayout;

    [RelayCommand]
    private void ToggleZenMode()
    {
        if (IsZenMode)
        {
            // Nothing was saved if the app started in zen — restore a sensible layout instead.
            var restore = _preZenLayout ?? new PanelLayout(false, false, true, true, false);
            IsChatPanelVisible = restore.Chat;
            IsMidiLogVisible = restore.MidiLog;
            IsProgressionVisible = restore.Progression;
            IsStatusBarVisible = restore.StatusBar;
            IsRecorderVisible = restore.Recorder;
        }
        else
        {
            _preZenLayout = new PanelLayout(
                IsChatPanelVisible, IsMidiLogVisible, IsProgressionVisible, IsStatusBarVisible,
                IsRecorderVisible);
            IsChatPanelVisible = IsMidiLogVisible = IsProgressionVisible = IsStatusBarVisible = false;
            IsRecorderVisible = false;
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
        IsRecorderVisible = saved.ShowRecorder;
        PianoKeyboard.SetKey(Settings.CurrentKey);
        UpdateAudiblePitchClasses();
        ApplyMetronomeSettings();
    }

    public AppSettings CaptureSettings()
    {
        var saved = Settings.ToAppSettings();
        saved.ShowMidiLog = IsMidiLogVisible;
        saved.ShowChatPanel = IsChatPanelVisible;
        saved.ShowProgression = IsProgressionVisible;
        saved.ShowStatusBar = IsStatusBarVisible;
        saved.ShowRecorder = IsRecorderVisible;
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
        // Timestamped first, before the audio engine or anything else can delay it.
        long timestamp = Stopwatch.GetTimestamp();
        Recorder.CaptureNoteOn(e.NoteNumber, e.Velocity, timestamp);

        // An out-of-key note is silenced, not swallowed: it still lights its key and still
        // counts toward the chord readout, so you can see what you actually played.
        bool audible = (_audiblePitchClasses & (1 << MusicNaming.PitchClassOf(e.NoteNumber))) != 0;

        lock (_soundingLock)
        {
            if (audible)
            {
                _audioEngine.NoteOn(e.Channel, e.NoteNumber, e.Velocity);
                if (TryGetSlot(e.Channel, e.NoteNumber, out int slot))
                    _liveSounding[slot] = true;
            }

            _dispatcher.BeginInvoke(() =>
            {
                if (IsNote(e.NoteNumber)) _liveHeld[e.NoteNumber] = true;
                PianoKeyboard.SetKeyPressed(e.NoteNumber, e.Velocity);
                RefreshAnalysis();
            });
        }
    }

    private void OnMidiNoteOff(object? sender, NoteEventArgs e)
    {
        long timestamp = Stopwatch.GetTimestamp();
        Recorder.CaptureNoteOff(e.NoteNumber, timestamp);

        lock (_soundingLock)
        {
            bool appHoldsKey = false;
            if (TryGetSlot(e.Channel, e.NoteNumber, out int slot))
            {
                _liveSounding[slot] = false;
                appHoldsKey = e.Channel == AppChannel && _appSounding[e.NoteNumber] > 0;
            }

            // Always released, never gated: a note-off for a note that never sounded is a no-op,
            // whereas gating here would strand a note that was audible when the key changed
            // under it and would then sustain forever. The one exception is a key an app note is
            // also sounding: releasing it would cut that note short, and the app's own note-off
            // releases the key once it is done with it.
            if (!appHoldsKey)
                _audioEngine.NoteOff(e.Channel, e.NoteNumber);

            _dispatcher.BeginInvoke(() =>
            {
                if (IsNote(e.NoteNumber))
                {
                    _liveHeld[e.NoteNumber] = false;
                    if (_appHeld[e.NoteNumber] > 0) return;   // still held by an app note
                }
                PianoKeyboard.SetKeyReleased(e.NoteNumber);
                RefreshAnalysis();
            });
        }
    }

    /// <summary>
    /// Sounds a note the app generated itself: playback, a clicked chord, a practice prompt.
    /// It bypasses mute-out-of-key, lights the key and feeds the chord readout just like a live
    /// note, but is never recorded. Pair every call with <see cref="PlayNoteOff"/>. Safe from any
    /// thread, including a <see cref="Services.Playback.NoteSequencePlayer"/>'s timing thread.
    /// </summary>
    public void PlayNoteOn(int note, int velocity)
    {
        if (!IsNote(note)) return;
        velocity = Math.Clamp(velocity, 1, 127);

        // BeginInvoke inside the lock, so the UI sees on/off pairs in the order the audio engine
        // did, even when two players touch the same key from different threads.
        lock (_soundingLock)
        {
            _appSounding[note]++;
            _audioEngine.NoteOn(AppChannel, note, velocity);

            _dispatcher.BeginInvoke(() =>
            {
                _appHeld[note]++;
                PianoKeyboard.SetKeyPressed(note, velocity);
                RefreshAnalysis();
            });
        }
    }

    /// <summary>Releases a note started by <see cref="PlayNoteOn"/>. An unmatched call is ignored.</summary>
    public void PlayNoteOff(int note)
    {
        if (!IsNote(note)) return;

        lock (_soundingLock)
        {
            if (_appSounding[note] == 0) return;
            _appSounding[note]--;

            // Also held live on the same channel: the voice carries on as the live note, and the
            // live key's note-off releases it.
            if (_appSounding[note] == 0 && !_liveSounding[AppChannel * 128 + note])
                _audioEngine.NoteOff(AppChannel, note);

            _dispatcher.BeginInvoke(() =>
            {
                if (_appHeld[note] > 0) _appHeld[note]--;
                if (_appHeld[note] > 0 || _liveHeld[note]) return;
                PianoKeyboard.SetKeyReleased(note);
                RefreshAnalysis();
            });
        }
    }

    private static bool IsNote(int note) => note is >= 0 and <= 127;

    private static bool TryGetSlot(int channel, int note, out int slot)
    {
        slot = channel * 128 + note;
        return channel is >= 0 and <= 15 && IsNote(note);
    }

    public void Dispose()
    {
        // First, while the engine can still take the note-offs the player sends on the way out.
        Recorder.Dispose();
        _activityTimer?.Dispose();
        _midiInput.NoteOn -= OnMidiNoteOn;
        _midiInput.NoteOff -= OnMidiNoteOff;
        _midiInput.MessageReceived -= OnRawMidiMessage;
        if (_audioEngine.Metronome is { } metronome)
            metronome.BeatStarted -= OnMetronomeBeat;
        _audioEngine.Stop();
        _audioEngine.Dispose();
        _midiInput.Close();
        _midiInput.Dispose();
    }
}
