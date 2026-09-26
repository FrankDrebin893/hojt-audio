using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Services;
using PianoMidiVisualizationApp.Services.Playback;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>One tile of the chord strip: a diatonic chord, and whether it is what's held.</summary>
public partial class ChordTileViewModel : ObservableObject
{
    public ChordTileViewModel(DiatonicChord chord) => Chord = chord;

    public DiatonicChord Chord { get; }

    public string Numeral => Chord.Numeral.Text;

    public string Name => Chord.Name;

    public string ToolTip => $"{Chord.Name}: {string.Join(" ", Chord.NoteNames)}. Click to hear it.";

    /// <summary>The held notes are exactly this chord, in any inversion or voicing.</summary>
    [ObservableProperty]
    private bool _isPlayed;
}

/// <summary>
/// The selected key's diatonic chords as a row of tiles. Hovering a tile marks its notes on the
/// keyboard, clicking plays it, and the tile matching what's held lights up.
/// </summary>
public partial class ChordStripViewModel : ObservableObject, IDisposable
{
    private const int AuditionVelocity = 80;

    /// <summary>Gap between the notes of the roll: heard as a spread, not as an arpeggio.</summary>
    private static readonly TimeSpan RollStep = TimeSpan.FromMilliseconds(30);

    /// <summary>When every note of the audition is released, measured from the first.</summary>
    private static readonly TimeSpan AuditionLength = TimeSpan.FromSeconds(1);

    private readonly PianoKeyboardViewModel _keyboard;

    /// <summary>
    /// Plays auditions through the app-note path. Loading the next chord stops the previous one
    /// and releases its notes first, so rapid clicks never stack up or leave a note hanging.
    /// </summary>
    private readonly NoteSequencePlayer _player;

    private MusicKey? _key;
    private int? _playedRoot;
    private int _heldMask;

    /// <summary>Whether the keyboard's hints are this strip's, so leaving never clears anyone else's.</summary>
    private bool _isHinting;

    public ChordStripViewModel(PianoKeyboardViewModel keyboard, Action<int, int> noteOn, Action<int> noteOff)
    {
        _keyboard = keyboard;
        _player = new NoteSequencePlayer(noteOn, noteOff);
    }

    public ObservableCollection<ChordTileViewModel> Tiles { get; } = new();

    /// <summary>Seventh chords instead of triads.</summary>
    [ObservableProperty]
    private bool _showSevenths;

    /// <summary>A key is selected, so there are chords to show.</summary>
    [ObservableProperty]
    private bool _hasChords;

    /// <summary>
    /// Where the chords come from when it is not the selected scale, i.e. for pentatonic and
    /// blues keys ("from A Minor"). Empty otherwise.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasCaption))]
    private string _caption = "";

    /// <summary>The caption's tooltip: why a pentatonic or blues key shows another scale's chords.</summary>
    [ObservableProperty]
    private string _captionDetail = "";

    public bool HasCaption => Caption.Length > 0;

    partial void OnShowSeventhsChanged(bool value) => Rebuild();

    /// <summary>Rebuilds the tiles for a new key; null (no key selected) empties the strip.</summary>
    public void SetKey(MusicKey? key)
    {
        if (key == _key) return;
        _key = key;
        Rebuild();
    }

    /// <summary>
    /// Lights the tile the held notes match. <paramref name="root"/> is the readout's root, null
    /// when nothing is held; <paramref name="heldMask"/> is the held pitch classes.
    /// </summary>
    public void UpdatePlayed(int? root, int heldMask)
    {
        _playedRoot = root;
        _heldMask = heldMask;
        foreach (var tile in Tiles)
            tile.IsPlayed = DiatonicChords.Matches(tile.Chord, root, heldMask);
    }

    private void Rebuild()
    {
        EndHover();
        Tiles.Clear();

        if (_key is not { } key)
        {
            HasChords = false;
            Caption = CaptionDetail = "";
            return;
        }

        var set = DiatonicChords.Build(key, ShowSevenths);
        foreach (var chord in set.Chords)
            Tiles.Add(new ChordTileViewModel(chord));

        Caption = set.IsFromParentScale ? $"from {set.HarmonyKey.DisplayName}" : "";
        CaptionDetail = set.IsFromParentScale ? ParentScaleExplanation(set) : "";
        HasChords = true;
        UpdatePlayed(_playedRoot, _heldMask);
    }

    /// <summary>Says plainly that these are not the pentatonic's or blues scale's own chords.</summary>
    private static string ParentScaleExplanation(DiatonicChordSet set)
    {
        var key = set.Key;
        string parent = set.HarmonyKey.DisplayName;
        return key.Scale switch
        {
            ScaleType.Blues =>
                $"{key.DisplayName} has only six notes, too few to stack chords in thirds. These are the chords of " +
                $"{parent}, which holds every note of it except the blue note, " +
                $"{DiatonicChords.PitchName(NoteSpeller.Spell(key.Tonic + 6, key))}.",
            _ =>
                $"{key.DisplayName} has only five notes, too few to stack chords in thirds. These are the chords of " +
                $"{parent}, the seven-note scale it is drawn from.",
        };
    }

    /// <summary>Marks the tile's voicing on the keyboard.</summary>
    public void BeginHover(ChordTileViewModel tile)
    {
        _keyboard.SetHintedNotes(tile.Chord.Voicing);
        _isHinting = true;
    }

    public void EndHover()
    {
        if (!_isHinting) return;
        _isHinting = false;
        _keyboard.ClearHints();
    }

    /// <summary>
    /// Plays the tile's voicing, lightly rolled from the bottom, all of it released together
    /// about a second after the first note. A click during an audition cuts it off and plays
    /// the new chord.
    /// </summary>
    [RelayCommand]
    private void Audition(ChordTileViewModel? tile)
    {
        if (tile is null) return;

        var voicing = tile.Chord.Voicing;
        var notes = voicing.Select((note, i) =>
        {
            var start = RollStep * i;
            return new TimedNote(start, AuditionLength - start, note, AuditionVelocity);
        });

        _player.Load(notes);
        _player.Play();
    }

    public void Dispose() => _player.Dispose();
}
