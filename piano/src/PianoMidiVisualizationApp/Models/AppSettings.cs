using System.IO;
using System.Text.Json;

namespace PianoMidiVisualizationApp.Models;

public class AppSettings
{
    public string? LastMidiDevice { get; set; }
    public string? LastAudioDriver { get; set; }
    public bool UseAsio { get; set; } = true;
    public string? SoundFontPath { get; set; }
    public float Volume { get; set; } = 0.8f;
    public string? AnthropicApiKey { get; set; }

    // Panel visibility. These defaults ARE the first-launch zen layout, and they also apply to
    // an existing settings.json written before these keys existed — Deserialize runs the
    // parameterless constructor and only overwrites keys actually present in the file.
    public bool ShowMidiLog { get; set; } = false;
    public bool ShowChatPanel { get; set; } = false;
    public bool ShowProgression { get; set; } = true;
    public bool ShowStatusBar { get; set; } = true;

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
