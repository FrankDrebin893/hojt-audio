using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PianoMidiVisualizationApp.Midi;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.ViewModels;

/// <summary>One action in the Piano controls list, and the control learned for it.</summary>
public partial class PianoControlRowViewModel : ObservableObject
{
    public PianoControlRowViewModel(PianoActionInfo info) => Info = info;

    public PianoActionInfo Info { get; }
    public PianoAction Action => Info.Action;
    public string Label => Info.Label;
    public string Shortcut => Info.Shortcut;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasTrigger))]
    [NotifyPropertyChangedFor(nameof(TriggerText))]
    [NotifyPropertyChangedFor(nameof(TriggerToolTip))]
    private MidiTrigger? _trigger;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(TriggerText))]
    private bool _isLearning;

    /// <summary>Briefly true when the control fires, so pressing it is visibly confirmed here.</summary>
    [ObservableProperty]
    private bool _isFiring;

    public bool HasTrigger => Trigger.HasValue;

    public string TriggerText => IsLearning ? "Press a control…"
        : Trigger is { } trigger ? trigger.DisplayName
        : "Not set";

    public string? TriggerToolTip => Trigger is { } trigger
        ? $"{trigger.DisplayName} {Info.Does}. {trigger.SideEffect}".TrimEnd()
        : null;
}

/// <summary>
/// Settings &gt; Piano controls: learns a key, pad, button or pedal on the MIDI keyboard for
/// each <see cref="PianoAction"/>, so the app can be driven with both hands on the keys.
///
/// <para>The <see cref="Router"/> does the matching on the MIDI callback thread; this view model
/// is its UI, on the UI thread. Fired actions are handed to the host through
/// <see cref="ActionFired"/>, which it runs.</para>
/// </summary>
public partial class PianoControlsViewModel : ObservableObject
{
    public const string DefaultMessage =
        "Choose Learn, then press a key, pad, button or pedal on your keyboard. A mapped key no longer plays its note.";

    /// <summary>How long a row lights up when its control fires.</summary>
    private static readonly TimeSpan FiringFlash = TimeSpan.FromMilliseconds(250);

    private readonly Dispatcher _dispatcher;
    private readonly DispatcherTimer _firingTimer;

    public MidiTriggerRouter Router { get; } = new();

    public IReadOnlyList<PianoControlRowViewModel> Rows { get; }

    /// <summary>The line under the heading: how Learn works, what it is listening for, or what it just learned.</summary>
    [ObservableProperty]
    private string _message = DefaultMessage;

    [ObservableProperty]
    private bool _isLearning;

    /// <summary>A learned control was pressed. Raised on the UI thread, with the press's own timestamp.</summary>
    public event EventHandler<PianoActionFiredEventArgs>? ActionFired;

    /// <summary>A control was learned. Raised on the UI thread.</summary>
    public event EventHandler<TriggerLearnedEventArgs>? TriggerLearned;

    /// <summary>Any mapping was learned or cleared.</summary>
    public event EventHandler? MappingsChanged;

    public PianoControlsViewModel(Dispatcher dispatcher)
    {
        _dispatcher = dispatcher;
        Rows = PianoActionInfo.All.Select(info => new PianoControlRowViewModel(info)).ToList();

        _firingTimer = new DispatcherTimer(FiringFlash, DispatcherPriority.Normal, (_, _) => EndFiringFlash(), dispatcher);
        _firingTimer.Stop();

        Router.Learned += (_, e) => _dispatcher.BeginInvoke(() => OnLearned(e));
        Router.Fired += (_, e) => _dispatcher.BeginInvoke(() => OnFired(e));
    }

    public MidiTrigger? TriggerFor(PianoAction action) => RowFor(action).Trigger;

    /// <summary>Learn, or Cancel on the row already listening.</summary>
    [RelayCommand]
    private void Learn(PianoControlRowViewModel? row)
    {
        if (row == null) return;
        if (row.IsLearning)
        {
            CancelLearning();
            return;
        }

        foreach (var other in Rows)
            other.IsLearning = false;

        row.IsLearning = true;
        IsLearning = true;
        Router.StartLearning(row.Action);
        Message = $"Listening for {row.Label}: press a key, pad, button or pedal. Esc to cancel.";
    }

    [RelayCommand]
    private void Clear(PianoControlRowViewModel? row)
    {
        if (row?.Trigger is not { } trigger) return;

        Router.Clear(row.Action);
        Refresh();
        Message = $"{row.Label} cleared. {trigger.DisplayName} plays normally again.";
        MappingsChanged?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Stops listening, e.g. on Esc or when Settings closes. Harmless when not learning.</summary>
    public void CancelLearning()
    {
        if (!IsLearning) return;

        Router.CancelLearning();
        foreach (var row in Rows)
            row.IsLearning = false;
        IsLearning = false;
        Message = DefaultMessage;
    }

    private void OnLearned(TriggerLearnedEventArgs e)
    {
        Refresh();

        // From the router, not assumed: Learn may already have been pressed on another row
        // while this press was on its way over from the MIDI thread.
        var stillLearning = Router.LearningAction;
        foreach (var row in Rows)
            row.IsLearning = row.Action == stillLearning;
        IsLearning = stillLearning != null;

        var info = PianoActionInfo.For(e.Action);
        string moved = e.PreviousAction is { } previous
            ? $" It no longer controls {PianoActionInfo.For(previous).Label}."
            : "";
        if (stillLearning == null)
            Message = $"{e.Trigger.DisplayName} now {info.Does}.{moved} {e.Trigger.SideEffect}".TrimEnd();

        TriggerLearned?.Invoke(this, e);
        MappingsChanged?.Invoke(this, EventArgs.Empty);
    }

    private void OnFired(PianoActionFiredEventArgs e)
    {
        foreach (var row in Rows)
            row.IsFiring = row.Action == e.Action;
        _firingTimer.Stop();
        _firingTimer.Start();

        ActionFired?.Invoke(this, e);
    }

    private void EndFiringFlash()
    {
        _firingTimer.Stop();
        foreach (var row in Rows)
            row.IsFiring = false;
    }

    /// <summary>Re-reads every row from the router, which also reflects a control moved between actions.</summary>
    private void Refresh()
    {
        foreach (var row in Rows)
            row.Trigger = Router.TriggerFor(row.Action);
    }

    private PianoControlRowViewModel RowFor(PianoAction action) => Rows.First(r => r.Action == action);

    public void ApplyFrom(AppSettings saved)
    {
        foreach (var mapping in saved.MidiMappings ?? [])
        {
            if (!TryParseName(mapping.Action, out PianoAction action)
                || !TryParseName(mapping.Kind, out MidiTriggerKind kind))
                continue;

            Router.Assign(action, new MidiTrigger(kind, mapping.Channel - 1, mapping.Number));
        }

        Refresh();
    }

    public void CaptureInto(AppSettings saved)
    {
        saved.MidiMappings = Router.Mappings()
            .OrderBy(m => m.Action)
            .Select(m => new MidiMappingSetting
            {
                Action = m.Action.ToString(),
                Kind = m.Trigger.Kind.ToString(),
                Channel = m.Trigger.Channel + 1,
                Number = m.Trigger.Number,
            })
            .ToList();
    }

    /// <summary>Enum.TryParse also accepts "3" or "99"; only a defined member's name counts here.</summary>
    private static bool TryParseName<TEnum>(string? name, out TEnum value) where TEnum : struct, Enum =>
        Enum.TryParse(name, out value) && Enum.IsDefined(value) && !int.TryParse(name, out _);
}
