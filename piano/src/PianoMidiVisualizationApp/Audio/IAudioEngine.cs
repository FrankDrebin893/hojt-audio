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
    float Volume { get; set; }
    bool IsRunning { get; }
}
