using System.Net;
using System.Text;
using PianoMidiVisualizationApp.Services;
using Xunit;

namespace UpdateCheckTests;

/// <summary>Answers every request with one canned response and remembers the requests it got.</summary>
internal sealed class FakeHandler(Func<HttpRequestMessage, HttpResponseMessage> respond) : HttpMessageHandler
{
    public List<HttpRequestMessage> Requests { get; } = [];

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancel)
    {
        Requests.Add(request);
        return Task.FromResult(respond(request));
    }

    public static FakeHandler Json(string json, HttpStatusCode status = HttpStatusCode.OK) =>
        new(_ => new HttpResponseMessage(status) { Content = new StringContent(json, Encoding.UTF8, "application/json") });
}

public class VersionTests
{
    [Theory]
    [InlineData("piano-v1.2.3", 1, 2, 3)]
    [InlineData("piano-v0.10.0", 0, 10, 0)]
    public void A_piano_tag_has_a_version(string tag, int major, int minor, int patch) =>
        Assert.Equal(new Version(major, minor, patch), UpdateCheck.ParseTag(tag));

    [Theory]
    [InlineData("v1.2.3")]                    // a plugin release
    [InlineData("piano-v1.2.3-rc.1")]         // pre-release
    [InlineData("piano-v1.2")]                // missing part
    [InlineData("piano-v1.2.x")]
    [InlineData("piano-1.2.3")]
    [InlineData("piano-v99999999999.0.0")]    // too big to be a version
    [InlineData("")]
    [InlineData(null)]
    public void Anything_else_is_not_a_piano_version(string? tag) =>
        Assert.Null(UpdateCheck.ParseTag(tag));

    [Theory]
    [InlineData("1.2.3", 1, 2, 3, false)]
    [InlineData("1.2.3.0", 1, 2, 3, false)]
    [InlineData("1.2.3+abcdef", 1, 2, 3, false)]
    [InlineData("0.0.0-ci.17", 0, 0, 0, true)]
    [InlineData("1.2.3-rc.1+abc", 1, 2, 3, true)]
    public void The_running_version_is_read_as_the_assembly_reports_it(string running, int major, int minor, int patch, bool pre)
    {
        var parsed = UpdateCheck.ParseRunning(running);
        Assert.NotNull(parsed);
        Assert.Equal(new Version(major, minor, patch), parsed.Value.Core);
        Assert.Equal(pre, parsed.Value.PreRelease);
    }

    [Theory]
    [InlineData("")]
    [InlineData("garbage")]
    [InlineData("1.2")]
    [InlineData(null)]
    public void A_malformed_running_version_is_never_older(string? running)
    {
        Assert.Null(UpdateCheck.ParseRunning(running));
        Assert.False(UpdateCheck.IsNewer(new Version(9, 9, 9), running));
    }

    [Theory]
    [InlineData("1.2.4", "1.2.3", true)]
    [InlineData("1.10.0", "1.9.0", true)]       // numbers, not text
    [InlineData("1.2.3", "1.2.3", false)]
    [InlineData("1.2.2", "1.2.3", false)]
    [InlineData("1.2.3", "1.2.3-ci.5", true)]   // a CI build is below its release
    [InlineData("1.2.3", "1.2.4-ci.5", false)]
    public void Newer_means_a_higher_number(string latest, string running, bool newer) =>
        Assert.Equal(newer, UpdateCheck.IsNewer(Version.Parse(latest), running));
}

public class GatingTests
{
    private static readonly DateTime Now = new(2026, 11, 13, 9, 0, 0, DateTimeKind.Utc);

    [Fact] public void Off_is_never_due() => Assert.False(UpdateCheck.IsDue(false, null, Now));
    [Fact] public void Never_checked_is_due() => Assert.True(UpdateCheck.IsDue(true, null, Now));
    [Fact] public void An_hour_ago_is_not_due() => Assert.False(UpdateCheck.IsDue(true, Now.AddHours(-1), Now));
    [Fact] public void A_day_ago_is_due() => Assert.True(UpdateCheck.IsDue(true, Now.AddDays(-1), Now));
    [Fact] public void A_last_check_in_the_future_is_due() => Assert.True(UpdateCheck.IsDue(true, Now.AddDays(3), Now));

    private static Func<Task<FetchResult>> Counting(FetchResult result, Action onCall) =>
        () => { onCall(); return Task.FromResult(result); };

    private static readonly UpdateInfo V120 = new(new Version(1, 2, 0), "piano-v1.2.0");

    [Fact]
    public async Task Off_makes_no_request_and_shows_nothing()
    {
        int calls = 0;
        var memory = new UpdateMemory(false, null, "1.2.0");
        var outcome = await UpdateCheck.RunAsync(memory, "1.0.0", Now, Counting(new(true, V120), () => calls++));
        Assert.Equal(0, calls);
        Assert.Null(outcome.Available);
        Assert.Equal(memory, outcome.Memory);
    }

    [Fact]
    public async Task A_due_check_asks_once_and_remembers_the_newer_version()
    {
        int calls = 0;
        var outcome = await UpdateCheck.RunAsync(new(true, null, null), "1.0.0", Now, Counting(new(true, V120), () => calls++));
        Assert.Equal(1, calls);
        Assert.True(outcome.RequestMade);
        Assert.Equal("piano-v1.2.0", outcome.Available?.Tag);
        Assert.Equal(Now, outcome.Memory.LastCheckUtc);
        Assert.Equal("1.2.0", outcome.Memory.LatestKnownVersion);
    }

    [Fact]
    public async Task A_check_made_today_asks_nothing_but_still_shows_what_was_found()
    {
        int calls = 0;
        var memory = new UpdateMemory(true, Now.AddHours(-2), "1.2.0");
        var outcome = await UpdateCheck.RunAsync(memory, "1.0.0", Now, Counting(new(true, null), () => calls++));
        Assert.Equal(0, calls);
        Assert.False(outcome.RequestMade);
        Assert.Equal("1.2.0", outcome.Available?.DisplayVersion);
    }

    [Fact]
    public async Task A_known_version_the_app_has_since_caught_up_with_is_not_shown()
    {
        var memory = new UpdateMemory(true, Now.AddHours(-2), "1.2.0");
        var outcome = await UpdateCheck.RunAsync(memory, "1.2.0", Now, Counting(new(true, null), () => { }));
        Assert.Null(outcome.Available);
    }

    [Fact]
    public async Task A_failed_request_is_silent_counts_as_the_days_try_and_keeps_what_was_known()
    {
        var memory = new UpdateMemory(true, Now.AddDays(-2), "1.2.0");
        var outcome = await UpdateCheck.RunAsync(memory, "1.0.0", Now, Counting(new(false, null), () => { }));
        Assert.True(outcome.RequestMade);
        Assert.Equal(Now, outcome.Memory.LastCheckUtc);
        Assert.Equal("1.2.0", outcome.Memory.LatestKnownVersion);
    }

    [Fact]
    public async Task An_answer_with_nothing_newer_clears_what_was_known()
    {
        var memory = new UpdateMemory(true, Now.AddDays(-2), "1.2.0");
        var outcome = await UpdateCheck.RunAsync(memory, "1.0.0", Now, Counting(new(true, null), () => { }));
        Assert.Null(outcome.Available);
        Assert.Null(outcome.Memory.LatestKnownVersion);
    }
}

public class RequestTests
{
    private const string List = """
        [
          {"tag_name":"v0.3.0","draft":false,"prerelease":false},
          {"tag_name":"piano-v1.1.0","draft":false,"prerelease":false},
          {"tag_name":"piano-v1.3.0","draft":true,"prerelease":false},
          {"tag_name":"piano-v1.4.0-rc.1","draft":false,"prerelease":true},
          {"tag_name":"piano-v1.2.0","draft":false,"prerelease":false},
          {"tag_name":"piano-vbad","draft":false,"prerelease":false},
          {"nothing":"here"}
        ]
        """;

    [Fact]
    public async Task The_newest_published_piano_release_is_picked_from_the_list()
    {
        var result = await UpdateCheck.FetchAsync(FakeHandler.Json(List), "1.0.0");
        Assert.True(result.Answered);
        Assert.Equal("piano-v1.2.0", result.Newer?.Tag);
        Assert.Equal("https://github.com/FrankDrebin893/hojt-audio/releases/tag/piano-v1.2.0", result.Newer?.DownloadUrl);
    }

    [Fact]
    public async Task Nothing_is_offered_when_the_running_version_is_the_newest()
    {
        var result = await UpdateCheck.FetchAsync(FakeHandler.Json(List), "1.2.0");
        Assert.True(result.Answered);
        Assert.Null(result.Newer);
    }

    [Fact]
    public async Task The_request_is_a_plain_get_that_names_the_app_and_its_core_version_only()
    {
        var handler = FakeHandler.Json("[]");
        await UpdateCheck.FetchAsync(handler, "1.0.0+9f8e7d6");

        var request = Assert.Single(handler.Requests);
        Assert.Equal(HttpMethod.Get, request.Method);
        Assert.Equal(UpdateCheck.ReleasesUrl, request.RequestUri!.ToString());
        Assert.Equal("PianoMidiVisualization/1.0.0", request.Headers.UserAgent.ToString());
        Assert.Null(request.Content);
        Assert.False(request.Headers.Contains("Authorization"));
        Assert.False(request.Headers.Contains("Cookie"));
    }

    [Theory]
    [InlineData(HttpStatusCode.Forbidden, "{\"message\":\"rate limit\"}")]
    [InlineData(HttpStatusCode.InternalServerError, "")]
    [InlineData(HttpStatusCode.OK, "not json")]
    [InlineData(HttpStatusCode.OK, "{\"an\":\"object\"}")]
    public async Task A_bad_answer_is_not_an_answer_and_never_throws(HttpStatusCode status, string body)
    {
        var result = await UpdateCheck.FetchAsync(FakeHandler.Json(body, status), "1.0.0");
        Assert.False(result.Answered);
        Assert.Null(result.Newer);
    }

    [Fact]
    public async Task A_network_error_is_silent()
    {
        var handler = new FakeHandler(_ => throw new HttpRequestException("no network"));
        var result = await UpdateCheck.FetchAsync(handler, "1.0.0");
        Assert.False(result.Answered);
    }
}
