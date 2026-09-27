namespace PianoMidiVisualizationApp.Midi;

/// <summary>A learned control was pressed. Raised on the MIDI callback thread.</summary>
public sealed class PianoActionFiredEventArgs(PianoAction action, MidiTrigger trigger, long timestamp) : EventArgs
{
    public PianoAction Action { get; } = action;
    public MidiTrigger Trigger { get; } = trigger;

    /// <summary>The <see cref="System.Diagnostics.Stopwatch"/> timestamp of the press, taken on arrival.</summary>
    public long Timestamp { get; } = timestamp;
}

/// <summary>A control was learned for an action. Raised on the MIDI callback thread.</summary>
public sealed class TriggerLearnedEventArgs(PianoAction action, MidiTrigger trigger, PianoAction? previousAction) : EventArgs
{
    public PianoAction Action { get; } = action;
    public MidiTrigger Trigger { get; } = trigger;

    /// <summary>The action this control was mapped to before, which has lost it.</summary>
    public PianoAction? PreviousAction { get; } = previousAction;
}

/// <summary>
/// Decides, on the MIDI callback thread and before anything sounds, whether a message belongs
/// to a learned control. Each <c>On…</c> method returns true when it consumed the message; the
/// caller must then drop it, so a learned control never reaches the audio engine, the
/// keyboard, the chord readout, the recorder or song scoring.
///
/// <para>One control maps to one action and one action to one control. A control fires once per
/// press: a note on its note-on, a controller when its value rises through
/// <see cref="MidiTrigger.PressedThreshold"/>, a program change on every message. Every other
/// message from a mapped control is swallowed too: a key's note-off, a button's release value,
/// the pedal's every movement.</para>
///
/// <para>Thread-safe. Messages arrive on the callback thread while the UI learns and clears.</para>
/// </summary>
public sealed class MidiTriggerRouter
{
    private readonly object _sync = new();
    private readonly Dictionary<MidiTrigger, PianoAction> _actions = new();
    private readonly Dictionary<PianoAction, MidiTrigger> _triggers = new();

    /// <summary>The action waiting for its control, if Learn is on.</summary>
    private PianoAction? _learning;

    /// <summary>Notes whose note-on was consumed, indexed channel * 128 + note, so their note-off is consumed too.</summary>
    private readonly bool[] _swallowedNotes = new bool[16 * 128];

    /// <summary>Per controller, whether its last value read as pressed, so a press fires once however many values it sends.</summary>
    private readonly bool[] _controllerPressed = new bool[16 * 128];

    public event EventHandler<PianoActionFiredEventArgs>? Fired;
    public event EventHandler<TriggerLearnedEventArgs>? Learned;

    public PianoAction? LearningAction
    {
        get { lock (_sync) return _learning; }
    }

    public MidiTrigger? TriggerFor(PianoAction action)
    {
        lock (_sync)
            return _triggers.TryGetValue(action, out var trigger) ? trigger : null;
    }

    /// <summary>Waits for the next press to become this action's control. Replaces any other action waiting.</summary>
    public void StartLearning(PianoAction action)
    {
        lock (_sync) _learning = action;
    }

    public void CancelLearning()
    {
        lock (_sync) _learning = null;
    }

    /// <summary>Maps a control directly, as loading saved settings does. Returns the action it was taken from, if any.</summary>
    public PianoAction? Assign(PianoAction action, MidiTrigger trigger)
    {
        if (!trigger.IsValid) return null;
        lock (_sync) return AssignLocked(action, trigger);
    }

    public void Clear(PianoAction action)
    {
        lock (_sync)
        {
            if (_triggers.Remove(action, out var trigger))
                _actions.Remove(trigger);
        }
    }

    public IReadOnlyList<(PianoAction Action, MidiTrigger Trigger)> Mappings()
    {
        lock (_sync)
            return _triggers.Select(pair => (pair.Key, pair.Value)).ToList();
    }

    public bool OnNoteOn(int channel, int note, long timestamp)
    {
        var trigger = new MidiTrigger(MidiTriggerKind.Note, channel, note);
        if (!trigger.IsValid) return false;

        EventArgs? raised;
        lock (_sync)
        {
            raised = LearnOrFireLocked(trigger, timestamp);
            if (raised == null) return false;
            _swallowedNotes[channel * 128 + note] = true;
        }

        Raise(raised);
        return true;
    }

    /// <summary>
    /// Consumes the note-off of a note-on this router consumed, whether or not the note is
    /// still mapped. A note that sounded before it was mapped is let through, so it can stop.
    /// </summary>
    public bool OnNoteOff(int channel, int note)
    {
        if (!new MidiTrigger(MidiTriggerKind.Note, channel, note).IsValid) return false;

        lock (_sync)
        {
            int slot = channel * 128 + note;
            if (!_swallowedNotes[slot]) return false;
            _swallowedNotes[slot] = false;
            return true;
        }
    }

    public bool OnControlChange(int channel, int controller, int value, long timestamp)
    {
        var trigger = new MidiTrigger(MidiTriggerKind.ControlChange, channel, controller);
        if (!trigger.IsValid) return false;

        EventArgs? raised = null;
        lock (_sync)
        {
            int slot = channel * 128 + controller;
            bool pressed = value >= MidiTrigger.PressedThreshold;
            bool wasPressed = _controllerPressed[slot];
            _controllerPressed[slot] = pressed;

            // Only a press is learned or fires. A mapped controller's other values are still
            // the app's: a pedal mapped to an action must not sustain on the way back up.
            if (pressed && !wasPressed)
                raised = LearnOrFireLocked(trigger, timestamp);
            if (raised == null && !_actions.ContainsKey(trigger))
                return false;
        }

        if (raised != null) Raise(raised);
        return true;
    }

    public bool OnProgramChange(int channel, int program, long timestamp)
    {
        var trigger = new MidiTrigger(MidiTriggerKind.ProgramChange, channel, program);
        if (!trigger.IsValid) return false;

        EventArgs? raised;
        lock (_sync) raised = LearnOrFireLocked(trigger, timestamp);

        if (raised == null) return false;
        Raise(raised);
        return true;
    }

    /// <summary>Learning takes the press if Learn is on; otherwise a mapped control fires. Null if neither.</summary>
    private EventArgs? LearnOrFireLocked(MidiTrigger trigger, long timestamp)
    {
        if (_learning is { } learning)
        {
            _learning = null;
            var previous = AssignLocked(learning, trigger);
            return new TriggerLearnedEventArgs(learning, trigger, previous);
        }

        return _actions.TryGetValue(trigger, out var action)
            ? new PianoActionFiredEventArgs(action, trigger, timestamp)
            : null;
    }

    private PianoAction? AssignLocked(PianoAction action, MidiTrigger trigger)
    {
        PianoAction? previous = null;
        if (_actions.Remove(trigger, out var owner))
        {
            _triggers.Remove(owner);
            if (owner != action) previous = owner;
        }

        if (_triggers.Remove(action, out var old))
            _actions.Remove(old);

        _actions[trigger] = action;
        _triggers[action] = trigger;
        return previous;
    }

    // Outside the lock, so a handler may call back in.
    private void Raise(EventArgs e)
    {
        if (e is PianoActionFiredEventArgs fired) Fired?.Invoke(this, fired);
        else if (e is TriggerLearnedEventArgs learned) Learned?.Invoke(this, learned);
    }
}
