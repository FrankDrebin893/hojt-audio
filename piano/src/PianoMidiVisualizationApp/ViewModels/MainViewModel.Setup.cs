using System.Collections.Specialized;
using System.IO;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>
/// First run and system status. Until the keyboard is connected and audio is running, the empty
/// readout shows a three-step setup checklist, and the title bar's status chip shows the same
/// state wherever you are. Both are rebuilt from live state by <see cref="RefreshSetupStatus"/>,
/// so neither can drift from what the Settings overlay shows.
/// </summary>
public partial class MainViewModel
{
    public SetupStep KeyboardStep { get; } = new(1);
    public SetupStep SoundStep { get; } = new(2);
    public SetupStep AudioStep { get; } = new(3);

    private SetupStep[]? _setupSteps;
    public IReadOnlyList<SetupStep> SetupSteps => _setupSteps ??= [KeyboardStep, SoundStep, AudioStep];

    /// <summary>
    /// False until <see cref="AutoConnect"/> has tried the saved keyboard and audio output. Until
    /// then nothing is known to be missing, so neither the checklist nor the chip's warning may
    /// show: on a normal launch they would flash up and vanish again.
    /// </summary>
    [ObservableProperty]
    private bool _hasAttemptedAutoConnect;

    /// <summary>What stopped the keyboard connecting, in a few plain words; null while nothing has.</summary>
    [ObservableProperty]
    private string? _midiProblem;

    /// <summary>What stopped audio starting, in a few plain words; null while nothing has.</summary>
    [ObservableProperty]
    private string? _audioProblem;

    // The longer explanation behind each problem, with the driver's own message, for tooltips.
    private string? _midiProblemDetail;
    private string? _audioProblemDetail;

    /// <summary>Saved. The chip still shows what isn't ready, so hiding the checklist loses nothing.</summary>
    [ObservableProperty]
    private bool _isSetupChecklistDismissed;

    /// <summary>The checklist, for the view to show whenever no chord is.</summary>
    [ObservableProperty]
    private bool _isSetupChecklistVisible;

    /// <summary>The faint "Play something", for the view to show whenever no chord is.</summary>
    [ObservableProperty]
    private bool _isPlayPromptVisible;

    /// <summary>The keyboard isn't connected or audio isn't running: the chip turns amber.</summary>
    [ObservableProperty]
    private bool _statusNeedsAttention;

    [ObservableProperty]
    private string _statusChipText = "Connecting…";

    [ObservableProperty]
    private string _statusChipToolTip = "";

    /// <summary>
    /// True for a moment after each note played on the keyboard, so the chip's dot flickers with
    /// your playing. Notes only, unlike <see cref="MidiActivity"/>: many keyboards send active
    /// sensing three times a second, which would make the dot blink on its own.
    /// </summary>
    [ObservableProperty]
    private bool _isKeyActivity;

    private DispatcherTimer? _keyActivityTimer;

    private string AudioMode => Settings.UseAsio ? "ASIO" : "WASAPI";

    private void InitializeSetupStatus()
    {
        Settings.PropertyChanged += (_, e) =>
        {
            // A problem belongs to the device or file it happened with: choosing another clears it.
            if (e.PropertyName == nameof(Settings.SelectedMidiDevice))
                SetMidiProblem(null);
            else if (e.PropertyName is nameof(Settings.SelectedAudioDriver) or nameof(Settings.UseAsio)
                                    or nameof(Settings.SoundFontPath))
                SetAudioProblem(null);
            else
                return;
            RefreshSetupStatus();
        };
        Settings.MidiDevices.CollectionChanged += OnMidiDevicesChanged;
        Settings.AsioDriverNames.CollectionChanged += OnAudioOutputsChanged;
        Settings.WasapiDeviceNames.CollectionChanged += OnAudioOutputsChanged;
        _midiInput.NoteOn += OnKeyPlayed;
        RefreshSetupStatus();
    }

    private void DisposeSetupStatus()
    {
        _midiInput.NoteOn -= OnKeyPlayed;
        _keyActivityTimer?.Stop();
    }

    private void OnMidiDevicesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        // "None found" is stale the moment the list changes; a failed open stays until retried.
        if (Settings.SelectedMidiDevice == null) SetMidiProblem(null);
        RefreshSetupStatus();
    }

    private void OnAudioOutputsChanged(object? sender, NotifyCollectionChangedEventArgs e) => RefreshSetupStatus();

    partial void OnIsMidiConnectedChanged(bool value) => RefreshSetupStatus();
    partial void OnIsAudioRunningChanged(bool value) => RefreshSetupStatus();
    partial void OnHasAttemptedAutoConnectChanged(bool value) => RefreshSetupStatus();
    partial void OnMidiProblemChanged(string? value) => RefreshSetupStatus();
    partial void OnAudioProblemChanged(string? value) => RefreshSetupStatus();
    partial void OnIsSetupChecklistDismissedChanged(bool value) => RefreshSetupStatus();

    private void SetMidiProblem(string? problem, string? detail = null)
    {
        _midiProblemDetail = problem == null ? null : detail;
        if (MidiProblem == problem) RefreshSetupStatus();   // the same words, maybe a new cause
        else MidiProblem = problem;
    }

    private void SetAudioProblem(string? problem, string? detail = null)
    {
        _audioProblemDetail = problem == null ? null : detail;
        if (AudioProblem == problem) RefreshSetupStatus();
        else AudioProblem = problem;
    }

    // ------------------------------------------------------------------ status

    /// <summary>Rewrites the checklist and the chip from the connection and settings state.</summary>
    private void RefreshSetupStatus()
    {
        var device = Settings.SelectedMidiDevice;
        string soundPath = Settings.SoundFontPath;
        bool soundChosen = !string.IsNullOrEmpty(soundPath);
        string soundName = soundChosen ? Path.GetFileName(soundPath) : "";
        bool soundFound = soundChosen && File.Exists(soundPath);
        string? driver = Settings.SelectedAudioDriver;
        string output = string.IsNullOrEmpty(driver) ? "" : $"{driver} · {AudioMode}";

        if (IsMidiConnected)
            KeyboardStep.Update(true, "Keyboard connected", device?.Name ?? "",
                                "Change the keyboard in Settings.");
        else if (MidiProblem != null)
            KeyboardStep.Update(false, "Connect your keyboard", MidiProblem,
                                $"{_midiProblemDetail}\nClick to try again.", hasProblem: true);
        else if (device != null)
            KeyboardStep.Update(false, "Connect your keyboard", device.Name,
                                $"Connect to {device.Name}.");
        else
            KeyboardStep.Update(false, "Connect your keyboard", "None found yet",
                                "Plug in your MIDI keyboard and switch it on, then click here.");

        if (soundFound)
            SoundStep.Update(true, "Piano sound chosen", soundName,
                             $"{soundPath}\nChange the sound in Settings.");
        else if (soundChosen)
            SoundStep.Update(false, "Choose a piano sound", $"Can't find {soundName}",
                             $"{soundPath} has been moved or deleted.\nClick to choose it again.", hasProblem: true);
        else
            SoundStep.Update(false, "Choose a piano sound", ".sf2 or .sfz file",
                             "Pick a SoundFont (.sf2) or SFZ (.sfz) piano.\nAudio starts as soon as you choose one.");

        if (IsAudioRunning)
            AudioStep.Update(true, "Audio on", output, "Change the output in Settings.");
        else if (AudioProblem != null)
            AudioStep.Update(false, "Start audio", AudioProblem,
                             $"{_audioProblemDetail}\nClick to try again, or pick another output in Settings.", hasProblem: true);
        else if (string.IsNullOrEmpty(driver))
            AudioStep.Update(false, "Start audio", "No audio output found",
                             "Connect speakers or headphones, then click here.", hasProblem: true);
        else
            AudioStep.Update(false, "Start audio", output,
                             soundFound ? $"Play through {driver}." : "Choose a piano sound first. Clicking here asks for one.");

        bool nextTaken = false;
        foreach (var step in SetupSteps)
        {
            if (step.IsDone) continue;
            step.State = nextTaken ? SetupStepState.Pending : SetupStepState.Next;
            nextTaken = true;
        }

        bool ready = IsMidiConnected && IsAudioRunning;
        IsSetupChecklistVisible = HasAttemptedAutoConnect && !ready && !IsSetupChecklistDismissed;
        // With the checklist hidden and no audio, the keys still light and the readout still reads:
        // prompting is honest as long as the keyboard is there to play.
        IsPlayPromptVisible = HasAttemptedAutoConnect && IsMidiConnected && (IsAudioRunning || IsSetupChecklistDismissed);
        StatusNeedsAttention = HasAttemptedAutoConnect && !ready;

        if (!HasAttemptedAutoConnect)
        {
            StatusChipText = "Connecting…";
            StatusChipToolTip = "Connecting to your keyboard and audio output";
            return;
        }

        string keyboardPart = IsMidiConnected ? device?.Name ?? "Keyboard"
                            : device == null ? "No keyboard"
                            : MidiProblem != null ? "Keyboard error"
                            : "Keyboard not connected";
        string audioPart = IsAudioRunning ? AudioMode
                         : AudioProblem != null ? "Audio error"
                         : !soundFound ? "No sound"
                         : "Audio off";
        StatusChipText = $"{keyboardPart} · {audioPart}";

        string keyboardLine = IsMidiConnected ? $"{device?.Name}, connected"
                            : MidiProblem != null ? _midiProblemDetail ?? MidiProblem
                            : device != null ? $"{device.Name}, not connected"
                            : "none found";
        string soundLine = soundFound ? soundName
                         : soundChosen ? $"can't find {soundName}"
                         : "none chosen";
        string audioLine = IsAudioRunning ? $"on, through {driver} ({AudioMode})"
                         : AudioProblem != null ? _audioProblemDetail ?? AudioProblem
                         : "off";
        StatusChipToolTip = $"Keyboard: {keyboardLine}\nSound: {soundLine}\nAudio: {audioLine}\nClick for settings (Ctrl+,)";
    }

    // ------------------------------------------------------------------ actions

    /// <summary>Does what a checklist step asks. A finished step opens Settings, where it can be changed.</summary>
    [RelayCommand]
    private void RunSetupStep(SetupStep? step)
    {
        if (step == null) return;

        if (step.IsDone)
            IsSettingsOverlayVisible = true;
        else if (step == KeyboardStep)
            ConnectKeyboard();
        else if (step == SoundStep)
            ChooseSound();
        else if (step == AudioStep)
        {
            if (!File.Exists(Settings.SoundFontPath))
            {
                ChooseSound();   // starts audio too, once there is something to play
                return;
            }
            if (string.IsNullOrEmpty(Settings.SelectedAudioDriver))
                RefreshDevices();
            StartAudio();
        }
    }

    [RelayCommand]
    private void DismissSetupChecklist() => IsSetupChecklistDismissed = true;

    /// <summary>Connects the chosen keyboard, first looking again if there is none or the last try failed.</summary>
    private void ConnectKeyboard()
    {
        if (Settings.SelectedMidiDevice == null || MidiProblem != null)
        {
            RefreshMidiDevices();
            if (Settings.SelectedMidiDevice == null)
            {
                SetMidiProblem("Still none found. Is it on?",
                               "No MIDI keyboard found. Check it's plugged in and switched on.");
                StatusText = "No MIDI keyboard found";
                return;
            }
        }

        ConnectMidi();
    }

    /// <summary>
    /// Re-reads the MIDI inputs only, keeping the chosen one if it is still there.
    /// <see cref="RefreshDevices"/> would also re-read the audio outputs and could lose their selection.
    /// </summary>
    private void RefreshMidiDevices()
    {
        string? wanted = Settings.SelectedMidiDevice?.Name;
        var devices = _midiInput.GetAvailableDevices();

        Settings.MidiDevices.Clear();
        foreach (var device in devices)
            Settings.MidiDevices.Add(device);

        Settings.SelectedMidiDevice = Settings.MidiDevices.FirstOrDefault(d => d.Name == wanted)
                                      ?? Settings.MidiDevices.FirstOrDefault();
    }

    /// <summary>
    /// Picks the sound and, if audio isn't running yet, starts it: choosing a piano and then
    /// having to press Start as well would be a step for nothing.
    /// </summary>
    private void ChooseSound()
    {
        string before = Settings.SoundFontPath;
        BrowseSoundFont();
        if (Settings.SoundFontPath != before && !IsAudioRunning)
            StartAudio();
    }

    private void TryConnectMidi(DeviceInfo device, string verb)
    {
        try
        {
            _midiInput.Open(device.Index);
            SetMidiProblem(null);
            IsMidiConnected = true;
            StatusText = $"{verb}: {device.Name}";
        }
        catch (Exception ex)
        {
            IsMidiConnected = false;
            // What NAudio's MmException says when another program already has the port open.
            if (ex.Message.Contains("AlreadyAllocated", StringComparison.OrdinalIgnoreCase))
            {
                SetMidiProblem("In use by another app",
                               $"{device.Name} is open in another app.\nClose that app, then connect again.");
                StatusText = $"Couldn't connect to {device.Name}: it's in use by another app";
            }
            else
            {
                SetMidiProblem($"Couldn't connect to {device.Name}",
                               $"{device.Name} couldn't be opened: {ex.Message}");
                StatusText = $"Couldn't connect to {device.Name}: {ex.Message}";
            }
        }
    }

    private void TryStartAudio(string verb)
    {
        string driver = Settings.SelectedAudioDriver ?? "";
        string soundName = Path.GetFileName(Settings.SoundFontPath);
        try
        {
            _audioEngine.Initialize(driver, Settings.UseAsio, Settings.SoundFontPath);
            _audioEngine.Volume = Settings.Volume;
            _audioEngine.Start();
            SetAudioProblem(null);
            IsAudioRunning = true;
            StatusText = $"{verb}: {driver} ({AudioMode})";
        }
        catch (Exception ex)
        {
            IsAudioRunning = false;
            // Initialize loads the sound and opens the output, so either can throw. A file that
            // won't read or parse throws one of these; a driver or device throws anything else.
            if (ex is IOException or InvalidDataException or UnauthorizedAccessException or FormatException)
            {
                SetAudioProblem($"Couldn't load {soundName}",
                                $"{soundName} couldn't be loaded: {ex.Message}");
                StatusText = $"Couldn't load {soundName}: {ex.Message}";
            }
            else
            {
                SetAudioProblem($"Couldn't start {driver}",
                                $"{driver} ({AudioMode}) didn't start: {ex.Message}");
                StatusText = $"Couldn't start audio on {driver}: {ex.Message}";
            }
        }
    }

    /// <summary>
    /// Settles ASIO versus WASAPI by which list the chosen output is in. A fresh install asks for
    /// ASIO, but <see cref="RefreshDevices"/> falls back to a WASAPI device when no ASIO driver is
    /// installed, and ASIO can't open that: without this, the first "Start audio" would fail.
    /// </summary>
    private void MatchAudioModeToDriver()
    {
        string? driver = Settings.SelectedAudioDriver;
        if (string.IsNullOrEmpty(driver)) return;

        bool isAsio = Settings.AsioDriverNames.Contains(driver);
        bool isWasapi = Settings.WasapiDeviceNames.Contains(driver);
        if (Settings.UseAsio && !isAsio && isWasapi)
            Settings.UseAsio = false;
        else if (!Settings.UseAsio && !isWasapi && isAsio)
            Settings.UseAsio = true;
    }

    // ------------------------------------------------------------------ activity

    private void OnKeyPlayed(object? sender, NoteEventArgs e) => _dispatcher.BeginInvoke(PulseKeyActivity);

    private void PulseKeyActivity()
    {
        if (_keyActivityTimer == null)
        {
            _keyActivityTimer = new DispatcherTimer(DispatcherPriority.Normal, _dispatcher)
            {
                Interval = TimeSpan.FromMilliseconds(90),
            };
            _keyActivityTimer.Tick += (_, _) =>
            {
                _keyActivityTimer.Stop();
                IsKeyActivity = false;
            };
        }

        IsKeyActivity = true;
        _keyActivityTimer.Stop();   // restart, so a run of notes holds the dot lit
        _keyActivityTimer.Start();
    }
}
