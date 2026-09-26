using System.IO;
using NAudio.CoreAudioApi;
using NAudio.Wave;
using PianoMidiVisualizationApp.Audio.Sfz;

namespace PianoMidiVisualizationApp.Audio;

public class AudioEngine : IAudioEngine
{
    private const int SampleRate = 44100;

    private IWavePlayer? _outputDevice;
    private INotePlayer? _sampleProvider;
    private MasterMixSampleProvider? _mixer;
    private float _volume = 1.0f;

    public bool IsRunning { get; private set; }

    /// <summary>
    /// Created once and reused by every Initialize, so its tempo and on/off state survive a
    /// change of soundbank or output device.
    /// </summary>
    public MetronomeSampleProvider Metronome { get; } = new(SampleRate);

    /// <summary>Master volume. Applies to the synth and the metronome click alike.</summary>
    public float Volume
    {
        get => _volume;
        set
        {
            _volume = value;
            if (_mixer != null)
                _mixer.Volume = value;
        }
    }

    public IReadOnlyList<string> GetAsioDriverNames()
    {
        try
        {
            return AsioOut.GetDriverNames().ToList();
        }
        catch
        {
            return Array.Empty<string>();
        }
    }

    public IReadOnlyList<string> GetWasapiDeviceNames()
    {
        try
        {
            using var enumerator = new MMDeviceEnumerator();
            return enumerator
                .EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active)
                .Select(d => d.FriendlyName)
                .ToList();
        }
        catch
        {
            return Array.Empty<string>();
        }
    }

    public void Initialize(string driverName, bool useAsio, string soundFontPath)
    {
        Stop();
        DisposeOutput();

        _sampleProvider = Path.GetExtension(soundFontPath).Equals(".sfz", StringComparison.OrdinalIgnoreCase)
            ? new SfzSampleProvider(soundFontPath, SampleRate)
            : new SoundFontSampleProvider(soundFontPath, SampleRate);
        _mixer = new MasterMixSampleProvider(_sampleProvider, Metronome) { Volume = _volume };

        if (useAsio)
        {
            var asioOut = new AsioOut(driverName);
            asioOut.Init(_mixer);
            _outputDevice = asioOut;
        }
        else
        {
            var device = GetWasapiDevice(driverName);
            var wasapiOut = new WasapiOut(device, AudioClientShareMode.Shared, true, 50);
            wasapiOut.Init(_mixer);
            _outputDevice = wasapiOut;
        }
    }

    public void Start()
    {
        _outputDevice?.Play();
        IsRunning = true;
    }

    public void Stop()
    {
        if (_outputDevice != null)
        {
            _outputDevice.Stop();
            IsRunning = false;
        }
    }

    public void NoteOn(int channel, int note, int velocity)
    {
        _sampleProvider?.NoteOn(channel, note, velocity);
    }

    public void NoteOff(int channel, int note)
    {
        _sampleProvider?.NoteOff(channel, note);
    }

    private MMDevice? GetWasapiDevice(string friendlyName)
    {
        using var enumerator = new MMDeviceEnumerator();
        var devices = enumerator.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active);
        return devices.FirstOrDefault(d => d.FriendlyName == friendlyName)
               ?? enumerator.GetDefaultAudioEndpoint(DataFlow.Render, Role.Multimedia);
    }

    private void DisposeOutput()
    {
        _outputDevice?.Dispose();
        _outputDevice = null;
        _sampleProvider = null;
        _mixer = null;
    }

    public void Dispose()
    {
        Stop();
        DisposeOutput();
    }
}
