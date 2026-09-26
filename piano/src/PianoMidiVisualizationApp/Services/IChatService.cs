namespace PianoMidiVisualizationApp.Services;

/// <param name="CurrentChord">The held chord, with its Roman numeral when a key is selected: "G7 (V7)".</param>
/// <param name="SavedChords">The saved progression, each with its numeral in the same form.</param>
/// <param name="Key">The selected key, e.g. "A Harmonic Minor", or null for none.</param>
public record MusicContext(
    string? CurrentChord,
    IReadOnlyList<string> SavedChords,
    IReadOnlyList<string> RecentMidiActivity,
    string? Key = null);

public interface IChatService
{
    bool IsConfigured { get; }
    void Configure(string apiKey);
    Task<string> SendMessageAsync(
        string userMessage,
        IReadOnlyList<ChatMessageDto> history,
        MusicContext context,
        CancellationToken cancellationToken = default);
}

public record ChatMessageDto(string Role, string Content);
