using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.Midi;

public class RawMidiMessageEventArgs : EventArgs
{
    public required string Description { get; init; }
}

/// <summary>A controller moved: a pedal, button, knob or wheel.</summary>
public class ControlChangeEventArgs : EventArgs
{
    /// <summary>0-based, like <see cref="NoteEventArgs.Channel"/>.</summary>
    public int Channel { get; init; }
    public int Controller { get; init; }
    public int Value { get; init; }
}

/// <summary>A program change, which some pads and buttons send instead of a note or controller.</summary>
public class ProgramChangeEventArgs : EventArgs
{
    /// <summary>0-based, like <see cref="NoteEventArgs.Channel"/>.</summary>
    public int Channel { get; init; }

    /// <summary>0-based, as sent. Keyboards usually label it one higher.</summary>
    public int Program { get; init; }
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

    // Defaulted so test doubles written before them compile unchanged, and simply never raise them.
    event EventHandler<ControlChangeEventArgs>? ControlChange { add { } remove { } }
    event EventHandler<ProgramChangeEventArgs>? ProgramChange { add { } remove { } }
}
