using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.Models;

public class AppSettings
{
    public string? LastMidiDevice { get; set; }
    public string? LastAudioDriver { get; set; }
    public bool UseAsio { get; set; } = true;
    public string? SoundFontPath { get; set; }
    public float Volume { get; set; } = 0.8f;
    public string? AnthropicApiKey { get; set; }

    // Key highlighted on the keyboard. The scale is stored as its enum name so the file stays
    // readable; read it through ResolveKeyScale, which also handles files from before it existed.
    public bool KeyHighlightEnabled { get; set; }
    public int KeyTonicPitchClass { get; set; }
    public string? KeyScale { get; set; }
    public bool MuteOutOfKeyNotes { get; set; }

    /// <summary>
    /// The pre-scale-types setting ("Major" or "Minor"). Only ever read, to migrate an old file;
    /// it is never assigned on save, so being null keeps it out of every file written since.
    /// </summary>
    [JsonPropertyName("KeyQuality")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? LegacyKeyQuality { get; set; }

    /// <summary>
    /// The saved scale. A file without KeyScale predates it, so its old major/minor choice maps
    /// across; a KeyScale that is present but unrecognised falls back to Major, never throws.
    /// </summary>
    public ScaleType ResolveKeyScale()
    {
        if (KeyScale is null)
            return LegacyKeyQuality == "Minor" ? ScaleType.NaturalMinor : ScaleType.Major;

        // Enum.TryParse also accepts "3" or "99"; only a defined member's name counts here.
        return Enum.TryParse<ScaleType>(KeyScale, out var scale)
               && Enum.IsDefined(scale)
               && !int.TryParse(KeyScale, out _)
            ? scale
            : ScaleType.Major;
    }

    // Metronome. Whether it is running is deliberately not saved: a click that starts by
    // itself on launch would be a surprise. Out-of-range values are clamped on load.
    public int MetronomeBpm { get; set; } = 90;
    public int MetronomeBeatsPerBar { get; set; } = 4;
    public float MetronomeVolume { get; set; } = 0.7f;

    // Panel visibility. These defaults ARE the first-launch zen layout, and they also apply to
    // an existing settings.json written before these keys existed — Deserialize runs the
    // parameterless constructor and only overwrites keys actually present in the file.
    public bool ShowMidiLog { get; set; } = false;
    public bool ShowChatPanel { get; set; } = false;
    public bool ShowProgression { get; set; } = true;
    public bool ShowStatusBar { get; set; } = true;
    public bool ShowRecorder { get; set; } = false;
    public bool ShowCircleOfFifths { get; set; } = true;
    public bool ShowGrandStaff { get; set; } = true;
    public bool ShowSongPractice { get; set; } = false;
    public bool ShowChordStrip { get; set; } = true;

    // Diatonic chord strip: seventh chords rather than triads.
    public bool ChordStripSevenths { get; set; }

    // Song practice: the song reopens on launch while practice is on. Speed is clamped on load.
    public string? SongPath { get; set; }
    public double SongSpeed { get; set; } = 1.0;
    public string SongMode { get; set; } = "Wait";

    // Window placement. Nullable so "never saved" is distinguishable from 0.
    public double? WindowLeft { get; set; }
    public double? WindowTop { get; set; }
    public double? WindowWidth { get; set; }
    public double? WindowHeight { get; set; }

    private static string DirectoryPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "PianoMidiVisualizationApp");

    private static string FilePath => Path.Combine(DirectoryPath, "settings.json");

    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(FilePath))
            {
                var json = File.ReadAllText(FilePath);
                return JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            }
        }
        catch
        {
            // If loading fails, return defaults
        }
        return new AppSettings();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(DirectoryPath);
            var json = JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(FilePath, json);
        }
        catch
        {
            // Silently fail on save errors
        }
    }
}
