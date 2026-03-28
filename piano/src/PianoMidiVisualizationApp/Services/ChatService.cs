using System.Net.Http;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PianoMidiVisualizationApp.Services;

public class ChatService : IChatService
{
    private readonly HttpClient _httpClient;
    private string? _apiKey;
    private const string ApiUrl = "https://api.anthropic.com/v1/messages";
    private const string Model = "claude-sonnet-4-20250514";
    private const int MaxTokens = 1024;

    public ChatService()
    {
        _httpClient = new HttpClient();
    }

    public bool IsConfigured => !string.IsNullOrWhiteSpace(_apiKey);

    public void Configure(string apiKey)
    {
        _apiKey = string.IsNullOrWhiteSpace(apiKey) ? null : apiKey;
    }

    public async Task<string> SendMessageAsync(
        string userMessage,
        IReadOnlyList<ChatMessageDto> history,
        MusicContext context,
        CancellationToken cancellationToken = default)
    {
        if (!IsConfigured)
            throw new InvalidOperationException("Chat service is not configured. Please set your API key in settings.");

        var systemPrompt = BuildSystemPrompt(context);
        var messages = BuildMessages(history, userMessage);

        var requestBody = new AnthropicRequest
        {
            Model = Model,
            MaxTokens = MaxTokens,
            System = systemPrompt,
            Messages = messages
        };

        using var request = new HttpRequestMessage(HttpMethod.Post, ApiUrl);
        request.Headers.Add("x-api-key", _apiKey);
        request.Headers.Add("anthropic-version", "2023-06-01");
        request.Content = JsonContent.Create(requestBody, options: JsonOptions);

        var response = await _httpClient.SendAsync(request, cancellationToken);

        if (!response.IsSuccessStatusCode)
        {
            var errorContent = await response.Content.ReadAsStringAsync(cancellationToken);
            throw new HttpRequestException($"API error ({response.StatusCode}): {errorContent}");
        }

        var result = await response.Content.ReadFromJsonAsync<AnthropicResponse>(JsonOptions, cancellationToken);
        return result?.Content?.FirstOrDefault()?.Text ?? "";
    }

    private static string BuildSystemPrompt(MusicContext context)
    {
        var sb = new StringBuilder();
        sb.AppendLine("You are a friendly music assistant integrated into a piano MIDI visualization app.");
        sb.AppendLine("Help users with music theory, chord progressions, practice tips, and musical guidance.");
        sb.AppendLine("Keep responses concise but helpful - the user is likely practicing piano.");
        sb.AppendLine();
        sb.AppendLine("Current context from the app:");

        if (!string.IsNullOrEmpty(context.CurrentChord))
            sb.AppendLine($"- Currently playing chord: {context.CurrentChord}");
        else
            sb.AppendLine("- No chord currently playing");

        if (context.SavedChords.Count > 0)
            sb.AppendLine($"- Saved chord progression: {string.Join(" -> ", context.SavedChords)}");
        else
            sb.AppendLine("- No saved chord progression yet");

        if (context.RecentMidiActivity.Count > 0)
        {
            sb.AppendLine("- Recent MIDI activity:");
            foreach (var activity in context.RecentMidiActivity.TakeLast(5))
                sb.AppendLine($"  {activity}");
        }

        return sb.ToString();
    }

    private static List<AnthropicMessage> BuildMessages(IReadOnlyList<ChatMessageDto> history, string userMessage)
    {
        var messages = new List<AnthropicMessage>();

        foreach (var msg in history)
        {
            messages.Add(new AnthropicMessage
            {
                Role = msg.Role,
                Content = msg.Content
            });
        }

        messages.Add(new AnthropicMessage
        {
            Role = "user",
            Content = userMessage
        });

        return messages;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    private class AnthropicRequest
    {
        public required string Model { get; set; }
        public int MaxTokens { get; set; }
        public string? System { get; set; }
        public List<AnthropicMessage> Messages { get; set; } = new();
    }

    private class AnthropicMessage
    {
        public required string Role { get; set; }
        public required string Content { get; set; }
    }

    private class AnthropicResponse
    {
        public List<ContentBlock>? Content { get; set; }
    }

    private class ContentBlock
    {
        public string? Type { get; set; }
        public string? Text { get; set; }
    }
}
