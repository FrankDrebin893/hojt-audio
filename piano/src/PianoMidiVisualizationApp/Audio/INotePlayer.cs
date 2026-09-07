using NAudio.Wave;

namespace PianoMidiVisualizationApp.Audio;

public interface INotePlayer : ISampleProvider
{
    void NoteOn(int channel, int note, int velocity);
    void NoteOff(int channel, int note);
}
