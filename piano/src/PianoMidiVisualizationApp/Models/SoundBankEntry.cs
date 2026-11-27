using System.IO;
using System.Text.Json.Serialization;

namespace PianoMidiVisualizationApp.Models;

/// <summary>
/// One piano soundbank (.sf2 or .sfz) in the user's soundbank library.
/// </summary>
public class SoundBankEntry
{
    public string Name { get; set; } = string.Empty;
    public string FilePath { get; set; } = string.Empty;

    [JsonIgnore]
    public string Format => Path.GetExtension(FilePath).ToLowerInvariant() == ".sfz" ? "SFZ" : "SF2";

    [JsonIgnore]
    public string FileName => string.IsNullOrEmpty(FilePath) ? string.Empty : Path.GetFileName(FilePath);

    public override string ToString() => string.IsNullOrWhiteSpace(Name) ? FileName : Name;
}
