using System.Globalization;
using System.IO;

namespace PianoMidiVisualizationApp.Audio.Sfz;

public static class SfzParser
{
    public static List<SfzRegion> Parse(string sfzPath)
    {
        var baseDirectory = Path.GetDirectoryName(Path.GetFullPath(sfzPath)) ?? "";
        var regions = new List<SfzRegion>();

        var groupOpcodes = new Dictionary<string, string>();
        Dictionary<string, string>? pendingRegionOpcodes = null;

        void FlushPendingRegion()
        {
            if (pendingRegionOpcodes != null)
            {
                var region = TryBuildRegion(pendingRegionOpcodes, baseDirectory);
                if (region != null)
                    regions.Add(region);
                pendingRegionOpcodes = null;
            }
        }

        foreach (var rawLine in File.ReadLines(sfzPath))
        {
            var line = StripComment(rawLine).Trim();
            if (line.Length == 0)
                continue;

            var pos = 0;
            while (pos < line.Length)
            {
                while (pos < line.Length && char.IsWhiteSpace(line[pos])) pos++;
                if (pos >= line.Length) break;

                if (line[pos] == '<')
                {
                    var end = line.IndexOf('>', pos);
                    if (end < 0) break;

                    var tag = line.Substring(pos + 1, end - pos - 1).Trim().ToLowerInvariant();
                    if (tag == "group")
                    {
                        FlushPendingRegion();
                        groupOpcodes = new Dictionary<string, string>();
                    }
                    else if (tag == "region")
                    {
                        FlushPendingRegion();
                        pendingRegionOpcodes = new Dictionary<string, string>(groupOpcodes);
                    }
                    else
                    {
                        FlushPendingRegion();
                    }

                    pos = end + 1;
                    continue;
                }

                var tokenStart = pos;
                while (pos < line.Length && !char.IsWhiteSpace(line[pos]) && line[pos] != '<') pos++;
                var token = line.Substring(tokenStart, pos - tokenStart);

                var eq = token.IndexOf('=');
                if (eq > 0)
                {
                    var key = token[..eq].ToLowerInvariant();
                    var value = token[(eq + 1)..];
                    var target = pendingRegionOpcodes ?? groupOpcodes;
                    target[key] = value;
                }
            }
        }

        FlushPendingRegion();
        return regions;
    }

    private static SfzRegion? TryBuildRegion(Dictionary<string, string> opcodes, string baseDirectory)
    {
        // Out of scope for the core playback engine: release-triggered samples (string
        // resonance, hammer noise) and CC64-triggered samples (sustain pedal noise).
        if (opcodes.TryGetValue("trigger", out var trigger) && trigger.Equals("release", StringComparison.OrdinalIgnoreCase))
            return null;
        if (opcodes.ContainsKey("on_locc64") || opcodes.ContainsKey("on_hicc64"))
            return null;

        if (!opcodes.TryGetValue("sample", out var sample) || sample.Length == 0)
            return null;

        var relativePath = sample.Replace('\\', Path.DirectorySeparatorChar).Replace('/', Path.DirectorySeparatorChar);
        var fullPath = Path.Combine(baseDirectory, relativePath);

        var loKey = GetInt(opcodes, "lokey", 0);
        var hiKey = GetInt(opcodes, "hikey", 127);

        return new SfzRegion
        {
            SamplePath = fullPath,
            LoKey = loKey,
            HiKey = hiKey,
            LoVel = GetInt(opcodes, "lovel", 0),
            HiVel = GetInt(opcodes, "hivel", 127),
            PitchKeyCenter = GetInt(opcodes, "pitch_keycenter", loKey),
            AmpVelTrack = GetDouble(opcodes, "amp_veltrack", 100),
            AmpegRelease = GetDouble(opcodes, "ampeg_release", 0.1)
        };
    }

    private static string StripComment(string line)
    {
        var idx = line.IndexOf("//", StringComparison.Ordinal);
        return idx >= 0 ? line[..idx] : line;
    }

    private static int GetInt(Dictionary<string, string> opcodes, string key, int fallback) =>
        opcodes.TryGetValue(key, out var v) && int.TryParse(v, NumberStyles.Integer, CultureInfo.InvariantCulture, out var n) ? n : fallback;

    private static double GetDouble(Dictionary<string, string> opcodes, string key, double fallback) =>
        opcodes.TryGetValue(key, out var v) && double.TryParse(v, NumberStyles.Float, CultureInfo.InvariantCulture, out var n) ? n : fallback;
}
