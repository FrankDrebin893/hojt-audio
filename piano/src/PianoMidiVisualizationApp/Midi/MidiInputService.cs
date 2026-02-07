using NAudio.Midi;
using PianoMidiVisualizationApp.Models;

namespace PianoMidiVisualizationApp.Midi;

public class MidiInputService : IMidiInputService
{
    private MidiIn? _midiIn;
    private bool _disposed;

    public bool IsOpen { get; private set; }

    public event EventHandler<NoteEventArgs>? NoteOn;
    public event EventHandler<NoteEventArgs>? NoteOff;
    public event EventHandler<RawMidiMessageEventArgs>? MessageReceived;

    public IReadOnlyList<DeviceInfo> GetAvailableDevices()
    {
        var devices = new List<DeviceInfo>();
        for (int i = 0; i < MidiIn.NumberOfDevices; i++)
        {
            var info = MidiIn.DeviceInfo(i);
            devices.Add(new DeviceInfo(i, info.ProductName));
        }
        return devices;
    }

    public void Open(int deviceIndex)
    {
        Close();

        _midiIn = new MidiIn(deviceIndex);
        _midiIn.MessageReceived += OnMidiMessageReceived;
        _midiIn.ErrorReceived += OnMidiErrorReceived;
        _midiIn.Start();
        IsOpen = true;
    }

    public void Close()
    {
        if (_midiIn != null)
        {
            _midiIn.Stop();
            _midiIn.MessageReceived -= OnMidiMessageReceived;
            _midiIn.ErrorReceived -= OnMidiErrorReceived;
            _midiIn.Dispose();
            _midiIn = null;
            IsOpen = false;
        }
    }

    private void OnMidiMessageReceived(object? sender, MidiInMessageEventArgs e)
    {
        try
        {
            var evt = e.MidiEvent;
            var cmd = evt.CommandCode;

            // Always fire the raw message event so the UI can show activity
            MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
            {
                Description = $"Ch{evt.Channel} {cmd} raw=0x{e.RawMessage:X8}"
            });

            switch (cmd)
            {
                case MidiCommandCode.NoteOn:
                {
                    // NAudio may return NoteEvent or NoteOnEvent for NoteOn commands.
                    // Use the base NoteEvent type to avoid cast failures.
                    var noteEvt = (NoteEvent)evt;
                    int velocity = noteEvt is NoteOnEvent noteOnEvt ? noteOnEvt.Velocity : 0;

                    MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
                    {
                        Description = $"NoteOn Ch{noteEvt.Channel} Note={noteEvt.NoteNumber} ({noteEvt.NoteName}) Vel={velocity}"
                    });

                    if (velocity > 0)
                    {
                        NoteOn?.Invoke(this, new NoteEventArgs
                        {
                            NoteNumber = noteEvt.NoteNumber,
                            Velocity = velocity,
                            Channel = noteEvt.Channel - 1
                        });
                    }
                    else
                    {
                        NoteOff?.Invoke(this, new NoteEventArgs
                        {
                            NoteNumber = noteEvt.NoteNumber,
                            Velocity = 0,
                            Channel = noteEvt.Channel - 1
                        });
                    }
                    break;
                }

                case MidiCommandCode.NoteOff:
                {
                    var noteOff = (NoteEvent)evt;
                    MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
                    {
                        Description = $"NoteOff Ch{noteOff.Channel} Note={noteOff.NoteNumber}"
                    });

                    NoteOff?.Invoke(this, new NoteEventArgs
                    {
                        NoteNumber = noteOff.NoteNumber,
                        Velocity = 0,
                        Channel = noteOff.Channel - 1
                    });
                    break;
                }

                case MidiCommandCode.ControlChange:
                {
                    var cc = (ControlChangeEvent)evt;
                    MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
                    {
                        Description = $"CC Ch{cc.Channel} Controller={cc.Controller} Value={cc.ControllerValue}"
                    });
                    break;
                }
            }
        }
        catch (Exception ex)
        {
            MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
            {
                Description = $"Error parsing MIDI: {ex.Message} (raw=0x{e.RawMessage:X8})"
            });
        }
    }

    private void OnMidiErrorReceived(object? sender, MidiInMessageEventArgs e)
    {
        MessageReceived?.Invoke(this, new RawMidiMessageEventArgs
        {
            Description = $"MIDI Error: raw=0x{e.RawMessage:X8}"
        });
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            Close();
            _disposed = true;
        }
    }
}
