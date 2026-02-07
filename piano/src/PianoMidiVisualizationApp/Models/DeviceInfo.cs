namespace PianoMidiVisualizationApp.Models;

public record DeviceInfo(int Index, string Name)
{
    public override string ToString() => Name;
}
