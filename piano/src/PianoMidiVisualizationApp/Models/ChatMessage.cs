namespace PianoMidiVisualizationApp.Models;

public class ChatMessage
{
    public required string Role { get; init; }
    public required string Content { get; init; }
    public DateTime Timestamp { get; init; } = DateTime.Now;

    public bool IsUser => Role == "user";
    public bool IsAssistant => Role == "assistant";
}
