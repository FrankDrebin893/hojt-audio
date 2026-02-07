using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.ViewModels;

public class PianoKeyboardViewModel : ObservableObject
{
    private static readonly string[] NoteNames = { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" };

    public ObservableCollection<PianoKey> Keys { get; } = new();

    private readonly Dictionary<int, PianoKey> _keyLookup = new();

    public PianoKeyboardViewModel(int lowestNote = 21, int highestNote = 108)
    {
        for (int note = lowestNote; note <= highestNote; note++)
        {
            var key = new PianoKey
            {
                NoteNumber = note,
                NoteName = GetNoteName(note),
                IsBlack = IsBlackKey(note),
            };
            Keys.Add(key);
            _keyLookup[note] = key;
        }
    }

    public void SetKeyPressed(int noteNumber, int velocity)
    {
        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = true;
            key.Velocity = velocity;
        }
    }

    public void SetKeyReleased(int noteNumber)
    {
        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = false;
            key.Velocity = 0;
        }
    }

    public static bool IsBlackKey(int noteNumber)
    {
        return (noteNumber % 12) is 1 or 3 or 6 or 8 or 10;
    }

    private static string GetNoteName(int noteNumber)
    {
        int octave = (noteNumber / 12) - 1;
        return $"{NoteNames[noteNumber % 12]}{octave}";
    }
}
