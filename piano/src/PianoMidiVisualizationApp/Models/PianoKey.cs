using CommunityToolkit.Mvvm.ComponentModel;

namespace PianoMidiVisualizationApp.Models;

/// <summary>Where a key sits in the selected key signature, if one is selected at all.</summary>
public enum KeyRole { None, InKey, Tonic }

public partial class PianoKey : ObservableObject
{
    public int NoteNumber { get; init; }
    public bool IsBlack { get; init; }

    /// <summary>
    /// Observable rather than init-only: the selected key decides whether this reads
    /// "A#4" or "Bb4", so the label has to be able to change after the keyboard is built.
    /// </summary>
    [ObservableProperty]
    private string _noteName = "";

    [ObservableProperty]
    private KeyRole _scaleRole;

    [ObservableProperty]
    private bool _isPressed;

    [ObservableProperty]
    private int _velocity;

    /// <summary>
    /// The app is suggesting this key: a chord to try, the next note of a song. Drawn as an
    /// overlay rather than a fill, so it shows alongside the pressed and scale-role colours.
    /// </summary>
    [ObservableProperty]
    private bool _isHinted;
}
