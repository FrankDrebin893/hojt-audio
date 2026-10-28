namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzSampleData
{
    public required float[] Samples { get; init; }
    public required int Channels { get; init; }
    public required int SampleRate { get; init; }
}
