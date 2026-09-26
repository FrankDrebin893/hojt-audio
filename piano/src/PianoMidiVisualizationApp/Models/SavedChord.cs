using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.Models;

public partial class SavedChord : ObservableObject
{
    public string ChordName { get; init; } = "";
    public List<int> NoteNumbers { get; init; } = new();
    public List<string> NoteNames { get; init; } = new();

    public string NotesDisplay => string.Join(" ", NoteNames);

    /// <summary>
    /// The chord's Roman numeral in the selected key, e.g. "V65/V". Re-analysed whenever the
    /// key changes, so a saved progression always reads in the current key. Empty with no key.
    /// </summary>
    [ObservableProperty]
    private string _function = "";

    /// <summary>How <see cref="Function"/> relates to the key. Meaningless while it is empty.</summary>
    [ObservableProperty]
    private RomanNumeralKind _functionKind;

    /// <summary>True while progression playback is sounding this chord, for its highlight.</summary>
    [ObservableProperty]
    private bool _isSounding;
}
