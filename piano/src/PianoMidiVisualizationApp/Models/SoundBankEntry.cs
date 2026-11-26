using System.IO;
using System.Text.Json.Serialization;
using PianoMidiVisualizationApp.Audio;

namespace PianoMidiVisualizationApp.Models;

/// <summary>
/// One piano soundbank (.sf2 or .sfz) in the user's soundbank library.
/// </summary>
public class SoundBankEntry
{
    public string Name { get; set; } = string.Empty;
    public string FilePath { get; set; } = string.Empty;

    [JsonIgnore]
    public string Format => BuiltInPiano.IsBuiltIn(FilePath) ? "Built-in"
        : Path.GetExtension(FilePath).ToLowerInvariant() == ".sfz" ? "SFZ" : "SF2";

    /// <summary>The piano that is part of the app: it has no file and cannot be removed.</summary>
    [JsonIgnore]
    public bool IsBuiltIn => BuiltInPiano.IsBuiltIn(FilePath);

    [JsonIgnore]
    public string FileName => string.IsNullOrEmpty(FilePath) ? string.Empty
        : BuiltInPiano.IsBuiltIn(FilePath) ? BuiltInPiano.Name : Path.GetFileName(FilePath);

    public override string ToString() => string.IsNullOrWhiteSpace(Name) ? FileName : Name;
}
