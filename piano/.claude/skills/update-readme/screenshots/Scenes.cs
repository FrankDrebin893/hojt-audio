using System.IO;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Services;
using PianoMidiVisualizationApp.Services.SongPractice;

namespace ReadmeScreenshots;

/// <param name="Name">The PNG's file name, without extension; README links use it.</param>
/// <param name="Width">Window size in DIPs. Keep it near 1280 wide: GitHub shows README images
/// at about 880px, so a much wider window makes the text unreadably small.</param>
/// <param name="SetUp">False starts like a first launch: no piano sound chosen and audio off.</param>
internal sealed record Scene(string Name, int Width, int Height, Action<Host> Setup, bool SetUp = true);

/// <summary>
/// Every README screenshot. Each starts from a fresh window with default settings (the
/// first-launch panel layout: progression sidebar, circle of fifths, grand staff, chord strip
/// and status bar on), the keyboard connected and audio on, and sets up only what it shows.
/// </summary>
internal static class Scenes
{
    // MIDI note numbers, middle C = 60.
    private const int C2 = 36, F2 = 41, G2 = 43, A2 = 45, B2 = 47,
                      C3 = 48, D3 = 50, E3 = 52, F3 = 53, G3 = 55, A3 = 57, B3 = 59,
                      C4 = 60, D4 = 62, E4 = 64, F4 = 65, G4 = 67, A4 = 69, C5 = 72;

    public static IReadOnlyList<Scene> All { get; } =
    [
        new("main-window", 1280, 720, Overview),
        new("song-practice", 1280, 720, SongPractice),
        new("recorder", 1280, 840, Recorder),
        new("settings", 1280, 560, Settings),
        new("zen-mode", 1280, 520, ZenMode),
        new("first-run", 1280, 600, _ => { }, SetUp: false),
    ];

    /// <summary>
    /// The hero shot: a progression saved in C major, and a G7 held over its third, so the
    /// readout shows a figured-bass numeral, the staff and circle follow it, and the chord
    /// strip, switched to sevenths, lights V7.
    /// </summary>
    private static void Overview(Host host)
    {
        var vm = host.Vm;
        vm.Settings.KeyScale = ScaleType.Major;
        vm.Settings.KeyTonicPitchClass = 0;
        vm.IsMetronomeOn = true;
        vm.ChordStrip.ShowSevenths = true;

        host.SaveChord(C3, G3, C4, E4);
        host.SaveChord(A2, E3, A3, C4, E4);
        host.SaveChord(F2, A3, C4, F4);
        host.SaveChord(G2, B3, D4, F4);

        host.Press(B2, F3, G3, D4);
    }

    /// <summary>Wait mode part-way through a song: a few chords played, one fluffed, the next one hinted.</summary>
    private static void SongPractice(Host host)
    {
        // The sidebar would sit empty beside the song; the falling notes get its width instead.
        host.Vm.IsProgressionVisible = false;

        var song = host.Vm.SongPractice;
        string path = DemoSong.Write(Path.Combine(Path.GetTempPath(), "ReadmeScreenshots"));
        if (!song.LoadSong(path, showTrackList: false))
            throw new InvalidOperationException("The demo song did not load: " + host.Vm.StatusText);

        song.Mode = PracticeMode.Wait;
        song.Play();

        for (int chord = 0; chord < 9; chord++)
        {
            Host.PumpUntil(() => song.IsWaiting, TimeSpan.FromSeconds(15), $"chord {chord + 1}");
            var notes = song.PendingChord!.Notes.ToArray();

            if (chord == 6)
            {
                host.Press(notes[0] + 2);   // a wrong note first, so the score is not suspiciously perfect
                host.Release(notes[0] + 2);
            }

            host.Press(notes);
            host.Release(notes);
        }

        Host.PumpUntil(() => song.IsWaiting, TimeSpan.FromSeconds(15), "the chord to hold at");
        Host.Pump(TimeSpan.FromMilliseconds(300));
    }

    /// <summary>A take recorded and playing back in an A–B loop, with the MIDI log open below.</summary>
    private static void Recorder(Host host)
    {
        var vm = host.Vm;
        vm.IsRecorderVisible = true;
        vm.IsMidiLogVisible = true;

        // A short warm-up take, so the list has more than one entry.
        vm.Recorder.ToggleRecordCommand.Execute(null);
        PlayBrokenChords(host, [[C3, E4, G4, C5]]);
        vm.Recorder.ToggleRecordCommand.Execute(null);

        vm.Recorder.ToggleRecordCommand.Execute(null);
        PlayBrokenChords(host, [[C3, E4, G4, C5], [A2, E4, A4, C5], [F2, F4, A4, C5], [G2, D4, G4, B3]]);
        vm.Recorder.ToggleRecordCommand.Execute(null);
        Host.PumpUntil(() => vm.Recorder.Takes.Count == 2, TimeSpan.FromSeconds(5), "the takes to be kept");

        var take = vm.Recorder.Takes.OrderByDescending(t => t.Length).First();
        vm.Recorder.SelectedTake = take;
        vm.Recorder.IsLooping = true;
        vm.Recorder.SetLoopRegion(take.Length * 0.25, take.Length * 0.75);
        vm.Recorder.TogglePlaybackCommand.Execute(null);

        // Into the loop, with the whole of the Am (about 1.1s to 2.0s into the take) held.
        Host.PumpUntil(() => vm.Recorder.PlaybackPosition >= TimeSpan.FromSeconds(1.8),
                       TimeSpan.FromSeconds(10), "the playhead to reach the Am");
    }

    /// <summary>Each chord as a held bass note and a rising arpeggio, in real time.</summary>
    private static void PlayBrokenChords(Host host, int[][] chords)
    {
        foreach (var chord in chords)
        {
            host.Press(chord[0]);
            foreach (var note in chord.Skip(1))
            {
                Host.Pump(TimeSpan.FromMilliseconds(140));
                host.Press(note);
            }
            Host.Pump(TimeSpan.FromMilliseconds(420));
            host.Release(chord);
        }
    }

    /// <summary>
    /// The settings overlay, connected and running, with a SoundFont picked (see <see cref="Host"/>),
    /// and two piano controls learned the way a player would: the soft pedal saves the chord, and
    /// a drum pad on channel 10 arms the recorder.
    /// </summary>
    private static void Settings(Host host)
    {
        var vm = host.Vm;
        vm.IsSettingsOverlayVisible = true;

        var controls = vm.PianoControls;
        controls.LearnCommand.Execute(controls.Rows.Single(r => r.Action == PianoAction.SaveChord));
        host.Midi.MoveControl(67, 127);   // the soft pedal, down
        host.Midi.MoveControl(67, 0);
        Host.PumpUntil(() => !controls.IsLearning, TimeSpan.FromSeconds(5), "the soft pedal to be learned");

        controls.LearnCommand.Execute(controls.Rows.Single(r => r.Action == PianoAction.ToggleRecord));
        host.Midi.PressKey(36, 100, channel: 9);   // a pad sending C2 (MIDI 36) on channel 10
        host.Midi.ReleaseKey(36, channel: 9);
        Host.PumpUntil(() => !controls.IsLearning, TimeSpan.FromSeconds(5), "the pad to be learned");

        host.Press(C3, G3, E4);
    }

    /// <summary>Zen mode (F11): everything but the readout and the keyboard hidden, in F major.</summary>
    private static void ZenMode(Host host)
    {
        var vm = host.Vm;
        vm.Settings.KeyScale = ScaleType.Major;
        vm.Settings.KeyTonicPitchClass = 5;
        vm.ToggleZenModeCommand.Execute(null);

        host.Press(36 + 10, A3, D4, F4);   // Bb2 A3 D4 F4: Bbmaj7, IVmaj7 in F
    }
}
