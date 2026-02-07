using CommunityToolkit.Mvvm.ComponentModel;

namespace PianoMidiVisualizationApp.Models;

public partial class PianoKey : ObservableObject
{
    public int NoteNumber { get; init; }
    public string NoteName { get; init; } = "";
    public bool IsBlack { get; init; }

    [ObservableProperty]
    private bool _isPressed;

    [ObservableProperty]
    private int _velocity;
}
