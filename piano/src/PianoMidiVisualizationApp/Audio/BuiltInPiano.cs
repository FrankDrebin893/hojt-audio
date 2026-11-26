namespace PianoMidiVisualizationApp.Audio;

/// <summary>
/// The piano that is part of the app: synthesized, so no sample file, no licence and nothing
/// to download. It sits in the soundbank list like a file would, under a path that no file
/// can have, so every place that keeps a soundbank path handles it without a second field.
/// </summary>
public static class BuiltInPiano
{
    /// <summary>Stored in settings.json as the soundbank path. Never the path of a real file.</summary>
    public const string Path = "builtin:stage-piano";

    public const string Name = "Højte Stage Piano";

    public static bool IsBuiltIn(string? path) =>
        string.Equals(path, Path, StringComparison.Ordinal);

    /// <summary>True when the sound can be played: the built-in piano, or a file that is there.</summary>
    public static bool Exists(string? path) =>
        !string.IsNullOrEmpty(path) && (IsBuiltIn(path) || System.IO.File.Exists(path));
}
