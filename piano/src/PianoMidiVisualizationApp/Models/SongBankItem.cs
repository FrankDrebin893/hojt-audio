namespace PianoMidiVisualizationApp.Models;

/// <summary>
/// An entry in the Song Practice bank — either one of the built-in classic piano pieces
/// or a user-imported Standard MIDI File stored in the local song library.
/// </summary>
public sealed class SongBankItem
{
    public required string Title { get; init; }

    public required string Subtitle { get; init; }

    public required string FilePath { get; init; }

    public bool IsBuiltIn { get; init; }

    public string Badge => IsBuiltIn ? "BUILT-IN" : "USER";

    public override string ToString() => Title;
}
