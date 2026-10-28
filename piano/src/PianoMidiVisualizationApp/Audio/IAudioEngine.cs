namespace PianoMidiVisualizationApp.Audio;

public interface IAudioEngine : IDisposable
{
    IReadOnlyList<string> GetAsioDriverNames();
    IReadOnlyList<string> GetWasapiDeviceNames();
    void Initialize(string driverName, bool useAsio, string soundFontPath);
    void Start();
    void Stop();
    void NoteOn(int channel, int note, int velocity);
    void NoteOff(int channel, int note);
    /// <summary>
    /// The sustain pedal (CC64) on a 0-based channel, as <see cref="INotePlayer.SetSustainPedal"/>.
    /// A freshly initialised engine starts with every pedal up. The default ignores it, so test
    /// doubles that predate the pedal compile unchanged.
    /// </summary>
    void SetSustainPedal(int channel, bool isDown) { }

    float Volume { get; set; }
    bool IsRunning { get; }

    /// <summary>
    /// The click mixed into the output, or null for an engine without one. The default
    /// keeps test doubles that predate the metronome compiling unchanged.
    /// </summary>
    MetronomeSampleProvider? Metronome => null;
}
