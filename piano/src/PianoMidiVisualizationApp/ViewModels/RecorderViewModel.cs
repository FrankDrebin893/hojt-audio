using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using PianoMidiVisualizationApp.Services.Playback;
using PianoMidiVisualizationApp.Services.Recording;

namespace PianoMidiVisualizationApp.ViewModels;

public enum RecordState { Idle, WaitingForFirstNote, Recording }

/// <summary>One entry in the playback-speed dropdown.</summary>
public record SpeedOption(double Factor, string Name);

/// <summary>
/// The recorder transport and the takes behind it: record, play back, loop an A–B region,
/// change speed, and export a take as a .mid file.
///
/// <para>Live notes reach the recorder through <see cref="CaptureNoteOn"/>/<see cref="CaptureNoteOff"/>
/// on the MIDI callback thread. Playback goes out through the host's app-note path, so what the
/// player sounds lights the keyboard and feeds the readout but is never recorded back in.</para>
/// </summary>
public partial class RecorderViewModel : ObservableObject, IDisposable
{
    public const int MaxTakes = 10;

    private readonly NoteRecorder _recorder = new();
    private readonly NoteSequencePlayer _player;
    private readonly Dispatcher _dispatcher;
    private readonly Func<int> _exportBpm;
    private readonly Action<string> _reportStatus;

    /// <summary>Drives the elapsed-time readout while recording, and notices the first note.</summary>
    private readonly DispatcherTimer _recordClock;

    /// <summary>The take the player currently holds, which may differ from the selection.</summary>
    private Take? _loadedTake;

    private int _nextTakeNumber = 1;

    /// <summary>Newest first.</summary>
    public ObservableCollection<Take> Takes { get; } = new();

    public IReadOnlyList<SpeedOption> Speeds { get; } =
    [
        new(0.5, "50%"),
        new(0.75, "75%"),
        new(1.0, "100%")
    ];

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsRecordArmed))]
    private RecordState _recordState;

    /// <summary>Recording, or waiting for the first note to start recording.</summary>
    public bool IsRecordArmed => RecordState != RecordState.Idle;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasSelectedTake))]
    private Take? _selectedTake;

    public bool HasSelectedTake => SelectedTake != null;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(SetLoopStartCommand))]
    [NotifyCanExecuteChangedFor(nameof(SetLoopEndCommand))]
    private bool _isPlaying;

    [ObservableProperty]
    private bool _isLooping;

    [ObservableProperty]
    private double _speed = 1.0;

    /// <summary>The playhead in take time, for the timeline. Zero while stopped.</summary>
    [ObservableProperty]
    private TimeSpan _playbackPosition;

    /// <summary>The A marker in the selected take, if set. Without B the loop runs to the end.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasLoopRegion))]
    [NotifyPropertyChangedFor(nameof(LoopRegionText))]
    [NotifyCanExecuteChangedFor(nameof(ClearLoopRegionCommand))]
    private TimeSpan? _loopStart;

    /// <summary>The B marker in the selected take, if set. Without A the loop runs from the start.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasLoopRegion))]
    [NotifyPropertyChangedFor(nameof(LoopRegionText))]
    [NotifyCanExecuteChangedFor(nameof(ClearLoopRegionCommand))]
    private TimeSpan? _loopEnd;

    public bool HasLoopRegion => LoopStart.HasValue || LoopEnd.HasValue;

    public string LoopRegionText => HasLoopRegion
        ? $"A {Take.FormatTime(LoopStart ?? TimeSpan.Zero)} – B {Take.FormatTime(LoopEnd ?? SelectedTake?.Length ?? TimeSpan.Zero)}"
        : "Drag across the timeline to set an A–B loop";

    /// <summary>Recording time, playback position, or "waiting" before the first note.</summary>
    [ObservableProperty]
    private string _clockText = "";

    /// <param name="noteOn">The host's app-note path: sounds and lights a note, never records it.</param>
    /// <param name="noteOff">Its release counterpart.</param>
    /// <param name="exportBpm">The tempo written into exported files.</param>
    /// <param name="reportStatus">Shows a message in the status bar; called on the UI thread.</param>
    public RecorderViewModel(Dispatcher dispatcher, Action<int, int> noteOn, Action<int> noteOff,
                             Func<int> exportBpm, Action<string> reportStatus)
    {
        _dispatcher = dispatcher;
        _exportBpm = exportBpm;
        _reportStatus = reportStatus;

        _player = new NoteSequencePlayer(noteOn, noteOff);
        _player.PositionChanged += OnPlayerPositionChanged;
        _player.PlaybackStopped += OnPlayerStopped;

        _recordClock = new DispatcherTimer(TimeSpan.FromMilliseconds(100), DispatcherPriority.Normal,
                                           (_, _) => OnRecordClockTick(), dispatcher);
        _recordClock.Stop();
    }

    // ----- Capture, called on the MIDI callback thread -----

    public void CaptureNoteOn(int note, int velocity, long timestamp) =>
        _recorder.NoteOn(note, velocity, timestamp);

    public void CaptureNoteOff(int note, long timestamp) =>
        _recorder.NoteOff(note, timestamp);

    // ----- Recording -----

    [RelayCommand]
    private void ToggleRecord()
    {
        if (RecordState == RecordState.Idle)
        {
            _recorder.Arm();
            RecordState = RecordState.WaitingForFirstNote;
            _recordClock.Start();
            UpdateClock();
            _reportStatus("Record armed — recording starts at your first note. Ctrl+R to stop.");
            return;
        }

        var notes = _recorder.Stop(Stopwatch.GetTimestamp());
        RecordState = RecordState.Idle;
        _recordClock.Stop();
        UpdateClock();

        if (notes.Count == 0)
        {
            _reportStatus("Recording cancelled — no notes were played");
            return;
        }

        var take = new Take(_nextTakeNumber++, notes, DateTime.Now);
        Takes.Insert(0, take);
        while (Takes.Count > MaxTakes)
            RemoveTake(Takes[^1]);

        SelectedTake = take;
        _reportStatus($"{take.Name} recorded: {take.LengthText}, {take.Summary}");
    }

    private void OnRecordClockTick()
    {
        if (RecordState == RecordState.WaitingForFirstNote && _recorder.HasStarted)
            RecordState = RecordState.Recording;
        UpdateClock();
    }

    // ----- Playback -----

    /// <summary>Plays the selected take, or the newest one if none is selected; stops if playing.</summary>
    [RelayCommand]
    private void TogglePlayback()
    {
        if (IsPlaying)
        {
            StopPlayback();
            return;
        }

        var take = SelectedTake ?? Takes.FirstOrDefault();
        if (take == null)
        {
            _reportStatus("Nothing to play yet — Ctrl+R to record a take");
            return;
        }

        SelectedTake = take;
        if (_loadedTake != take)
        {
            _player.Load(take.Notes, take.Length);
            _loadedTake = take;
        }

        ApplyLoopRegion();
        _player.IsLooping = IsLooping;
        _player.Speed = Speed;
        _player.Play();

        IsPlaying = _player.IsPlaying;
        UpdateClock();
    }

    /// <summary>Jumps the playhead while playing, as a click on the timeline does. Ignored when stopped.</summary>
    public void Seek(TimeSpan position)
    {
        if (!IsPlaying) return;
        _player.Play(position);
        PlaybackPosition = position;
        UpdateClock();
    }

    private void StopPlayback()
    {
        _player.Stop();
        IsPlaying = false;
        PlaybackPosition = TimeSpan.Zero;
        UpdateClock();
    }

    partial void OnIsLoopingChanged(bool value) => _player.IsLooping = value;

    partial void OnSpeedChanged(double value) => _player.Speed = value;

    partial void OnSelectedTakeChanged(Take? value)
    {
        // A–B markers belong to one take. Playback follows the selection too, so the timeline
        // never shows one take's notes under another take's playhead.
        LoopStart = null;
        LoopEnd = null;
        if (IsPlaying && _loadedTake != value)
            StopPlayback();
    }

    partial void OnLoopStartChanged(TimeSpan? value) => ApplyLoopRegion();

    partial void OnLoopEndChanged(TimeSpan? value) => ApplyLoopRegion();

    private void ApplyLoopRegion()
    {
        if (_loadedTake == null) return;

        if (_loadedTake == SelectedTake && HasLoopRegion)
        {
            var start = LoopStart ?? TimeSpan.Zero;
            var end = LoopEnd ?? _loadedTake.Length;
            if (end - start >= NoteSequencePlayer.MinimumLoopLength)
            {
                _player.SetLoopRange(start, end);
                return;
            }
        }

        _player.ClearLoopRange();
    }

    // Both markers can also be dragged out on the timeline, which sets them directly.

    /// <summary>Marks A at the playhead — for finding a loop point by ear while it plays.</summary>
    [RelayCommand(CanExecute = nameof(IsPlaying))]
    private void SetLoopStart()
    {
        var at = PlaybackPosition;
        if (LoopEnd is { } end && end - at < NoteSequencePlayer.MinimumLoopLength)
            LoopEnd = null;
        LoopStart = at;
    }

    /// <summary>Marks B at the playhead, completing the region, and turns looping on.</summary>
    [RelayCommand(CanExecute = nameof(IsPlaying))]
    private void SetLoopEnd()
    {
        var at = PlaybackPosition;
        if (at - (LoopStart ?? TimeSpan.Zero) < NoteSequencePlayer.MinimumLoopLength)
        {
            _reportStatus("B has to come after A");
            return;
        }
        LoopEnd = at;
        IsLooping = true;
    }

    /// <summary>Sets both markers at once, as a drag across the timeline does, and turns looping on.</summary>
    public void SetLoopRegion(TimeSpan start, TimeSpan end)
    {
        if (end < start) (start, end) = (end, start);
        LoopStart = start;
        LoopEnd = end;
        IsLooping = true;
    }

    [RelayCommand(CanExecute = nameof(HasLoopRegion))]
    private void ClearLoopRegion()
    {
        LoopStart = null;
        LoopEnd = null;
    }

    private void OnPlayerPositionChanged(object? sender, PlaybackPositionEventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            // A late update from a run that has since been stopped must not move the playhead.
            if (!IsPlaying) return;
            PlaybackPosition = e.Position;
            UpdateClock();
        });
    }

    private void OnPlayerStopped(object? sender, EventArgs e)
    {
        _dispatcher.BeginInvoke(() =>
        {
            // Re-read rather than assume: a Play queued before this ran would otherwise be undone.
            IsPlaying = _player.IsPlaying;
            if (!IsPlaying) PlaybackPosition = TimeSpan.Zero;
            UpdateClock();
        });
    }

    // ----- Takes -----

    [RelayCommand]
    private void DeleteTake(Take? take)
    {
        if (take == null || !Takes.Contains(take)) return;
        RemoveTake(take);
        _reportStatus($"Deleted {take.Name}");
    }

    private void RemoveTake(Take take)
    {
        if (_loadedTake == take)
        {
            StopPlayback();
            _player.Load([]);
            _loadedTake = null;
        }

        bool wasSelected = SelectedTake == take;
        Takes.Remove(take);
        if (wasSelected)
            SelectedTake = Takes.FirstOrDefault();
    }

    [RelayCommand]
    private void ExportTake(Take? take)
    {
        take ??= SelectedTake;
        if (take == null)
        {
            _reportStatus("No take to export");
            return;
        }

        int bpm = _exportBpm();
        var dialog = new SaveFileDialog
        {
            Title = $"Export {take.Name} as MIDI",
            Filter = "MIDI files (*.mid)|*.mid|All files (*.*)|*.*",
            DefaultExt = ".mid",
            AddExtension = true,
            FileName = $"{take.Name} {take.CreatedAt:yyyy-MM-dd HHmm}.mid"
        };

        if (dialog.ShowDialog() != true) return;

        try
        {
            TakeMidiExporter.Export(take, dialog.FileName, bpm);
            _reportStatus($"Exported {take.Name} to {dialog.FileName} at {bpm} BPM");
        }
        catch (Exception ex)
        {
            _reportStatus($"Export failed: {ex.Message}");
        }
    }

    private void UpdateClock()
    {
        ClockText = RecordState switch
        {
            RecordState.WaitingForFirstNote => "waiting",
            RecordState.Recording => Take.FormatTime(_recorder.Elapsed),
            _ when IsPlaying => Take.FormatTime(PlaybackPosition),
            _ => ""
        };
    }

    public void Dispose()
    {
        _recordClock.Stop();
        _recorder.Cancel();
        _player.PositionChanged -= OnPlayerPositionChanged;
        _player.PlaybackStopped -= OnPlayerStopped;
        _player.Dispose();
    }
}
