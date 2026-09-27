using PianoMidiVisualizationApp.Audio;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Models;

namespace ReadmeScreenshots;

/// <summary>
/// A MIDI keyboard the scenes play by hand. It raises the same messages, in the same words, as
/// <see cref="MidiInputService"/>, so the MIDI log and status bar read as they do for real.
/// </summary>
internal sealed class FakeMidiInput : IMidiInputService
{
    private static readonly string[] NoteNames = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

    public IReadOnlyList<DeviceInfo> GetAvailableDevices() => [new DeviceInfo(0, "Digital Piano")];

    public void Open(int deviceIndex) => IsOpen = true;
    public void Close() => IsOpen = false;
    public bool IsOpen { get; private set; }

    public event EventHandler<NoteEventArgs>? NoteOn;
    public event EventHandler<NoteEventArgs>? NoteOff;
    public event EventHandler<RawMidiMessageEventArgs>? MessageReceived;
    public event EventHandler<ControlChangeEventArgs>? ControlChange;

    /// <param name="channel">0-based, like <see cref="NoteEventArgs.Channel"/>: 9 is a drum pad's channel 10.</param>
    public void PressKey(int note, int velocity, int channel = 0)
    {
        Raw("NoteOn", channel, 0x90 | channel | note << 8 | velocity << 16);
        // Middle C (60) is "C4", as the keyboard and the real log (MusicNaming.WithOctave) name it.
        Log($"NoteOn Ch{channel + 1} Note={note} ({NoteNames[note % 12]}{note / 12 - 1}) Vel={velocity}");
        NoteOn?.Invoke(this, new NoteEventArgs { NoteNumber = note, Velocity = velocity, Channel = channel });
    }

    public void ReleaseKey(int note, int channel = 0)
    {
        Raw("NoteOff", channel, 0x80 | channel | note << 8 | 0x40 << 16);
        Log($"NoteOff Ch{channel + 1} Note={note}");
        NoteOff?.Invoke(this, new NoteEventArgs { NoteNumber = note, Velocity = 0, Channel = channel });
    }

    /// <summary>A pedal, button or knob: 127 is down or full, 0 up.</summary>
    public void MoveControl(int controller, int value, int channel = 0)
    {
        Raw("ControlChange", channel, 0xB0 | channel | controller << 8 | value << 16);
        Log($"CC Ch{channel + 1} Controller={controller} Value={value}");
        ControlChange?.Invoke(this, new ControlChangeEventArgs { Channel = channel, Controller = controller, Value = value });
    }

    private void Raw(string command, int channel, int message) => Log($"Ch{channel + 1} {command} raw=0x{message:X8}");

    private void Log(string description) =>
        MessageReceived?.Invoke(this, new RawMidiMessageEventArgs { Description = description });

    public void Dispose() { }
}

/// <summary>Silent: every call succeeds, nothing is opened or played.</summary>
internal sealed class FakeAudioEngine : IAudioEngine
{
    public IReadOnlyList<string> GetAsioDriverNames() => ["ASIO4ALL v2"];
    public IReadOnlyList<string> GetWasapiDeviceNames() => ["Speakers (High Definition Audio)"];

    public void Initialize(string driverName, bool useAsio, string soundFontPath) { }
    public void Start() => IsRunning = true;
    public void Stop() => IsRunning = false;
    public void NoteOn(int channel, int note, int velocity) { }
    public void NoteOff(int channel, int note) { }

    public float Volume { get; set; }
    public bool IsRunning { get; private set; }

    public void Dispose() { }
}
