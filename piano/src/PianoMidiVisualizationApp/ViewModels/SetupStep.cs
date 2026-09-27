using CommunityToolkit.Mvvm.ComponentModel;

namespace PianoMidiVisualizationApp.ViewModels;

public enum SetupStepState
{
    /// <summary>Not done, and not the first thing to do either. Still clickable: no step waits on another.</summary>
    Pending,

    /// <summary>The first step that isn't done: the one the checklist points you at.</summary>
    Next,

    Done,
}

/// <summary>
/// One line of the first-run setup checklist. <see cref="MainViewModel"/> rewrites all three
/// from live state whenever the connection or the settings behind them change, and clicking
/// one runs <see cref="MainViewModel.RunSetupStepCommand"/> with it.
/// </summary>
public partial class SetupStep : ObservableObject
{
    public SetupStep(int number) => Number = number;

    public int Number { get; }

    [ObservableProperty]
    private string _title = "";

    /// <summary>What the step acts on ("Digital Piano"), what it did, or what went wrong.</summary>
    [ObservableProperty]
    private string _detail = "";

    [ObservableProperty]
    private string _toolTip = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsDone))]
    private SetupStepState _state;

    /// <summary>The last attempt failed, or what the step needs has gone missing. <see cref="Detail"/> says which.</summary>
    [ObservableProperty]
    private bool _hasProblem;

    public bool IsDone => State == SetupStepState.Done;

    /// <summary>Pending versus Next is settled afterwards, across all three steps.</summary>
    internal void Update(bool done, string title, string detail, string toolTip, bool hasProblem = false)
    {
        Title = title;
        Detail = detail;
        ToolTip = toolTip;
        HasProblem = hasProblem;
        if (done) State = SetupStepState.Done;
        else if (State == SetupStepState.Done) State = SetupStepState.Pending;
    }
}
