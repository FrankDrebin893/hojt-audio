using NAudio.CoreAudioApi;
using NAudio.Wave;
using NAudio.Wave.SampleProviders;

namespace PianoMidiVisualizationApp.Audio;

public class AudioEngine : IAudioEngine
{
    private IWavePlayer? _outputDevice;
    private SoundFontSampleProvider? _sampleProvider;
    private VolumeSampleProvider? _volumeProvider;

    public bool IsRunning { get; private set; }

    public float Volume
    {
        get => _volumeProvider?.Volume ?? 1.0f;
        set
        {
            if (_volumeProvider != null)
                _volumeProvider.Volume = value;
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

        _sampleProvider = new SoundFontSampleProvider(soundFontPath, 44100);
        _volumeProvider = new VolumeSampleProvider(_sampleProvider) { Volume = 1.0f };

        if (useAsio)
        {
            var asioOut = new AsioOut(driverName);
            asioOut.Init(_volumeProvider);
            _outputDevice = asioOut;
        }
        else
        {
            var device = GetWasapiDevice(driverName);
            var wasapiOut = new WasapiOut(device, AudioClientShareMode.Shared, true, 50);
            wasapiOut.Init(_volumeProvider);
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
        _volumeProvider = null;
    }

    public void Dispose()
    {
        Stop();
        DisposeOutput();
    }
}
