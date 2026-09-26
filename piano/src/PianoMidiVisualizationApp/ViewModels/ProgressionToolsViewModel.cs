using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;
using PianoMidiVisualizationApp.Services.Playback;
using PianoMidiVisualizationApp.Services.Progression;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>One entry in the chord-length dropdown. Zero beats means one bar of the metronome.</summary>
public record ChordLengthOption(int Beats, string Name);

/// <summary>
/// A next-chord idea as a sidebar chip. <see cref="Function"/> and <see cref="FunctionKind"/> are
/// named as on <see cref="SavedChord"/>, so the shared numeral style colours both alike.
/// </summary>
/// <param name="ChordName">The chord in root position, e.g. "F".</param>
/// <param name="Notes">The voicing that clicking adds and hovering hints: led smoothly from the last chord.</param>
/// <param name="VoicingText">The voicing in words, for the tooltip, e.g. "F/C  C4 F4 A4".</param>
public sealed record ChordSuggestionItem(
    string Function, RomanNumeralKind FunctionKind, string ChordName,
    IReadOnlyList<int> Notes, string VoicingText);

/// <summary>
/// The tools under the saved chord progression: play it back at the metronome's tempo, loop
/// it, transpose it by semitones, and suggest what could come next.
///
/// <para>Playback goes out through the host's app-note path, so the chords sound (past the
/// out-of-key mute), light the keyboard and feed the readout, but are never recorded. It keeps
/// its own clock at the metronome's tempo and leaves the metronome itself alone: when that is
/// on, it simply clicks along.</para>
/// </summary>
public partial class ProgressionToolsViewModel : ObservableObject, IDisposable
{
    /// <summary>An 88-key piano's range; transposition stops rather than push a note past it.</summary>
    private const int LowestNote = 21, HighestNote = 108;

    private const int PlaybackVelocity = 80;

    private readonly ObservableCollection<SavedChord> _chords;
    private readonly SettingsViewModel _settings;
    private readonly PianoKeyboardViewModel _keyboard;
    private readonly Dispatcher _dispatcher;
    private readonly Func<IEnumerable<int>, SavedChord> _createChord;
    private readonly int _maxChords;
    private readonly Action<string> _reportStatus;
    private readonly NoteSequencePlayer _player;
    private readonly ChordAnalyzer _analyzer = new();

    // What the player was last loaded with, to map its position back onto a chord.
    private TimeSpan _loadedChordLength = TimeSpan.FromSeconds(1);
    private int _loadedChordCount;
    private int _loadedBpm;

    /// <summary>The chord index last sent to the UI, so the timing thread posts only changes.</summary>
    private int _postedChordIndex = -1;

    private bool _resumePending;

    /// <summary>The keyboard's hints from before a suggestion was hovered, to put back after.</summary>
    private List<int>? _hintsBeforePreview;

    public IReadOnlyList<ChordLengthOption> ChordLengths { get; } =
    [
        new(0, "1 bar"),
        new(1, "1 beat"),
        new(2, "2 beats"),
        new(3, "3 beats"),
        new(4, "4 beats"),
        new(6, "6 beats"),
        new(8, "8 beats"),
    ];

    [ObservableProperty]
    private bool _isPlaying;

    [ObservableProperty]
    private bool _isLooping = true;

    /// <summary>How long each chord sounds, in metronome beats; 0 for one bar.</summary>
    [ObservableProperty]
    private int _beatsPerChord;

    /// <summary>Whether transposing also moves the selected key, so the numerals stay put.</summary>
    [ObservableProperty]
    private bool _transposeMovesKey = true;

    /// <summary>Suggestions need a key to reason in, so the whole section hides without one.</summary>
    [ObservableProperty]
    private bool _showSuggestions;

    /// <summary>"After V", or a prompt while there is no chord to continue from.</summary>
    [ObservableProperty]
    private string _suggestionsHeader = "";

    public ObservableCollection<ChordSuggestionItem> Suggestions { get; } = new();

    public bool HasChords => _chords.Count > 0;

    /// <param name="chords">The saved progression, owned by the host.</param>
    /// <param name="createChord">
    /// Builds a saved chord from notes exactly as saving from the keyboard does: named, spelled
    /// and given its numeral in the current key.
    /// </param>
    /// <param name="noteOn">The host's app-note path: sounds and lights a note, never records it.</param>
    /// <param name="noteOff">Its release counterpart.</param>
    /// <param name="reportStatus">Shows a message in the status bar; called on the UI thread.</param>
    public ProgressionToolsViewModel(
        ObservableCollection<SavedChord> chords, int maxChords, Func<IEnumerable<int>, SavedChord> createChord,
        SettingsViewModel settings, PianoKeyboardViewModel keyboard, Dispatcher dispatcher,
        Action<int, int> noteOn, Action<int> noteOff, Action<string> reportStatus)
    {
        _chords = chords;
        _maxChords = maxChords;
        _createChord = createChord;
        _settings = settings;
        _keyboard = keyboard;
        _dispatcher = dispatcher;
        _reportStatus = reportStatus;

        _player = new NoteSequencePlayer(noteOn, noteOff) { IsLooping = IsLooping };
        _player.PositionChanged += OnPlayerPositionChanged;
        _player.PlaybackStopped += OnPlayerStopped;

        _chords.CollectionChanged += OnChordsChanged;
        _settings.PropertyChanged += OnSettingsChanged;

        RefreshSuggestions();
    }

    // ----- Playback -----

    [RelayCommand]
    private void TogglePlayback()
    {
        if (IsPlaying)
        {
            _player.Stop();
            SetPlaying(false);
            _reportStatus("Progression stopped");
            return;
        }

        if (_chords.Count == 0)
        {
            _reportStatus("Save a few chords first (Space), then play them back");
            return;
        }

        StartAt(0);
        _reportStatus($"Playing the progression at {_loadedBpm} BPM, "
                      + $"{ChordLengths.First(o => o.Beats == BeatsPerChord).Name} per chord (Ctrl+P to stop)");
    }

    /// <summary>(Re)loads the progression as it is now and plays from the start of a chord.</summary>
    private void StartAt(int chordIndex)
    {
        int bpm = _settings.MetronomeBpm;
        int beats = BeatsPerChord > 0 ? BeatsPerChord : _settings.MetronomeBeatsPerBar;
        var length = TimeSpan.FromSeconds(beats * 60.0 / bpm);

        // Each chord lets go a moment early, so a note it shares with the next one re-strikes
        // instead of blurring into a single held note.
        var gap = TimeSpan.FromMilliseconds(Math.Min(60, length.TotalMilliseconds * 0.1));

        var notes = new List<TimedNote>();
        for (int i = 0; i < _chords.Count; i++)
            foreach (int note in _chords[i].NoteNumbers)
                notes.Add(new TimedNote(length * i, length - gap, note, PlaybackVelocity));

        _player.Load(notes, length * _chords.Count);
        _player.IsLooping = IsLooping;
        _player.Speed = 1.0;
        _loadedChordLength = length;
        _loadedChordCount = _chords.Count;
        _loadedBpm = bpm;

        int start = Math.Clamp(chordIndex, 0, _chords.Count - 1);
        Interlocked.Exchange(ref _postedChordIndex, start);
        _player.Play(length * start);

        SetPlaying(true);
        Highlight(start);
    }

    /// <summary>
    /// Picks up an edit made mid-playback: reloads and carries on from the chord that was
    /// sounding, which re-strikes it in its new form. Coalesced, so transposing all eight chords
    /// reloads once rather than eight times.
    /// </summary>
    private void ScheduleResume()
    {
        if (_resumePending) return;
        _resumePending = true;
        _dispatcher.BeginInvoke(() =>
        {
            _resumePending = false;
            if (!IsPlaying) return;

            if (_chords.Count == 0)
            {
                _player.Stop();
                SetPlaying(false);
                return;
            }

            StartAt(CurrentChordIndex());
        });
    }

    private int CurrentChordIndex() =>
        (int)(_player.Position.Ticks / Math.Max(1, _loadedChordLength.Ticks));

    private void SetPlaying(bool playing)
    {
        IsPlaying = playing;
        if (!playing) Highlight(-1);
    }

    private void Highlight(int index)
    {
        for (int i = 0; i < _chords.Count; i++)
            _chords[i].IsSounding = i == index;
    }

    /// <summary>On the timing thread, ~30 times a second: posts only when the chord changes.</summary>
    private void OnPlayerPositionChanged(object? sender, PlaybackPositionEventArgs e)
    {
        int count = _loadedChordCount;
        if (count == 0) return;

        int index = Math.Clamp((int)(e.Position.Ticks / Math.Max(1, _loadedChordLength.Ticks)), 0, count - 1);
        if (Interlocked.Exchange(ref _postedChordIndex, index) == index) return;

        _dispatcher.BeginInvoke(() =>
        {
            // A late update from a run that has since stopped must not light a chord.
            if (IsPlaying) Highlight(index);
        });
    }

    private void OnPlayerStopped(object? sender, EventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            // Re-read rather than assume: a reload raises this too, and has already restarted.
            if (!_player.IsPlaying) SetPlaying(false);
        });
    }

    partial void OnIsLoopingChanged(bool value) => _player.IsLooping = value;

    partial void OnBeatsPerChordChanged(int value)
    {
        if (IsPlaying) ScheduleResume();
    }

    /// <summary>
    /// Follows a tempo change by speeding the player up or down, which keeps the chord that is
    /// sounding; only a change beyond the player's speed range needs a reload.
    /// </summary>
    private void ApplyTempo()
    {
        double ratio = (double)_settings.MetronomeBpm / Math.Max(1, _loadedBpm);
        if (ratio is >= NoteSequencePlayer.MinimumSpeed and <= NoteSequencePlayer.MaximumSpeed)
            _player.Speed = ratio;
        else
            ScheduleResume();
    }

    // ----- Transpose -----

    [RelayCommand]
    private void TransposeUp() => Transpose(+1);

    [RelayCommand]
    private void TransposeDown() => Transpose(-1);

    private void Transpose(int semitones)
    {
        if (_chords.Count == 0)
        {
            _reportStatus("Nothing to transpose yet");
            return;
        }

        if (_chords.SelectMany(c => c.NoteNumbers).Any(n => n + semitones is < LowestNote or > HighestNote))
        {
            _reportStatus($"Can't transpose {(semitones > 0 ? "up" : "down")}: a note would leave the piano's range");
            return;
        }

        // The key moves first, so the chords below are named, spelled and numbered in it.
        string keyNote = "";
        if (TransposeMovesKey && _settings.KeyTonicPitchClass is { } tonic)
        {
            _settings.KeyTonicPitchClass = MusicNaming.PitchClassOf(tonic + semitones);
            keyNote = $"; key is now {_settings.CurrentKey?.DisplayName}";
        }

        // Rebuilt rather than shifted in place, so everything derived from the notes is
        // re-analysed exactly as if the chords had been played and saved this way.
        for (int i = 0; i < _chords.Count; i++)
            _chords[i] = _createChord(_chords[i].NoteNumbers.Select(n => n + semitones));

        _reportStatus($"Transposed {(semitones > 0 ? "up" : "down")} a semitone{keyNote}");
    }

    // ----- Suggestions -----

    private void RefreshSuggestions()
    {
        // The chips are about to be replaced, possibly from under the pointer.
        EndSuggestionPreview();
        Suggestions.Clear();

        var key = _settings.CurrentKey;
        ShowSuggestions = key is not null;
        if (key is not { } k)
        {
            SuggestionsHeader = "";
            return;
        }

        if (_chords.Count == 0)
        {
            SuggestionsHeader = "Save a chord to see what could follow";
            return;
        }

        bool flats = k.UsesFlats;
        var last = _chords[^1];
        var lastAnalysis = _analyzer.Analyze(last.NoteNumbers, flats, k);
        SuggestionsHeader = $"After {(string.IsNullOrEmpty(lastAnalysis.Function) ? lastAnalysis.Name : lastAnalysis.Function)}";

        foreach (var suggestion in NextChordSuggester.Suggest(last.NoteNumbers, k).Suggestions)
        {
            // Named and numbered in root position, the way a chord is thought of; the voicing,
            // usually an inversion, is what the hint shows and the tooltip spells out.
            var pitchClasses = suggestion.Function.PitchClasses(k.Tonic);
            int root = 48 + pitchClasses[0];
            var rootPosition = pitchClasses.Select(pc => root + MusicNaming.PitchClassOf(pc - pitchClasses[0]));
            var plain = _analyzer.Analyze(rootPosition, flats, k);
            var voiced = _analyzer.Analyze(suggestion.Notes, flats, k);

            string notes = string.Join(" ", suggestion.Notes.Select(n => MusicNaming.WithOctave(n, flats)));
            Suggestions.Add(new ChordSuggestionItem(
                plain.Function, plain.FunctionKind, plain.Name, suggestion.Notes, $"{voiced.Name}  {notes}"));
        }
    }

    /// <summary>Hovering a suggestion shows its voicing on the keyboard.</summary>
    public void PreviewSuggestion(ChordSuggestionItem item)
    {
        _hintsBeforePreview ??= _keyboard.Keys.Where(k => k.IsHinted).Select(k => k.NoteNumber).ToList();
        _keyboard.SetHintedNotes(item.Notes);
    }

    /// <summary>
    /// Takes the preview down again. Anything song practice was hinting before the hover comes
    /// back rather than being wiped along with it.
    /// </summary>
    public void EndSuggestionPreview()
    {
        if (_hintsBeforePreview is not { } previous) return;
        _hintsBeforePreview = null;

        if (previous.Count == 0) _keyboard.ClearHints();
        else _keyboard.SetHintedNotes(previous);
    }

    [RelayCommand]
    private void AddSuggestion(ChordSuggestionItem? item)
    {
        if (item is null) return;

        if (_chords.Count >= _maxChords)
        {
            _reportStatus($"Maximum {_maxChords} chords saved - remove one first");
            return;
        }

        var chord = _createChord(item.Notes);
        _chords.Add(chord);
        _reportStatus($"Added {chord.ChordName}");
    }

    // ----- Host wiring -----

    private void OnChordsChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        OnPropertyChanged(nameof(HasChords));
        RefreshSuggestions();
        if (IsPlaying) ScheduleResume();
    }

    private void OnSettingsChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(SettingsViewModel.KeyTonicPitchClass):
            case nameof(SettingsViewModel.KeyScale):
                RefreshSuggestions();
                break;
            case nameof(SettingsViewModel.MetronomeBpm) when IsPlaying:
                ApplyTempo();
                break;
            case nameof(SettingsViewModel.MetronomeBeatsPerBar) when IsPlaying && BeatsPerChord == 0:
                ScheduleResume();
                break;
        }
    }

    public void ApplyFrom(AppSettings saved)
    {
        IsLooping = saved.ProgressionLoop;
        BeatsPerChord = ChordLengths.Any(o => o.Beats == saved.ProgressionBeatsPerChord)
            ? saved.ProgressionBeatsPerChord
            : 0;
        TransposeMovesKey = saved.TransposeMovesKey;
    }

    public void CaptureInto(AppSettings saved)
    {
        saved.ProgressionLoop = IsLooping;
        saved.ProgressionBeatsPerChord = BeatsPerChord;
        saved.TransposeMovesKey = TransposeMovesKey;
    }

    public void Dispose()
    {
        _chords.CollectionChanged -= OnChordsChanged;
        _settings.PropertyChanged -= OnSettingsChanged;
        _player.PositionChanged -= OnPlayerPositionChanged;
        _player.PlaybackStopped -= OnPlayerStopped;
        _player.Dispose();
    }
}
