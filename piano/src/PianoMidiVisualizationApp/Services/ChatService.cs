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
    private const string Model = "gemini-2.0-flash";
    private const string ApiBaseUrl = "https://generativelanguage.googleapis.com/v1beta/models";

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
        var contents = BuildContents(history, userMessage, systemPrompt);

        var requestBody = new GeminiRequest
        {
            Contents = contents
        };

        var url = $"{ApiBaseUrl}/{Model}:generateContent?key={_apiKey}";

        using var request = new HttpRequestMessage(HttpMethod.Post, url);
        request.Content = JsonContent.Create(requestBody, options: JsonOptions);

        var response = await _httpClient.SendAsync(request, cancellationToken);

        if (!response.IsSuccessStatusCode)
        {
            var errorContent = await response.Content.ReadAsStringAsync(cancellationToken);
            throw new HttpRequestException($"API error ({response.StatusCode}): {errorContent}");
        }

        var result = await response.Content.ReadFromJsonAsync<GeminiResponse>(JsonOptions, cancellationToken);
        return result?.Candidates?.FirstOrDefault()?.Content?.Parts?.FirstOrDefault()?.Text ?? "";
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

    private static List<GeminiContent> BuildContents(IReadOnlyList<ChatMessageDto> history, string userMessage, string systemPrompt)
    {
        var contents = new List<GeminiContent>();

        // Add system context as first user message
        contents.Add(new GeminiContent
        {
            Role = "user",
            Parts = [new GeminiPart { Text = $"[System context - respond naturally to my questions]\n\n{systemPrompt}" }]
        });

        contents.Add(new GeminiContent
        {
            Role = "model",
            Parts = [new GeminiPart { Text = "I understand. I'm ready to help you with music theory, chord progressions, and piano practice. What would you like to know?" }]
        });

        // Add conversation history
        foreach (var msg in history)
        {
            contents.Add(new GeminiContent
            {
                Role = msg.Role == "user" ? "user" : "model",
                Parts = [new GeminiPart { Text = msg.Content }]
            });
        }

        // Add current user message
        contents.Add(new GeminiContent
        {
            Role = "user",
            Parts = [new GeminiPart { Text = userMessage }]
        });

        return contents;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    private class GeminiRequest
    {
        public List<GeminiContent> Contents { get; set; } = new();
    }

    private class GeminiContent
    {
        public required string Role { get; set; }
        public List<GeminiPart> Parts { get; set; } = new();
    }

    private class GeminiPart
    {
        public string? Text { get; set; }
    }

    private class GeminiResponse
    {
        public List<GeminiCandidate>? Candidates { get; set; }
    }

    private class GeminiCandidate
    {
        public GeminiContent? Content { get; set; }
    }
}
