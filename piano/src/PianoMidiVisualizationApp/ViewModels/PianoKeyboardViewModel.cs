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

    /// <summary>Let go but still sounding under the sustain pedal. Never overlaps <see cref="_pressedNotes"/>.</summary>
    private readonly HashSet<int> _sustainedNotes = new();

    /// <summary>The selected key signature, or null when highlighting is off.</summary>
    private MusicKey? _activeKey;

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

    /// <summary>
    /// Points every key at a new key signature: recolours it by scale role and re-spells its
    /// label. Passing null clears the highlight and returns every name to sharps.
    /// </summary>
    public void SetKey(MusicKey? key)
    {
        _activeKey = key;
        bool useFlats = key?.UsesFlats ?? false;

        foreach (var pianoKey in Keys)
        {
            int pitchClass = MusicNaming.PitchClassOf(pianoKey.NoteNumber);

            pianoKey.NoteName = MusicNaming.WithOctave(pianoKey.NoteNumber, useFlats);
            pianoKey.ScaleRole = key switch
            {
                { } k when k.IsTonic(pitchClass) => KeyRole.Tonic,
                { } k when k.Contains(pitchClass) => KeyRole.InKey,
                _ => KeyRole.None
            };
        }
    }

    /// <summary>The key signature currently highlighted, if any.</summary>
    public MusicKey? ActiveKey => _activeKey;

    public void SetKeyPressed(int noteNumber, int velocity)
    {
        _pressedNotes.Add(noteNumber);
        _sustainedNotes.Remove(noteNumber);

        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = true;
            key.IsSustained = false;
            key.Velocity = velocity;
        }
    }

    public void SetKeyReleased(int noteNumber)
    {
        _pressedNotes.Remove(noteNumber);
        _sustainedNotes.Remove(noteNumber);

        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            key.IsPressed = false;
            key.IsSustained = false;
            key.Velocity = 0;
        }
    }

    /// <summary>The key was let go, but the sustain pedal keeps it sounding.</summary>
    public void SetKeySustained(int noteNumber)
    {
        _pressedNotes.Remove(noteNumber);
        _sustainedNotes.Add(noteNumber);

        if (_keyLookup.TryGetValue(noteNumber, out var key))
        {
            // Sustained first, so the fill goes straight from pressed to ringing.
            key.IsSustained = true;
            key.IsPressed = false;
        }
    }

    /// <summary>Every note held down, including any outside the drawn range.</summary>
    public IEnumerable<int> GetPressedNotes() => _pressedNotes;

    /// <summary>Every note sounding: held down, or let go under the sustain pedal.</summary>
    public IEnumerable<int> GetSoundingNotes() => _pressedNotes.Concat(_sustainedNotes);

    /// <summary>
    /// Marks exactly these MIDI notes as hinted and un-hints every other key, so each call
    /// replaces the previous hint rather than adding to it. Notes outside the drawn range are
    /// ignored. Call on the UI thread, like the other key setters.
    /// </summary>
    public void SetHintedNotes(IEnumerable<int> noteNumbers)
    {
        var hinted = noteNumbers.ToHashSet();
        foreach (var key in Keys)
            key.IsHinted = hinted.Contains(key.NoteNumber);
    }

    public void ClearHints()
    {
        foreach (var key in Keys)
            key.IsHinted = false;
    }

    public static bool IsBlackKey(int noteNumber)
    {
        return (noteNumber % 12) is 1 or 3 or 6 or 8 or 10;
    }

    private static string GetNoteName(int noteNumber) => MusicNaming.WithOctave(noteNumber);
}
