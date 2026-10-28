using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

public interface INotePlayer : ISampleProvider
{
    void NoteOn(int channel, int note, int velocity);
    void NoteOff(int channel, int note);

    /// <summary>
    /// The sustain pedal (CC64) on a 0-based channel. While it is down a note-off leaves the
    /// note ringing; lifting it releases every note it was holding whose key is up.
    /// </summary>
    void SetSustainPedal(int channel, bool isDown);
}
