namespace PianoMidiVisualizationApp.Models;

public class SavedChord
{
    public string ChordName { get; init; } = "";
    public List<int> NoteNumbers { get; init; } = new();
    public List<string> NoteNames { get; init; } = new();

    public string NotesDisplay => string.Join(" ", NoteNames);
}
