using System.Globalization;
using System.Net.Http;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace PianoMidiVisualizationApp.Services;

/// <summary>A newer piano release than the running one: its version and the tag it was published under.</summary>
public sealed record UpdateInfo(Version Version, string Tag)
{
    /// <summary>"1.2.0", for the line the player reads.</summary>
    public string DisplayVersion => Version.ToString(3);

    /// <summary>
    /// The release's page, built from the tag and never taken from the answer: the one thing the
    /// check ever hands the player is a link into this repository's releases.
    /// </summary>
    public string DownloadUrl => UpdateCheck.ReleasePageBase + Uri.EscapeDataString(Tag);
}

/// <summary>What the app remembers of its update check between launches (kept in the settings file).</summary>
public sealed record UpdateMemory(bool Enabled, DateTime? LastCheckUtc, string? LatestKnownVersion);

/// <summary>What one request found: whether GitHub answered with a readable list, and the newer release in it, if any.</summary>
public sealed record FetchResult(bool Answered, UpdateInfo? Newer);

/// <summary>What a startup check leaves: the memory to save and the release to tell the player about.</summary>
public sealed record UpdateOutcome(UpdateMemory Memory, UpdateInfo? Available, bool RequestMade);

/// <summary>
/// The piano viz's update check: once a day at startup, one plain request for this repository's
/// releases, the newest <c>piano-vX.Y.Z</c> compared with the running version, a link and nothing
/// more. Nothing but the request is sent (its User-Agent names the app and its version, which is
/// what GitHub's API asks for), and a failure of any kind is silent. No WPF in here, so the tests
/// run it with a fake handler.
/// </summary>
public static class UpdateCheck
{
    /// <summary>
    /// The plugins' <c>releases/latest</c> answers for the plugins only (the piano's releases are
    /// never marked latest), so the check reads the list and picks the piano tags out of it.
    /// </summary>
    public const string ReleasesUrl = "https://api.github.com/repos/FrankDrebin893/hojt-audio/releases?per_page=100";

    public const string ReleasePageBase = "https://github.com/FrankDrebin893/hojt-audio/releases/tag/";

    public static readonly TimeSpan Interval = TimeSpan.FromDays(1);

    private static readonly Regex PianoTag = new(@"^piano-v(\d+)\.(\d+)\.(\d+)$", RegexOptions.CultureInvariant);

    // A running version as the assembly reports it: 1.2.3, 1.2.3.0, 1.2.3-ci.7, 1.2.3+commit.
    private static readonly Regex Running =
        new(@"^v?(\d+)\.(\d+)\.(\d+)(?:\.\d+)?(-[0-9A-Za-z.-]+)?(\+.*)?$", RegexOptions.CultureInvariant);

    /// <summary>
    /// The version of a <c>piano-vX.Y.Z</c> tag. Null for anything else: a plugin's <c>vX.Y.Z</c>,
    /// a pre-release suffix, a missing part, a number too large to be a version.
    /// </summary>
    public static Version? ParseTag(string? tag)
    {
        if (tag is null) return null;
        var m = PianoTag.Match(tag);
        return m.Success ? Build(m) : null;
    }

    /// <summary>
    /// The running version: the core number, and whether it carries a pre-release suffix (a CI
    /// build, "0.0.0-ci.7"), which by semver sorts below the release of the same number.
    /// </summary>
    public static (Version Core, bool PreRelease)? ParseRunning(string? version)
    {
        if (version is null) return null;
        var m = Running.Match(version.Trim());
        if (!m.Success) return null;
        var core = Build(m);
        return core is null ? null : (core, m.Groups[4].Success);
    }

    private static Version? Build(Match m)
    {
        // A part that is not a small number is malformed, not a huge version.
        if (!int.TryParse(m.Groups[1].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int major)
            || !int.TryParse(m.Groups[2].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int minor)
            || !int.TryParse(m.Groups[3].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int patch))
            return null;
        return new Version(major, minor, patch);
    }

    /// <summary>Whether <paramref name="latest"/> is newer than the running version. An unreadable running version is never older.</summary>
    public static bool IsNewer(Version latest, string? running)
    {
        if (ParseRunning(running) is not { } parsed) return false;
        int cmp = latest.CompareTo(parsed.Core);
        return cmp > 0 || (cmp == 0 && parsed.PreRelease);
    }

    /// <summary>
    /// Whether a request is due: the check is on and none was made in the last day. A last check
    /// in the future (the clock was set back) counts as due, so it can never block the check for good.
    /// </summary>
    public static bool IsDue(bool enabled, DateTime? lastCheckUtc, DateTime nowUtc) =>
        enabled && (lastCheckUtc is not { } last || last > nowUtc || nowUtc - last >= Interval);

    /// <summary>
    /// The newest release in GitHub's list that is a published, final <c>piano-vX.Y.Z</c> and newer
    /// than <paramref name="running"/>. Null when there is none; throws on JSON that is not a list,
    /// which <see cref="FetchAsync"/> turns into "not answered".
    /// </summary>
    public static UpdateInfo? PickNewer(string json, string? running)
    {
        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.ValueKind != JsonValueKind.Array)
            throw new JsonException("The releases answer is not a list.");

        UpdateInfo? best = null;
        foreach (var release in doc.RootElement.EnumerateArray())
        {
            if (release.ValueKind != JsonValueKind.Object) continue;
            if (Flag(release, "draft") || Flag(release, "prerelease")) continue;
            if (!release.TryGetProperty("tag_name", out var tagElement) || tagElement.ValueKind != JsonValueKind.String) continue;

            string tag = tagElement.GetString()!;
            if (ParseTag(tag) is not { } version) continue;
            if (best is null || version > best.Version) best = new UpdateInfo(version, tag);
        }

        return best is not null && IsNewer(best.Version, running) ? best : null;
    }

    private static bool Flag(JsonElement release, string name) =>
        release.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.True;

    /// <summary>
    /// The one request. Never throws: a network error, a status other than 200, a timeout or an
    /// unreadable body all come back as "not answered".
    /// </summary>
    public static async Task<FetchResult> FetchAsync(HttpMessageHandler handler, string running, CancellationToken cancel = default)
    {
        try
        {
            using var client = new HttpClient(handler, disposeHandler: false)
            {
                Timeout = TimeSpan.FromSeconds(10),
                MaxResponseContentBufferSize = 2 * 1024 * 1024,
            };

            using var request = new HttpRequestMessage(HttpMethod.Get, ReleasesUrl);
            request.Headers.TryAddWithoutValidation("User-Agent", "PianoMidiVisualization/" + UserAgentVersion(running));
            request.Headers.TryAddWithoutValidation("Accept", "application/vnd.github+json");

            using var response = await client.SendAsync(request, cancel).ConfigureAwait(false);
            if (!response.IsSuccessStatusCode) return new FetchResult(false, null);

            string json = await response.Content.ReadAsStringAsync(cancel).ConfigureAwait(false);
            return new FetchResult(true, PickNewer(json, running));
        }
        catch (Exception)
        {
            return new FetchResult(false, null);
        }
    }

    /// <summary>The running version for a User-Agent: its core number, so no build metadata leaves the machine.</summary>
    public static string UserAgentVersion(string? running) =>
        ParseRunning(running) is { } parsed ? parsed.Core.ToString(3) : "0.0.0";

    /// <summary>
    /// A startup check. Off, it does nothing at all. On, it asks only when a day has passed since
    /// the last request (a failed request counts, so GitHub is never asked twice in a day);
    /// otherwise it tells about the newer version found earlier, if the running one is still older.
    /// </summary>
    /// <param name="fetch">Makes the request; the app's passes <see cref="FetchAsync"/> over a real handler, the tests a fake.</param>
    public static async Task<UpdateOutcome> RunAsync(
        UpdateMemory memory, string running, DateTime nowUtc, Func<Task<FetchResult>> fetch)
    {
        if (!memory.Enabled) return new UpdateOutcome(memory, null, false);

        string? known = memory.LatestKnownVersion;
        var current = memory;
        bool requested = false;

        if (IsDue(memory.Enabled, memory.LastCheckUtc, nowUtc))
        {
            requested = true;
            var result = await fetch().ConfigureAwait(false);
            current = current with { LastCheckUtc = nowUtc };
            if (result.Answered)
                known = result.Newer?.Version.ToString(3);   // nothing newer clears what was known
        }

        current = current with { LatestKnownVersion = known };

        UpdateInfo? available = null;
        if (known is not null && ParseTag("piano-v" + known) is { } version && IsNewer(version, running))
            available = new UpdateInfo(version, "piano-v" + known);

        return new UpdateOutcome(current, available, requested);
    }
}
