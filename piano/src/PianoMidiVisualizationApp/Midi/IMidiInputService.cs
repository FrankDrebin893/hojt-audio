using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.Midi;

public class RawMidiMessageEventArgs : EventArgs
{
    public required string Description { get; init; }
}

public interface IMidiInputService : IDisposable
{
    IReadOnlyList<DeviceInfo> GetAvailableDevices();
    void Open(int deviceIndex);
    void Close();
    bool IsOpen { get; }

    event EventHandler<NoteEventArgs>? NoteOn;
    event EventHandler<NoteEventArgs>? NoteOff;
    event EventHandler<RawMidiMessageEventArgs>? MessageReceived;
}
