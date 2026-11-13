using System.Diagnostics;
using System.Net.Http;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>
/// The one quiet line in the title bar that says a newer piano viz exists, with a link to its
/// download page and a switch that turns the check off for good. The check itself is
/// <see cref="UpdateCheck"/>; this only holds what the window shows and what the settings file
/// remembers. Nothing here starts a request: <see cref="CheckAtStartupAsync"/> is called by
/// the app, so the screenshot harness and the tests never reach the network.
/// </summary>
public partial class UpdateNoticeViewModel : ObservableObject
{
    private readonly Action<string> _openUrl;
    private DateTime? _lastCheckUtc;
    private string? _latestKnownVersion;

    public UpdateNoticeViewModel(Action<string>? openUrl = null) => _openUrl = openUrl ?? OpenInBrowser;

    /// <summary>The check is on. Persisted; the View menu's "Check for Updates" and the line's "Turn off" both set it.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsVisible))]
    private bool _checkEnabled = true;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsVisible))]
    [NotifyPropertyChangedFor(nameof(Text))]
    private UpdateInfo? _available;

    /// <summary>Hidden for this run only, by the line's close; the next launch shows it again.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsVisible))]
    private bool _dismissed;

    public bool IsVisible => CheckEnabled && Available is not null && !Dismissed;

    public string Text => Available is { } info ? $"Version {info.DisplayVersion} is out" : "";

    /// <summary>Shows the line for <paramref name="info"/>. What the check calls, and what the harness calls with a made-up version.</summary>
    public void Show(UpdateInfo info)
    {
        Dismissed = false;
        Available = info;
    }

    [RelayCommand]
    private void OpenDownload()
    {
        if (Available is { } info) _openUrl(info.DownloadUrl);
    }

    [RelayCommand]
    private void TurnOff() => CheckEnabled = false;

    [RelayCommand]
    private void Dismiss() => Dismissed = true;

    public void ApplyFrom(AppSettings saved)
    {
        CheckEnabled = saved.CheckForUpdates;
        _lastCheckUtc = saved.LastUpdateCheckUtc;
        _latestKnownVersion = saved.LatestKnownVersion;
    }

    public void CaptureInto(AppSettings saved)
    {
        saved.CheckForUpdates = CheckEnabled;
        saved.LastUpdateCheckUtc = _lastCheckUtc;
        saved.LatestKnownVersion = _latestKnownVersion;
    }

    /// <summary>
    /// The startup check, on a background task and silent about every failure. Call it from the UI
    /// thread; the line appears on that thread when the answer comes.
    /// </summary>
    public async Task CheckAtStartupAsync(string runningVersion)
    {
        try
        {
            var memory = new UpdateMemory(CheckEnabled, _lastCheckUtc, _latestKnownVersion);
            var outcome = await Task.Run(() => UpdateCheck.RunAsync(
                memory, runningVersion, DateTime.UtcNow,
                async () =>
                {
                    using var handler = new HttpClientHandler();
                    return await UpdateCheck.FetchAsync(handler, runningVersion);
                }));

            _lastCheckUtc = outcome.Memory.LastCheckUtc;
            _latestKnownVersion = outcome.Memory.LatestKnownVersion;

            // Turned off while the request was out: the answer is dropped.
            if (CheckEnabled && outcome.Available is { } info) Show(info);
        }
        catch (Exception)
        {
            // An update notice is never worth an error.
        }
    }

    private static void OpenInBrowser(string url)
    {
        try
        {
            Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        }
        catch (Exception)
        {
            // No browser to open: nothing to tell the player that they could act on.
        }
    }
}
