using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.Services;

namespace PianoMidiVisualizationApp.ViewModels;

public class PianoKeyboardViewModel : ObservableObject
{
    public ObservableCollection<PianoKey> Keys { get; } = new();

    private readonly Dictionary<int, PianoKey> _keyLookup = new();

    /// <summary>
    /// Sounding notes, tracked independently of the drawn key range so that notes outside it
    /// (a Keystation's octave-shift buttons transmit well beyond its 61 physical keys) still
    /// reach chord detection instead of silently vanishing from the readout.
    /// </summary>
    private readonly HashSet<int> _pressedNotes = new();

    /// <summary>C2-C7 — the 61 keys on the user's controller.</summary>
    public PianoKeyboardViewModel(int lowestNote = 36, int highestNote = 96)
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
        _pressedNotes.Add(noteNumber);

        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = true;
            key.Velocity = velocity;
        }
    }

    public void SetKeyReleased(int noteNumber)
    {
        _pressedNotes.Remove(noteNumber);

        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = false;
            key.Velocity = 0;
        }
    }

    /// <summary>Every sounding note, including any outside the drawn range.</summary>
    public IEnumerable<int> GetPressedNotes() => _pressedNotes;

    public static bool IsBlackKey(int noteNumber)
    {
        return (noteNumber % 12) is 1 or 3 or 6 or 8 or 10;
    }

    private static string GetNoteName(int noteNumber) => MusicNaming.WithOctave(noteNumber);
}
