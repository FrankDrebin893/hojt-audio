using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

public partial class ChatViewModel : ObservableObject
{
    private readonly IChatService _chatService;
    private readonly Func<MusicContext> _getMusicContext;
    private CancellationTokenSource? _cancellationTokenSource;

    [ObservableProperty]
    private string _inputText = "";

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private bool _isPanelExpanded;

    [ObservableProperty]
    private string _errorMessage = "";

    public ObservableCollection<ChatMessage> Messages { get; } = new();

    public bool IsConfigured => _chatService.IsConfigured;

    public ChatViewModel(IChatService chatService, Func<MusicContext> getMusicContext)
    {
        _chatService = chatService;
        _getMusicContext = getMusicContext;
    }

    public void Configure(string apiKey)
    {
        _chatService.Configure(apiKey);
        OnPropertyChanged(nameof(IsConfigured));
    }

    [RelayCommand(CanExecute = nameof(CanSend))]
    private async Task SendAsync()
    {
        if (string.IsNullOrWhiteSpace(InputText)) return;

        var userMessage = InputText.Trim();
        InputText = "";
        ErrorMessage = "";

        Messages.Add(new ChatMessage { Role = "user", Content = userMessage });

        IsLoading = true;
        _cancellationTokenSource = new CancellationTokenSource();

        try
        {
            var context = _getMusicContext();
            var history = Messages
                .Take(Messages.Count - 1)
                .Select(m => new ChatMessageDto(m.Role, m.Content))
                .ToList();

            var response = await _chatService.SendMessageAsync(
                userMessage,
                history,
                context,
                _cancellationTokenSource.Token);

            Messages.Add(new ChatMessage { Role = "assistant", Content = response });
        }
        catch (OperationCanceledException)
        {
            // User cancelled
        }
        catch (Exception ex)
        {
            ErrorMessage = ex.Message;
        }
        finally
        {
            IsLoading = false;
            _cancellationTokenSource?.Dispose();
            _cancellationTokenSource = null;
        }
    }

    private bool CanSend() => !IsLoading && !string.IsNullOrWhiteSpace(InputText) && IsConfigured;

    partial void OnInputTextChanged(string value) => SendCommand.NotifyCanExecuteChanged();
    partial void OnIsLoadingChanged(bool value) => SendCommand.NotifyCanExecuteChanged();

    [RelayCommand]
    private void TogglePanel() => IsPanelExpanded = !IsPanelExpanded;

    [RelayCommand]
    private void ClearHistory()
    {
        Messages.Clear();
        ErrorMessage = "";
    }

    [RelayCommand]
    private void Cancel()
    {
        _cancellationTokenSource?.Cancel();
    }
}
