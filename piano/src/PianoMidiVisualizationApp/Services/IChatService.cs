namespace PianoMidiVisualizationApp.Services;

public record MusicContext(
    string? CurrentChord,
    IReadOnlyList<string> SavedChords,
    IReadOnlyList<string> RecentMidiActivity);

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
