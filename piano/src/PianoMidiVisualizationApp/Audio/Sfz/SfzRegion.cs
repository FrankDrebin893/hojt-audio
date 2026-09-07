namespace PianoMidiVisualizationApp.Audio.Sfz;

public class SfzRegion
{
    public required string SamplePath { get; init; }
    public int LoKey { get; init; }
    public int HiKey { get; init; }
    public int LoVel { get; init; }
    public int HiVel { get; init; }
    public int PitchKeyCenter { get; init; }
    public double AmpVelTrack { get; init; } = 100;
    public double AmpegRelease { get; init; } = 0.1;

    public bool Matches(int note, int velocity) =>
        note >= LoKey && note <= HiKey && velocity >= LoVel && velocity <= HiVel;
}
