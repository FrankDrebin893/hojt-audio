using System.Collections.ObjectModel;
using System.Windows;
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
    private readonly ChordDetector _chordDetector = new();
    private System.Threading.Timer? _activityTimer;

    public PianoKeyboardViewModel PianoKeyboard { get; }
    public SettingsViewModel Settings { get; }

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
    private string _currentChord = "";

    private const int MaxLogLines = 100;
    public ObservableCollection<string> MidiLog { get; } = new();

    public MainViewModel(IMidiInputService midiInput, IAudioEngine audioEngine, Dispatcher dispatcher)
    {
        _midiInput = midiInput;
        _audioEngine = audioEngine;
        _dispatcher = dispatcher;

        PianoKeyboard = new PianoKeyboardViewModel();
        Settings = new SettingsViewModel();

        _midiInput.NoteOn += OnMidiNoteOn;
        _midiInput.NoteOff += OnMidiNoteOff;
        _midiInput.MessageReceived += OnRawMidiMessage;

        Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(Settings.Volume))
                _audioEngine.Volume = Settings.Volume;
        };
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
            Filter = "SoundFont files (*.sf2)|*.sf2|All files (*.*)|*.*",
            Title = "Select SoundFont File"
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
            CurrentChord = _chordDetector.Detect(PianoKeyboard.GetPressedNotes()) ?? "";
        });
    }

    private void OnMidiNoteOff(object? sender, NoteEventArgs e)
    {
        _audioEngine.NoteOff(e.Channel, e.NoteNumber);

        _dispatcher.BeginInvoke(() =>
        {
            PianoKeyboard.SetKeyReleased(e.NoteNumber);
            CurrentChord = _chordDetector.Detect(PianoKeyboard.GetPressedNotes()) ?? "";
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
