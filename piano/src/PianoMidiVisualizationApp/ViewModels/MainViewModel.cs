using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Globalization;
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

    // ----- Sustain pedal -----

    /// <summary>
    /// CC64 per channel, as the audio engine last heard it. Guarded by <see cref="_soundingLock"/>,
    /// so a note-off and a pedal change reach the engine and the UI in the same order.
    /// </summary>
    private readonly bool[] _pedalDown = new bool[16];

    /// <summary>
    /// UI thread: for each key let go under the pedal and still sounding, the pedal's channel
    /// plus one; 0 for a key that is not ringing. Lifting that pedal releases these keys.
    /// </summary>
    private readonly int[] _ringingChannel = new int[128];

    public PianoKeyboardViewModel PianoKeyboard { get; }
    public SettingsViewModel Settings { get; }
    public ChatViewModel Chat { get; }
    public RecorderViewModel Recorder { get; }
    public SongPracticeViewModel SongPractice { get; }
    public ChordStripViewModel ChordStrip { get; }
    public ProgressionToolsViewModel ProgressionTools { get; }
    public PianoControlsViewModel PianoControls { get; }

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

    /// <summary>
    /// The chord readout: what is sounding, or for a moment after, the last chord that did
    /// (see <see cref="IsReadoutLatched"/>). It is what Space saves.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CurrentChord))]
    [NotifyPropertyChangedFor(nameof(HasChord))]
    private ChordAnalysis _analysis = ChordAnalysis.Empty;

    /// <summary>The chord name alone — what gets saved to the progression and sent to the AI.</summary>
    public string CurrentChord => Analysis.Name;

    public bool HasChord => !Analysis.IsEmpty;

    // ----- Chord readout: live while notes sound, latched for a moment after -----

    /// <summary>How long the readout keeps the last chord once nothing sounds. Style.Readout.Latch fades over the same time.</summary>
    public static readonly TimeSpan LatchDuration = TimeSpan.FromSeconds(2);

    /// <summary>
    /// How long a chord that has only lost notes waits before the readout shows what is left,
    /// counted from the latest note to leave. Two hands never leave the keys at quite the same
    /// moment; without this, letting go of a chord would flicker through its fragments, and a
    /// fragment is what would latch.
    /// </summary>
    public static readonly TimeSpan ReleaseGrace = TimeSpan.FromMilliseconds(150);

    private readonly DispatcherTimer _latchTimer;
    private readonly DispatcherTimer _graceTimer;

    /// <summary>The notes the readout is showing, live or latched, lowest first.</summary>
    private IReadOnlyList<int> _readoutNotes = Array.Empty<int>();

    /// <summary>Whether the readout's chord was saved already, so a second press does not save it twice.</summary>
    private bool _readoutSaved;

    /// <summary>
    /// Nothing is sounding, and the readout is holding the last chord for
    /// <see cref="LatchDuration"/>, dimming as it goes, so it can still be saved.
    /// </summary>
    [ObservableProperty]
    private bool _isReadoutLatched;

    /// <summary>The line under the readout: how to save the chord, or that it was.</summary>
    [ObservableProperty]
    private string _saveHint = "Space to save";

    /// <summary>The empty progression sidebar's line: how a chord gets there, with any learned control.</summary>
    [ObservableProperty]
    private string _progressionEmptyHint = "Hold a chord and press Space to add it here.";

    /// <summary>
    /// The root of what is sounding right now, for the circle of fifths' root outline. Unlike
    /// the readout it never latches: blue on the stage means sounding now.
    /// </summary>
    [ObservableProperty]
    private int? _soundingRootPitchClass;

    // ----- Echo: what an action fired from the piano just did -----

    /// <summary>A few words on the last action fired from the piano, shown briefly over the stage.</summary>
    [ObservableProperty]
    private string _echoText = "";

    /// <summary>Raised on the UI thread each time an echo is shown, even one with the same words as the last.</summary>
    public event EventHandler? EchoShown;

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

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isCircleOfFifthsVisible = true;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isGrandStaffVisible = true;

    /// <summary>
    /// The diatonic chord strip. Shown only while a key is selected and song practice is off;
    /// this flag is the user's choice, which those conditions sit on top of.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isChordStripVisible = true;

    /// <summary>
    /// Song practice: while on, its falling-notes view takes the readout row, and the grand
    /// staff, readout and circle of fifths that live there are hidden.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsZenMode))]
    private bool _isSongPracticeVisible;

    // ----- Grand staff -----

    /// <summary>The held notes spelled for notation, lowest first. Refreshed with the chord readout.</summary>
    [ObservableProperty]
    private IReadOnlyList<SpelledNote> _staffNotes = Array.Empty<SpelledNote>();

    /// <summary>The selected key's signature for the staff; none with no key, as for C major.</summary>
    [ObservableProperty]
    private KeySignature _staffSignature = KeySignature.None;

    /// <summary>
    /// Derived rather than stored, so it can never desync: turning any panel back on
    /// manually leaves zen mode with no extra bookkeeping.
    /// </summary>
    public bool IsZenMode => !IsChatPanelVisible && !IsMidiLogVisible
                          && !IsProgressionVisible && !IsStatusBarVisible
                          && !IsRecorderVisible && !IsCircleOfFifthsVisible
                          && !IsGrandStaffVisible && !IsSongPracticeVisible
                          && !IsChordStripVisible;

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

        SongPractice = new SongPracticeViewModel(PianoKeyboard, PlayNoteOn, PlayNoteOff, status => StatusText = status);
        SongPractice.SongLoaded += (_, _) => IsSongPracticeVisible = true;

        // The strip follows the key, and lights whichever of its chords is sounding.
        ChordStrip = new ChordStripViewModel(PianoKeyboard, PlayNoteOn, PlayNoteOff);
        Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(Settings.CurrentKey))
                ChordStrip.SetKey(Settings.CurrentKey);
        };

        _latchTimer = new DispatcherTimer(LatchDuration, DispatcherPriority.Normal, (_, _) => ClearReadout(), dispatcher);
        _latchTimer.Stop();
        _graceTimer = new DispatcherTimer(ReleaseGrace, DispatcherPriority.Normal, (_, _) => OnReleaseGraceEnded(), dispatcher);
        _graceTimer.Stop();

        ProgressionTools = new ProgressionToolsViewModel(SavedChords, MaxSavedChords, CreateSavedChord,
                                                         Settings, PianoKeyboard, dispatcher,
                                                         PlayNoteOn, PlayNoteOff, status => StatusText = status);

        // Taking a chord out of the progression makes the readout's chord savable again.
        SavedChords.CollectionChanged += (_, e) =>
        {
            if (e.Action != System.Collections.Specialized.NotifyCollectionChangedAction.Add)
                _readoutSaved = false;
            UpdateSaveHint();
        };

        PianoControls = new PianoControlsViewModel(dispatcher);
        PianoControls.ActionFired += OnPianoActionFired;
        PianoControls.TriggerLearned += (_, e) =>
            AppendLog($"Learned {e.Trigger.DisplayName} for {PianoActionInfo.For(e.Action).Label}");
        PianoControls.MappingsChanged += (_, _) => UpdateSaveHint();

        // Closing Settings, by any route, stops Learn listening.
        PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(IsSettingsOverlayVisible) && !IsSettingsOverlayVisible)
                PianoControls.CancelLearning();
        };

        _midiInput.NoteOn += OnMidiNoteOn;
        _midiInput.NoteOff += OnMidiNoteOff;
        _midiInput.MessageReceived += OnRawMidiMessage;
        _midiInput.ControlChange += OnMidiControlChange;
        _midiInput.ProgramChange += OnMidiProgramChange;

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
                RenderReadout();   // the readout re-spells with the new key, latched or not
                RefreshSavedChordFunctions();
            }
            else if (e.PropertyName == nameof(Settings.MuteOutOfKeyNotes))
                UpdateAudiblePitchClasses();
            else if (e.PropertyName is nameof(Settings.MetronomeBpm)
                                    or nameof(Settings.MetronomeBeatsPerBar)
                                    or nameof(Settings.MetronomeVolume))
                ApplyMetronomeSettings();
        };

        InitializeSetupStatus();
    }

    private MusicContext GetMusicContext()
    {
        return new MusicContext(
            WithFunction(CurrentChord, Analysis.Function),
            SavedChords.Select(c => WithFunction(c.ChordName, c.Function)).ToList(),
            MidiLog.TakeLast(10).ToList(),
            Settings.CurrentKey?.DisplayName);

        static string WithFunction(string chord, string function) =>
            string.IsNullOrEmpty(function) ? chord : $"{chord} ({function})";
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

    /// <summary>
    /// Call on the UI thread after any change to what is sounding. The live marks (the circle's
    /// root outline, the chord strip) follow at once. The readout follows new notes at once,
    /// waits out <see cref="ReleaseGrace"/> when notes only leave, and latches the last chord
    /// once nothing sounds.
    /// </summary>
    private void OnSoundingChanged()
    {
        var sounding = SoundingNotes();
        int? root = _analyzer.Analyze(sounding).RootPitchClass;
        SoundingRootPitchClass = root;
        ChordStrip.UpdatePlayed(root, DiatonicChords.MaskOf(sounding));

        if (sounding.Count == 0)
        {
            _graceTimer.Stop();
            if (IsReadoutLatched) return;

            if (Analysis.IsEmpty)
            {
                ClearReadout();   // a lone note: no chord worth keeping
                return;
            }

            IsReadoutLatched = true;
            _latchTimer.Start();
            return;
        }

        // Only notes leaving a chord that is still up: hold it a moment, in case the rest
        // follow. Each further note leaving starts the wait again, so a slow two-handed
        // release still latches the whole chord.
        if (!IsReadoutLatched && sounding.Count < _readoutNotes.Count && !sounding.Except(_readoutNotes).Any())
        {
            _graceTimer.Stop();
            _graceTimer.Start();
            return;
        }

        _graceTimer.Stop();
        ShowReadout(sounding);
    }

    private void OnReleaseGraceEnded()
    {
        _graceTimer.Stop();
        var sounding = SoundingNotes();
        if (sounding.Count > 0)
            ShowReadout(sounding);
    }

    private List<int> SoundingNotes() => PianoKeyboard.GetSoundingNotes().Order().ToList();

    /// <summary>Shows these notes live, ending any latch.</summary>
    private void ShowReadout(IReadOnlyList<int> notes)
    {
        _latchTimer.Stop();
        IsReadoutLatched = false;
        if (notes.SequenceEqual(_readoutNotes)) return;

        _readoutNotes = notes;
        _readoutSaved = false;
        RenderReadout();
    }

    /// <summary>The latch ran out. Cleared before unlatching, so the text never flashes back to full strength.</summary>
    private void ClearReadout()
    {
        _latchTimer.Stop();
        _readoutNotes = Array.Empty<int>();
        _readoutSaved = false;
        RenderReadout();
        IsReadoutLatched = false;
    }

    /// <summary>
    /// Reads the readout's notes into the chord readout and the grand staff. The staff spells
    /// letter-correctly (E#, Cb) where the readout keeps its simpler sharp-or-flat names.
    /// </summary>
    private void RenderReadout()
    {
        Analysis = _analyzer.Analyze(_readoutNotes, UseFlats, Settings.CurrentKey);
        StaffSignature = KeySignature.For(Settings.CurrentKey);
        StaffNotes = NoteSpeller.SpellChord(_readoutNotes, Settings.CurrentKey);
        UpdateSaveHint();
    }

    private void UpdateSaveHint()
    {
        string saveKeys = PianoControls.TriggerFor(PianoAction.SaveChord) is { } trigger
            ? $"Space or {trigger.DisplayName}"
            : "Space";

        SaveHint = _readoutSaved ? $"Saved · {SavedChords.Count}/{MaxSavedChords}"
                 : SavedChords.Count >= MaxSavedChords ? $"Progression full · {MaxSavedChords}/{MaxSavedChords}"
                 : $"{saveKeys} to save";
        ProgressionEmptyHint = $"Hold a chord and press {saveKeys} to add it here.";
    }

    /// <summary>Re-reads every saved chord's numeral in the current key. At most eight chords, all cached.</summary>
    private void RefreshSavedChordFunctions()
    {
        foreach (var chord in SavedChords)
        {
            var analysis = _analyzer.Analyze(chord.NoteNumbers, UseFlats, Settings.CurrentKey);
            chord.Function = analysis.Function;
            chord.FunctionKind = analysis.FunctionKind;
        }
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

        TryConnectMidi(Settings.SelectedMidiDevice, "MIDI connected");
    }

    [RelayCommand]
    private void DisconnectMidi()
    {
        _midiInput.Close();
        ResetSustainPedals();
        IsMidiConnected = false;
        StatusText = "MIDI disconnected";
    }

    [RelayCommand]
    private void StartAudio()
    {
        if (string.IsNullOrEmpty(Settings.SoundFontPath))
        {
            StatusText = "Choose a piano sound (.sf2 or .sfz) first";
            return;
        }

        MatchAudioModeToDriver();
        if (string.IsNullOrEmpty(Settings.SelectedAudioDriver))
        {
            SetAudioProblem("No audio output found", "No audio output found. Connect speakers or headphones.");
            StatusText = "No audio output found";
            return;
        }

        TryStartAudio("Audio started");
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

    /// <summary>
    /// Saves the readout's chord: the one sounding, or the one it is still holding after the
    /// keys came up. A miss is echoed on the stage, since the status bar may be hidden.
    /// </summary>
    [RelayCommand]
    private void SaveCurrentChord()
    {
        if (!TrySaveReadoutChord(out string result))
            ShowEcho(result);
    }

    /// <param name="result">What happened, in a few words, for the stage echo.</param>
    private bool TrySaveReadoutChord(out string result)
    {
        if (Analysis.IsEmpty || _readoutNotes.Count == 0)
        {
            StatusText = "No chord to save - play one first";
            result = "Play a chord first";
            return false;
        }

        if (_readoutSaved)
        {
            StatusText = result = $"{CurrentChord} is already saved";
            return false;
        }

        if (SavedChords.Count >= MaxSavedChords)
        {
            StatusText = "Maximum 8 chords saved - remove one first";
            result = "Progression full · remove a chord first";
            return false;
        }

        var savedChord = CreateSavedChord(_readoutNotes);
        _readoutSaved = true;
        SavedChords.Add(savedChord);
        StatusText = $"Saved chord: {savedChord.ChordName}";
        result = $"Saved {savedChord.ChordName} · {SavedChords.Count}/{MaxSavedChords}";
        return true;
    }

    /// <summary>
    /// A saved chord for these notes: named, spelled and given its numeral in the current key.
    /// The one way chords enter the progression, whether saved from the keyboard, transposed or
    /// picked from the suggestions, so they all read alike.
    /// </summary>
    private SavedChord CreateSavedChord(IEnumerable<int> notes)
    {
        var sorted = notes.Distinct().OrderBy(n => n).ToList();
        var analysis = _analyzer.Analyze(sorted, UseFlats, Settings.CurrentKey);

        return new SavedChord
        {
            ChordName = analysis.Name,
            NoteNumbers = sorted,
            NoteNames = sorted.Select(n => MusicNaming.WithOctave(n, UseFlats)).ToList(),
            Function = analysis.Function,
            FunctionKind = analysis.FunctionKind,
        };
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

    [RelayCommand]
    private void TapMetronome() => Tap(_tapClock.Elapsed);

    /// <summary>
    /// Sets the tempo from the average of the last few taps; the first tap only starts the
    /// count. Returns the tempo set, after clamping, or null for a first tap.
    /// </summary>
    private int? Tap(TimeSpan at)
    {
        if (_tapTempo.Tap(at) is not { } bpm) return null;
        Settings.MetronomeBpm = bpm;
        return Settings.MetronomeBpm;
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

    [RelayCommand]
    private void ToggleCircleOfFifths() => IsCircleOfFifthsVisible = !IsCircleOfFifthsVisible;

    [RelayCommand]
    private void ToggleGrandStaff() => IsGrandStaffVisible = !IsGrandStaffVisible;

    [RelayCommand]
    private void ToggleSongPractice() => IsSongPracticeVisible = !IsSongPracticeVisible;

    [RelayCommand]
    private void ToggleChordStrip() => IsChordStripVisible = !IsChordStripVisible;

    /// <summary>Hiding the practice view pauses the song rather than letting it play on unseen.</summary>
    partial void OnIsSongPracticeVisibleChanged(bool value) => SongPractice.IsActive = value;

    private readonly record struct PanelLayout(bool Chat, bool MidiLog, bool Progression, bool StatusBar,
                                               bool Recorder, bool CircleOfFifths, bool GrandStaff,
                                               bool SongPractice, bool ChordStrip);

    private PanelLayout? _preZenLayout;

    [RelayCommand]
    private void ToggleZenMode()
    {
        if (IsZenMode)
        {
            // Nothing was saved if the app started in zen — restore a sensible layout instead.
            var restore = _preZenLayout ?? new PanelLayout(false, false, true, true, false, true, true, false, true);
            IsChatPanelVisible = restore.Chat;
            IsMidiLogVisible = restore.MidiLog;
            IsProgressionVisible = restore.Progression;
            IsStatusBarVisible = restore.StatusBar;
            IsRecorderVisible = restore.Recorder;
            IsCircleOfFifthsVisible = restore.CircleOfFifths;
            IsGrandStaffVisible = restore.GrandStaff;
            IsSongPracticeVisible = restore.SongPractice;
            IsChordStripVisible = restore.ChordStrip;
        }
        else
        {
            _preZenLayout = new PanelLayout(
                IsChatPanelVisible, IsMidiLogVisible, IsProgressionVisible, IsStatusBarVisible,
                IsRecorderVisible, IsCircleOfFifthsVisible, IsGrandStaffVisible, IsSongPracticeVisible,
                IsChordStripVisible);
            IsChatPanelVisible = IsMidiLogVisible = IsProgressionVisible = IsStatusBarVisible = false;
            IsRecorderVisible = IsCircleOfFifthsVisible = IsGrandStaffVisible = false;
            IsSongPracticeVisible = IsChordStripVisible = false;
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
        IsCircleOfFifthsVisible = saved.ShowCircleOfFifths;
        IsGrandStaffVisible = saved.ShowGrandStaff;
        IsChordStripVisible = saved.ShowChordStrip;
        IsSetupChecklistDismissed = saved.HideSetupChecklist;
        ChordStrip.ShowSevenths = saved.ChordStripSevenths;
        PianoKeyboard.ShowAllNoteNames = saved.ShowAllNoteNames;
        SongPractice.Speed = saved.SongSpeed;
        SongPractice.Mode = Enum.TryParse<Services.SongPractice.PracticeMode>(saved.SongMode, out var mode)
                            && Enum.IsDefined(mode) && !int.TryParse(saved.SongMode, out _)
            ? mode
            : Services.SongPractice.PracticeMode.Wait;
        // The last song reopens only if practice was on; a file since moved just leaves the
        // empty "open a song" state rather than an error.
        if (saved.ShowSongPractice && !string.IsNullOrEmpty(saved.SongPath) && System.IO.File.Exists(saved.SongPath))
            SongPractice.LoadSong(saved.SongPath, showTrackList: false);
        IsSongPracticeVisible = saved.ShowSongPractice;
        ProgressionTools.ApplyFrom(saved);
        PianoControls.ApplyFrom(saved);
        UpdateSaveHint();
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
        saved.ShowCircleOfFifths = IsCircleOfFifthsVisible;
        saved.ShowGrandStaff = IsGrandStaffVisible;
        saved.ShowSongPractice = IsSongPracticeVisible;
        saved.ShowChordStrip = IsChordStripVisible;
        saved.HideSetupChecklist = IsSetupChecklistDismissed;
        saved.ChordStripSevenths = ChordStrip.ShowSevenths;
        saved.ShowAllNoteNames = PianoKeyboard.ShowAllNoteNames;
        saved.SongPath = SongPractice.SongPath;
        saved.SongSpeed = SongPractice.Speed;
        saved.SongMode = SongPractice.Mode.ToString();
        ProgressionTools.CaptureInto(saved);
        PianoControls.CaptureInto(saved);
        return saved;
    }

    public void AutoConnect()
    {
        // Auto-connect MIDI if a device is selected
        if (Settings.SelectedMidiDevice != null && !IsMidiConnected)
            TryConnectMidi(Settings.SelectedMidiDevice, "MIDI auto-connected");

        // Before anything reads the mode: the setup checklist names it too.
        MatchAudioModeToDriver();

        // Auto-start audio if we have a SoundFont and audio driver
        if (!string.IsNullOrEmpty(Settings.SoundFontPath)
            && System.IO.File.Exists(Settings.SoundFontPath)
            && !string.IsNullOrEmpty(Settings.SelectedAudioDriver)
            && !IsAudioRunning)
            TryStartAudio("Audio auto-started");

        // Only now can the setup checklist tell "not set up" from "not reconnected yet".
        HasAttemptedAutoConnect = true;
    }

    private void OnRawMidiMessage(object? sender, RawMidiMessageEventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            // Not the status bar: a message per note there buried every status ("Saved chord: G7",
            // "Audio stopped") within a keypress. The MIDI log and the activity dot carry them.
            MidiActivity = true;
            LastMidiMessage = e.Description;
            AppendLog(e.Description);

            // Turn off the activity light after 80ms
            _activityTimer?.Dispose();
            _activityTimer = new System.Threading.Timer(_ =>
            {
                _dispatcher.BeginInvoke(() => MidiActivity = false);
            }, null, 80, System.Threading.Timeout.Infinite);
        });
    }

    /// <summary>UI thread. Adds a line to the MIDI log, dropping the oldest past <see cref="MaxLogLines"/>.</summary>
    private void AppendLog(string line)
    {
        // Invariant: a format's ":" is the culture's time separator, so Danish Windows printed
        // "22.06.13.526", the seconds indistinguishable from the milliseconds.
        MidiLog.Add($"[{DateTime.Now.ToString("HH:mm:ss.fff", CultureInfo.InvariantCulture)}] {line}");
        while (MidiLog.Count > MaxLogLines)
            MidiLog.RemoveAt(0);
    }

    private void OnMidiNoteOn(object? sender, NoteEventArgs e)
    {
        // Timestamped first, before the audio engine or anything else can delay it.
        long timestamp = Stopwatch.GetTimestamp();

        // A learned control belongs to the app, not the piano: it never sounds, lights a key,
        // reaches the readout or gets recorded or scored.
        if (PianoControls.Router.OnNoteOn(e.Channel, e.NoteNumber, timestamp)) return;

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
                if (IsNote(e.NoteNumber))
                {
                    _liveHeld[e.NoteNumber] = true;
                    _ringingChannel[e.NoteNumber] = 0;
                }
                PianoKeyboard.SetKeyPressed(e.NoteNumber, e.Velocity);
                OnSoundingChanged();
                SongPractice.OnLiveNoteOn(e.NoteNumber);
            });
        }
    }

    private void OnMidiNoteOff(object? sender, NoteEventArgs e)
    {
        long timestamp = Stopwatch.GetTimestamp();
        if (PianoControls.Router.OnNoteOff(e.Channel, e.NoteNumber)) return;
        Recorder.CaptureNoteOff(e.NoteNumber, timestamp);

        lock (_soundingLock)
        {
            bool appHoldsKey = false;
            bool pedalHolds = false;
            if (TryGetSlot(e.Channel, e.NoteNumber, out int slot))
            {
                _liveSounding[slot] = false;
                appHoldsKey = e.Channel == AppChannel && _appSounding[e.NoteNumber] > 0;
                pedalHolds = _pedalDown[e.Channel];
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
                SongPractice.OnLiveNoteOff(e.NoteNumber);
                if (IsNote(e.NoteNumber))
                {
                    _liveHeld[e.NoteNumber] = false;
                    if (_appHeld[e.NoteNumber] > 0) return;   // still held by an app note
                }
                ReleaseKey(e.NoteNumber, pedalHolds ? e.Channel : -1);
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
                _ringingChannel[note] = 0;
                PianoKeyboard.SetKeyPressed(note, velocity);
                OnSoundingChanged();
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

            // The pedal is the piano's, so it holds the app's notes as it would any other.
            bool pedalHolds = _pedalDown[AppChannel];
            _dispatcher.BeginInvoke(() =>
            {
                if (_appHeld[note] > 0) _appHeld[note]--;
                if (_appHeld[note] > 0 || _liveHeld[note]) return;
                ReleaseKey(note, pedalHolds ? AppChannel : -1);
            });
        }
    }

    /// <summary>
    /// UI thread: nothing holds this key down any more. It rings on, dimmer, under the pedal on
    /// <paramref name="pedalChannel"/>, or with -1 goes dark.
    /// </summary>
    private void ReleaseKey(int note, int pedalChannel)
    {
        if (pedalChannel >= 0 && IsNote(note))
        {
            _ringingChannel[note] = pedalChannel + 1;
            PianoKeyboard.SetKeySustained(note);
        }
        else
        {
            if (IsNote(note)) _ringingChannel[note] = 0;
            PianoKeyboard.SetKeyReleased(note);
        }

        OnSoundingChanged();
    }

    // ----- Sustain pedal and other controllers -----

    private void OnMidiControlChange(object? sender, ControlChangeEventArgs e)
    {
        long timestamp = Stopwatch.GetTimestamp();
        if (PianoControls.Router.OnControlChange(e.Channel, e.Controller, e.Value, timestamp)) return;

        if (e.Controller == MidiTrigger.SustainPedal)
            SetSustainPedal(e.Channel, e.Value >= MidiTrigger.PressedThreshold, timestamp);
    }

    /// <summary>Program changes only ever drive learned controls; the piano sound never changes.</summary>
    private void OnMidiProgramChange(object? sender, ProgramChangeEventArgs e) =>
        PianoControls.Router.OnProgramChange(e.Channel, e.Program, Stopwatch.GetTimestamp());

    /// <summary>
    /// The sustain pedal on one channel, from any thread. The engine sustains the sound, the
    /// recorder the note lengths, and the keys it holds stay lit, dimmer, until it lifts.
    /// </summary>
    private void SetSustainPedal(int channel, bool isDown, long timestamp)
    {
        if (channel is < 0 or > 15) return;

        lock (_soundingLock)
        {
            if (_pedalDown[channel] == isDown) return;
            _pedalDown[channel] = isDown;

            _audioEngine.SetSustainPedal(channel, isDown);
            Recorder.CaptureSustainPedal(Array.IndexOf(_pedalDown, true) >= 0, timestamp);

            if (!isDown)
                _dispatcher.BeginInvoke(() => ReleaseRinging(channel));
        }
    }

    /// <summary>UI thread: the pedal on this channel lifted, so every key it was holding goes dark.</summary>
    private void ReleaseRinging(int channel)
    {
        bool released = false;
        for (int note = 0; note < _ringingChannel.Length; note++)
        {
            if (_ringingChannel[note] != channel + 1) continue;
            _ringingChannel[note] = 0;
            PianoKeyboard.SetKeyReleased(note);
            released = true;
        }

        if (released) OnSoundingChanged();
    }

    /// <summary>
    /// Lifts every pedal. For when the keyboard goes away or the engine restarts, since the
    /// pedal-up that would release its notes may never arrive.
    /// </summary>
    private void ResetSustainPedals()
    {
        long timestamp = Stopwatch.GetTimestamp();
        for (int channel = 0; channel < _pedalDown.Length; channel++)
            SetSustainPedal(channel, false, timestamp);
    }

    // ----- Actions fired from the piano -----

    private void OnPianoActionFired(object? sender, PianoActionFiredEventArgs e)
    {
        AppendLog($"{e.Trigger.DisplayName} → {PianoActionInfo.For(e.Action).Label}");
        ShowEcho(RunPianoAction(e.Action, e.Timestamp));
    }

    /// <summary>Runs an action for a learned control. Returns what happened, in a few words, for the echo.</summary>
    private string RunPianoAction(PianoAction action, long timestamp)
    {
        switch (action)
        {
            case PianoAction.SaveChord:
                TrySaveReadoutChord(out string saved);
                return saved;

            case PianoAction.ToggleProgressionPlayback:
                if (SavedChords.Count == 0) return "No chords saved yet";
                ProgressionTools.TogglePlaybackCommand.Execute(null);
                return ProgressionTools.IsPlaying ? "Progression playing" : "Progression stopped";

            case PianoAction.ToggleRecord:
            {
                var newest = Recorder.Takes.FirstOrDefault();
                Recorder.ToggleRecordCommand.Execute(null);
                if (Recorder.IsRecordArmed) return "Recording armed · starts at your first note";

                var take = Recorder.Takes.FirstOrDefault();
                return take != null && take != newest
                    ? $"{take.Name} recorded · {take.LengthText}"
                    : "Recording cancelled · no notes played";
            }

            case PianoAction.ToggleTakePlayback:
                if (Recorder.Takes.Count == 0) return "No takes yet · record one first";
                Recorder.TogglePlaybackCommand.Execute(null);
                return Recorder.IsPlaying ? $"Playing {Recorder.SelectedTake?.Name}" : "Take stopped";

            case PianoAction.ToggleMetronome:
                ToggleMetronome();
                return !IsMetronomeOn ? "Metronome off"
                     : IsAudioRunning ? $"Metronome on · {Settings.MetronomeBpm} BPM"
                     : "Metronome on · start audio to hear it";

            case PianoAction.TapTempo:
                // Timed from the press itself, not from whenever the UI thread got to it.
                var at = _tapClock.Elapsed - Stopwatch.GetElapsedTime(timestamp);
                return Tap(at) is { } bpm ? $"Tempo {bpm} BPM" : "Tap tempo · keep tapping";

            case PianoAction.ToggleSongPlayback:
                if (!SongPractice.HasSong) return "No song open";
                IsSongPracticeVisible = true;
                SongPractice.TogglePlayCommand.Execute(null);
                return SongPractice.IsPlaying ? "Song playing" : "Song paused";

            case PianoAction.RestartSong:
                if (!SongPractice.HasSong) return "No song open";
                IsSongPracticeVisible = true;
                SongPractice.RestartCommand.Execute(null);
                return "Song restarted";

            default:
                return "";
        }
    }

    private void ShowEcho(string text)
    {
        if (string.IsNullOrEmpty(text)) return;
        EchoText = text;
        EchoShown?.Invoke(this, EventArgs.Empty);
    }

    private static bool IsNote(int note) => note is >= 0 and <= 127;

    private static bool TryGetSlot(int channel, int note, out int slot)
    {
        slot = channel * 128 + note;
        return channel is >= 0 and <= 15 && IsNote(note);
    }

    public void Dispose()
    {
        // First, while the engine can still take the note-offs the players send on the way out.
        Recorder.Dispose();
        SongPractice.Dispose();
        ChordStrip.Dispose();
        ProgressionTools.Dispose();
        _activityTimer?.Dispose();
        DisposeSetupStatus();
        _midiInput.NoteOn -= OnMidiNoteOn;
        _midiInput.NoteOff -= OnMidiNoteOff;
        _midiInput.MessageReceived -= OnRawMidiMessage;
        _midiInput.ControlChange -= OnMidiControlChange;
        _midiInput.ProgramChange -= OnMidiProgramChange;
        _latchTimer.Stop();
        _graceTimer.Stop();
        if (_audioEngine.Metronome is { } metronome)
            metronome.BeatStarted -= OnMetronomeBeat;
        _audioEngine.Stop();
        _audioEngine.Dispose();
        _midiInput.Close();
        _midiInput.Dispose();
    }
}
