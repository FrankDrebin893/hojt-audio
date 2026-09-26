using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

/// <summary>
/// Key geometry for a contiguous note range, in the keyboard's intrinsic (pre-Viewbox) units.
/// Shared by <see cref="PianoKeyboardControl"/> and <see cref="FallingNotesControl"/> so a
/// falling note and the key it lands on are placed by the same arithmetic, not two copies of it.
/// </summary>
public sealed class KeyboardLayout
{
    // These are intrinsic dimensions, not on-screen pixels: the keyboard sits in a Viewbox that
    // scales it to the window, so what these really fix is the keyboard's aspect ratio.
    public const double WhiteKeyWidth = 26;
    public const double WhiteKeyHeight = 200;
    public const double BlackKeyWidth = 16;
    public const double BlackKeyHeight = 128;

    /// <summary>White keys are drawn one unit narrower than their slot, leaving a seam.</summary>
    public const double WhiteKeyDrawnWidth = WhiteKeyWidth - 1;

    public int LowestNote { get; }
    public int HighestNote { get; }
    public int WhiteKeyCount { get; }

    /// <summary>The drawn width of the whole keyboard.</summary>
    public double TotalWidth => WhiteKeyCount * WhiteKeyWidth;

    /// <summary>C2-C7, the keyboard this app draws.</summary>
    public static KeyboardLayout Default { get; } = new(36, 96);

    public KeyboardLayout(int lowestNote, int highestNote)
    {
        if (highestNote < lowestNote)
            throw new ArgumentOutOfRangeException(nameof(highestNote));

        LowestNote = lowestNote;
        HighestNote = highestNote;
        WhiteKeyCount = WhiteKeysBelow(highestNote + 1);
    }

    public bool Contains(int note) => note >= LowestNote && note <= HighestNote;

    public static bool IsBlack(int note) => PianoKeyboardViewModel.IsBlackKey(note);

    /// <summary>Left edge of the key's drawn rectangle.</summary>
    public double KeyLeft(int note)
    {
        int whiteKeysBefore = WhiteKeysBelow(note);
        if (!IsBlack(note))
            return whiteKeysBefore * WhiteKeyWidth;

        // A black key sits over the seam after the white key to its left, nudged by an
        // offset that varies per key to mimic a real keyboard's uneven spacing.
        int leftWhiteKeyIndex = whiteKeysBefore - 1;
        return (leftWhiteKeyIndex * WhiteKeyWidth) + (WhiteKeyWidth * BlackKeyOffset(note)) - (BlackKeyWidth / 2.0);
    }

    /// <summary>Width of the key's drawn rectangle.</summary>
    public static double KeyWidth(int note) => IsBlack(note) ? BlackKeyWidth : WhiteKeyDrawnWidth;

    public double KeyCenter(int note) => KeyLeft(note) + (KeyWidth(note) / 2.0);

    /// <summary>
    /// Moves a note into range by whole octaves, so it lands on the key with the same name.
    /// Notes already in range come back unchanged.
    /// </summary>
    public int Fold(int note)
    {
        while (note < LowestNote) note += 12;
        while (note > HighestNote) note -= 12;
        return note;
    }

    /// <summary>White keys in the range strictly below <paramref name="note"/>.</summary>
    private int WhiteKeysBelow(int note)
    {
        int count = 0;
        for (int n = LowestNote; n < note && n <= HighestNote; n++)
        {
            if (!IsBlack(n)) count++;
        }
        return count;
    }

    private static double BlackKeyOffset(int note) => (note % 12) switch
    {
        1 => 0.6,   // C#
        3 => 0.7,   // D#
        6 => 0.6,   // F#
        8 => 0.65,  // G#
        10 => 0.7,  // A#
        _ => 0.6
    };
}
