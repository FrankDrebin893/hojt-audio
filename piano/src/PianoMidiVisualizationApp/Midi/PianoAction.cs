namespace PianoMidiVisualizationApp.Midi;

/// <summary>
/// What a learned control on the piano can do. Saved by name, so the order here is free to
/// change: it is the order the Settings grid lists them in, row by row, pairing related actions.
/// </summary>
public enum PianoAction
{
    SaveChord,
    ToggleProgressionPlayback,
    ToggleRecord,
    ToggleTakePlayback,
    ToggleMetronome,
    TapTempo,
    ToggleSongPlayback,
    RestartSong,
}

/// <summary>How an action reads in the Settings list, and in what it says once learned.</summary>
public sealed record PianoActionInfo(PianoAction Action, string Label, string Shortcut, string Does)
{
    public static readonly IReadOnlyList<PianoActionInfo> All =
    [
        new(PianoAction.SaveChord, "Save chord", "Space",
            "saves the chord to the progression"),
        new(PianoAction.ToggleProgressionPlayback, "Play progression", "Ctrl+P",
            "plays and stops the progression"),
        new(PianoAction.ToggleRecord, "Record", "Ctrl+R",
            "arms and stops recording"),
        new(PianoAction.ToggleTakePlayback, "Play take", "Ctrl+Shift+R",
            "plays and stops the latest take"),
        new(PianoAction.ToggleMetronome, "Metronome", "Ctrl+M",
            "turns the metronome on and off"),
        new(PianoAction.TapTempo, "Tap tempo", "",
            "taps the metronome tempo"),
        new(PianoAction.ToggleSongPlayback, "Play song", "",
            "plays and pauses the song"),
        new(PianoAction.RestartSong, "Restart song", "",
            "restarts the song"),
    ];

    public static PianoActionInfo For(PianoAction action) => All.First(a => a.Action == action);
}
