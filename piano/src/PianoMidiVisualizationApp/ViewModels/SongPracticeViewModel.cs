using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Windows.Media;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using PianoMidiVisualizationApp.Services.SongPractice;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>One entry in a part's role dropdown.</summary>
public record PartRoleOption(PartRole Role, string Name);

/// <summary>A song part as the track list shows it, with the role you picked for it.</summary>
public partial class SongPartViewModel : ObservableObject
{
    public SongPart Part { get; }

    public string Name => Part.Name;
    public string Instrument => Part.Instrument;
    public int NoteCount => Part.NoteCount;
    public int ColorIndex => Part.ColorIndex;

    public string Details => $"{Part.Instrument} · {Part.NoteCount} {(Part.NoteCount == 1 ? "note" : "notes")}";

    [ObservableProperty]
    private PartRole _role;

    public SongPartViewModel(SongPart part)
    {
        Part = part;
        _role = part.DefaultRole;
    }
}

/// <summary>
/// Falling-notes song practice: a loaded MIDI song, its parts and their roles, and the
/// transport that plays it — play/pause, restart, speed, and an A–B loop by bars.
/// </summary>
/// <remarks>
/// <para>Time comes from <see cref="Clock"/>, which the falling-notes view reads every frame.
/// Auto-play parts sound through <see cref="SongAutoPlayer"/> on the host's app-note path, so
/// they light the keyboard like any app-played note. The clock's barrier is placed at the
/// point playback must stop or turn — the loop end or the song's end — so it is caught
/// exactly, not a frame late.</para>
///
/// <para>Runs on the UI thread. Per-frame work is driven by <see cref="CompositionTarget.Rendering"/>
/// and only while playing.</para>
/// </remarks>
public partial class SongPracticeViewModel : ObservableObject, IDisposable
{
    public const double MinimumSpeed = 0.25;
    public const double MaximumSpeed = 1.0;
    public const double SpeedStep = 0.05;

    /// <summary>
    /// Real time between pressing play and the first note reaching the keys, so it can be
    /// seen coming. Scaled by the speed into song time.
    /// </summary>
    private static readonly TimeSpan LeadIn = TimeSpan.FromSeconds(1.5);

    private readonly SongAutoPlayer _autoPlayer;
    private readonly Action<string> _reportStatus;

    /// <summary>Auto-play notes starting before this song time have been handed to a player.</summary>
    private TimeSpan _scheduledUntil;

    private bool _isTicking;

    public SongClock Clock { get; } = new();

    public ObservableCollection<SongPartViewModel> Parts { get; } = new();

    public static IReadOnlyList<PartRoleOption> RoleOptions { get; } =
    [
        new(PartRole.YouPlay, "You play"),
        new(PartRole.AutoPlay, "Auto-play"),
        new(PartRole.Mute, "Mute")
    ];

    /// <summary>The notes the view draws: every part that is not muted, in start order.</summary>
    public IReadOnlyList<SongNote> VisibleNotes { get; private set; } = [];

    /// <summary>The longest visible note, so the view knows how far back a note can still be showing.</summary>
    public TimeSpan LongestVisibleNote { get; private set; }

    /// <summary>Each part's role, indexed by part, for the view's per-note lookups.</summary>
    public IReadOnlyList<PartRole> PartRoles { get; private set; } = [];

    /// <summary>
    /// Raised on the UI thread whenever what the falling-notes view shows may have changed:
    /// every frame while playing, and after any seek, load or setting change.
    /// </summary>
    public event EventHandler? FrameAdvanced;

    /// <summary>Raised after a song loads successfully.</summary>
    public event EventHandler? SongLoaded;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasSong))]
    [NotifyPropertyChangedFor(nameof(Title))]
    [NotifyPropertyChangedFor(nameof(SongDetails))]
    [NotifyPropertyChangedFor(nameof(BarCount))]
    [NotifyPropertyChangedFor(nameof(BarText))]
    [NotifyCanExecuteChangedFor(nameof(TogglePlayCommand))]
    [NotifyCanExecuteChangedFor(nameof(RestartCommand))]
    [NotifyCanExecuteChangedFor(nameof(CloseSongCommand))]
    [NotifyCanExecuteChangedFor(nameof(StartFromTrackListCommand))]
    private Song? _song;

    public bool HasSong => Song != null;

    public string Title => Song?.Title ?? "";

    public string SongDetails => Song is { } s
        ? $"{Math.Round(s.InitialBpm)} BPM · {s.BarCount} {(s.BarCount == 1 ? "bar" : "bars")}"
        : "";

    public int BarCount => Song?.BarCount ?? 0;

    /// <summary>The file the song came from, persisted so it reopens on the next launch.</summary>
    public string? SongPath { get; private set; }

    /// <summary>Whether the practice view is on screen. Hiding it pauses playback.</summary>
    [ObservableProperty]
    private bool _isActive;

    [ObservableProperty]
    private bool _isPlaying;

    /// <summary>Reached the end; the next play starts over.</summary>
    [ObservableProperty]
    private bool _isFinished;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SpeedText))]
    private double _speed = 1.0;

    public string SpeedText => $"{Math.Round(Speed * 100)}%";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(BarText))]
    private int _currentBar = 1;

    public string BarText => HasSong ? $"{CurrentBar} / {BarCount}" : "";

    [ObservableProperty]
    private bool _isLoopEnabled;

    /// <summary>First bar of the loop, 1-based.</summary>
    [ObservableProperty]
    private int _loopStartBar = 1;

    /// <summary>Last bar of the loop, 1-based and inclusive.</summary>
    [ObservableProperty]
    private int _loopEndBar = 1;

    [ObservableProperty]
    private bool _isTrackListOpen;

    /// <param name="noteOn">The host's app-note path, which sounds and lights a note without recording it.</param>
    /// <param name="noteOff">Its release counterpart.</param>
    /// <param name="reportStatus">Shows a message in the status bar.</param>
    public SongPracticeViewModel(Action<int, int> noteOn, Action<int> noteOff, Action<string> reportStatus)
    {
        _autoPlayer = new SongAutoPlayer(noteOn, noteOff);
        _reportStatus = reportStatus;
    }

    // ------------------------------------------------------------------ loop range

    /// <summary>Where the loop starts, in song time. Only meaningful with a song loaded.</summary>
    public TimeSpan LoopStartTime => Song?.BarStart(LoopStartBar) ?? TimeSpan.Zero;

    /// <summary>Where the loop wraps (exclusive): the end of <see cref="LoopEndBar"/>.</summary>
    public TimeSpan LoopEndTime => Song?.BarStart(LoopEndBar + 1) ?? TimeSpan.Zero;

    private bool IsLooping => IsLoopEnabled && Song != null;

    /// <summary>Where playback stops or turns: the loop end, or the end of the song.</summary>
    private TimeSpan PlaybackEnd => IsLooping ? LoopEndTime : Song?.Duration ?? TimeSpan.Zero;

    /// <summary>Where a restart goes: the loop start, or the first note you would see.</summary>
    private TimeSpan StartPoint => IsLooping
        ? LoopStartTime
        : VisibleNotes.Count > 0 ? VisibleNotes[0].Start : TimeSpan.Zero;

    private TimeSpan LeadInSongTime => LeadIn * Speed;

    partial void OnIsLoopEnabledChanged(bool value) => OnLoopRangeChanged();

    partial void OnLoopStartBarChanged(int value)
    {
        int clamped = Math.Clamp(value, 1, Math.Max(1, BarCount));
        if (clamped != value) { LoopStartBar = clamped; return; }
        if (LoopEndBar < value) LoopEndBar = value;
        OnLoopRangeChanged();
    }

    partial void OnLoopEndBarChanged(int value)
    {
        int clamped = Math.Clamp(value, 1, Math.Max(1, BarCount));
        if (clamped != value) { LoopEndBar = clamped; return; }
        if (LoopStartBar > value) LoopStartBar = value;
        OnLoopRangeChanged();
    }

    [RelayCommand]
    private void LoopStartEarlier() => LoopStartBar--;

    [RelayCommand]
    private void LoopStartLater() => LoopStartBar++;

    [RelayCommand]
    private void LoopEndEarlier() => LoopEndBar--;

    [RelayCommand]
    private void LoopEndLater() => LoopEndBar++;

    /// <summary>
    /// Keeps playback inside a changed loop: from outside it, jump to its start; from inside,
    /// carry on, re-scheduling the accompaniment if the old range had handed out too much.
    /// </summary>
    private void OnLoopRangeChanged()
    {
        if (Song == null) return;

        if (IsLooping)
        {
            var position = Clock.Position;
            if (position < LoopStartTime - LeadInSongTime || position >= LoopEndTime)
            {
                SeekTo(LoopStartTime - LeadInSongTime);
                return;
            }
        }

        RefreshBarrier();
        RaiseFrame();
    }

    // ------------------------------------------------------------------ loading

    [RelayCommand]
    private void OpenSong()
    {
        var dialog = new OpenFileDialog
        {
            Title = "Open a MIDI song",
            Filter = "MIDI files (*.mid;*.midi;*.kar)|*.mid;*.midi;*.kar|All files (*.*)|*.*"
        };
        if (SongPath is { } previous && Path.GetDirectoryName(previous) is { } folder && Directory.Exists(folder))
            dialog.InitialDirectory = folder;

        if (dialog.ShowDialog() == true)
            LoadSong(dialog.FileName, showTrackList: true);
    }

    /// <summary>Loads a song, replacing any current one. Reports and returns false if it cannot be read.</summary>
    public bool LoadSong(string path, bool showTrackList)
    {
        Song song;
        try
        {
            song = MidiSongLoader.Load(path);
        }
        catch (Exception ex)
        {
            _reportStatus($"Could not open {Path.GetFileName(path)}: {ex.Message}");
            return false;
        }

        Pause();
        foreach (var part in Parts)
            part.PropertyChanged -= OnPartPropertyChanged;
        Parts.Clear();

        Song = song;
        SongPath = path;
        foreach (var part in song.Parts)
        {
            var vm = new SongPartViewModel(part);
            vm.PropertyChanged += OnPartPropertyChanged;
            Parts.Add(vm);
        }

        // A four-bar loop from the top is a useful default the moment looping is switched on.
        IsLoopEnabled = false;
        LoopEndBar = Math.Min(4, song.BarCount);
        LoopStartBar = 1;

        IsFinished = false;
        ApplyRoles();
        SeekTo(StartPoint - LeadInSongTime);
        IsTrackListOpen = showTrackList;

        int yours = Parts.Count(p => p.Role == PartRole.YouPlay);
        _reportStatus($"Loaded {song.Title}: {Parts.Count} {(Parts.Count == 1 ? "part" : "parts")}, " +
                      $"{yours} for you to play, {song.BarCount} bars at {Math.Round(song.InitialBpm)} BPM");
        SongLoaded?.Invoke(this, EventArgs.Empty);
        return true;
    }

    [RelayCommand(CanExecute = nameof(HasSong))]
    private void CloseSong()
    {
        Pause();
        foreach (var part in Parts)
            part.PropertyChanged -= OnPartPropertyChanged;
        Parts.Clear();
        _autoPlayer.SetNotes([]);
        VisibleNotes = [];
        PartRoles = [];
        Song = null;
        SongPath = null;
        IsTrackListOpen = false;
        IsFinished = false;
        RaiseFrame();
    }

    private void OnPartPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(SongPartViewModel.Role)) return;
        ApplyRoles();
        RestartSchedulingFromHere();
    }

    /// <summary>Rebuilds what is drawn and what is auto-played from the parts' roles.</summary>
    private void ApplyRoles()
    {
        if (Song == null) return;

        PartRoles = Parts.Select(p => p.Role).ToArray();
        VisibleNotes = Song.Notes.Where(n => PartRoles[n.Part] != PartRole.Mute).ToArray();
        LongestVisibleNote = VisibleNotes.Count > 0 ? VisibleNotes.Max(n => n.Duration) : TimeSpan.Zero;
        _autoPlayer.SetNotes(Song.Notes.Where(n => PartRoles[n.Part] == PartRole.AutoPlay));
    }

    [RelayCommand]
    private void ToggleTrackList() => IsTrackListOpen = !IsTrackListOpen;

    [RelayCommand]
    private void CloseTrackList() => IsTrackListOpen = false;

    /// <summary>The track list's own button: closes it, and starts the song if it is not already playing.</summary>
    [RelayCommand(CanExecute = nameof(HasSong))]
    private void StartFromTrackList()
    {
        IsTrackListOpen = false;
        Play();
    }

    // ------------------------------------------------------------------ transport

    [RelayCommand(CanExecute = nameof(HasSong))]
    private void TogglePlay()
    {
        if (IsPlaying) Pause();
        else Play();
    }

    /// <summary>Back to the start (or the loop start) with a lead-in. Keeps playing if it was.</summary>
    [RelayCommand(CanExecute = nameof(HasSong))]
    private void Restart()
    {
        IsFinished = false;
        SeekTo(StartPoint - LeadInSongTime);
    }

    public void Play()
    {
        if (Song == null || IsPlaying) return;

        if (IsFinished)
        {
            IsFinished = false;
            Clock.Seek(StartPoint - LeadInSongTime);
        }

        IsTrackListOpen = false;
        IsPlaying = true;
        Clock.Start();
        _scheduledUntil = FirstSchedulable(Clock.Position);
        RefreshBarrier();
        StartTicking();
        RaiseFrame();
    }

    public void Pause()
    {
        if (!IsPlaying) return;
        Clock.Pause();
        _autoPlayer.StopAll();
        IsPlaying = false;
        StopTicking();
        RaiseFrame();
    }

    [RelayCommand]
    private void IncreaseSpeed() => Speed += SpeedStep;

    [RelayCommand]
    private void DecreaseSpeed() => Speed -= SpeedStep;

    partial void OnSpeedChanged(double value)
    {
        // Snapped to the 5% grid so repeated steps never drift to 0.7499999.
        double snapped = double.IsFinite(value)
            ? Math.Clamp(Math.Round(Math.Round(value / SpeedStep) * SpeedStep, 2), MinimumSpeed, MaximumSpeed)
            : MaximumSpeed;
        if (snapped != value)
        {
            Speed = snapped;
            return;
        }

        Clock.Speed = value;
        _autoPlayer.Speed = value;
    }

    partial void OnIsActiveChanged(bool value)
    {
        if (!value) Pause();
    }

    /// <summary>Jumps to a song time, carrying on playing if it was.</summary>
    private void SeekTo(TimeSpan position)
    {
        _autoPlayer.StopAll();
        Clock.Seek(position);
        _scheduledUntil = FirstSchedulable(position);
        RefreshBarrier();
        UpdateCurrentBar(position);
        RaiseFrame();
    }

    /// <summary>After a role change: stop what was handed out and re-schedule from the playhead.</summary>
    private void RestartSchedulingFromHere()
    {
        _autoPlayer.StopAll();
        _scheduledUntil = FirstSchedulable(Clock.Position);
        RefreshBarrier();
        RaiseFrame();
    }

    /// <summary>Notes before the loop start belong to the lead-in's run-up, not to the loop.</summary>
    private TimeSpan FirstSchedulable(TimeSpan position) =>
        IsLooping && position < LoopStartTime ? LoopStartTime : position;

    /// <summary>
    /// Places the clock's barrier where playback next has to stop or turn, and hands the
    /// auto-play parts everything up to it.
    /// </summary>
    private void RefreshBarrier()
    {
        if (Song == null)
        {
            Clock.SetBarrier(null);
            return;
        }

        var barrier = PlaybackEnd;

        // The range shrank behind what was already handed out (the loop moved): take it back.
        if (barrier < _scheduledUntil)
        {
            _autoPlayer.StopAll();
            _scheduledUntil = FirstSchedulable(Clock.Position);
        }

        Clock.SetBarrier(barrier);

        if (IsPlaying && barrier > _scheduledUntil)
        {
            _autoPlayer.Schedule(_scheduledUntil, barrier, Clock.Position);
            _scheduledUntil = barrier;
        }
    }

    // ------------------------------------------------------------------ per frame

    private void StartTicking()
    {
        if (_isTicking) return;
        CompositionTarget.Rendering += OnRendering;
        _isTicking = true;
    }

    private void StopTicking()
    {
        if (!_isTicking) return;
        CompositionTarget.Rendering -= OnRendering;
        _isTicking = false;
    }

    private void OnRendering(object? sender, EventArgs e) => Tick();

    /// <summary>One frame: handles reaching the barrier, then tells the view to redraw.</summary>
    public void Tick()
    {
        if (Song == null || !IsPlaying) return;

        if (Clock.IsAtBarrier && Clock.Barrier is { } barrier)
        {
            if (IsLooping && barrier == LoopEndTime)
            {
                // Straight back to the loop start, without a lead-in: the loop should keep time.
                SeekTo(LoopStartTime);
                return;
            }

            if (barrier >= Song.Duration)
            {
                Finish();
                return;
            }
        }

        UpdateCurrentBar(Clock.Position);
        RaiseFrame();
    }

    private void Finish()
    {
        Pause();
        IsFinished = true;
        _reportStatus($"Finished {Song?.Title}");
    }

    private void UpdateCurrentBar(TimeSpan position)
    {
        if (Song != null) CurrentBar = Song.BarAt(position);
    }

    private void RaiseFrame() => FrameAdvanced?.Invoke(this, EventArgs.Empty);

    public void Dispose()
    {
        StopTicking();
        _autoPlayer.Dispose();
    }
}
