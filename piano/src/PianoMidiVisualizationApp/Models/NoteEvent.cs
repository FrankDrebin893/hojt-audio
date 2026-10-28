namespace PianoMidiVisualizationApp.Models;

public class NoteEventArgs : EventArgs
{
    public int NoteNumber { get; init; }
    public int Velocity { get; init; }
    public int Channel { get; init; }
}
