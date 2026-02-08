# Piano MIDI Visualization

A Windows desktop app that accepts MIDI input from a physical keyboard, plays piano sounds using SoundFont (.sf2) files, and visualizes the played notes on a virtual 88-key piano in real-time.

![Piano MIDI Visualization Demo](demo.png)

## Features

- **MIDI input** — connect any MIDI keyboard and see notes light up as you play
- **SoundFont playback** — realistic piano sounds via .sf2 files (MeltySynth synthesizer)
- **ASIO support** — ultra-low latency audio output (~5ms) with ASIO drivers
- **WASAPI fallback** — works without ASIO using Windows built-in audio
- **Real-time visualization** — 88-key piano with animated key highlighting
- **MIDI log** — scrolling console showing all raw MIDI messages for debugging
- **Settings persistence** — device selections and preferences saved between sessions

## Requirements

- Windows 10/11
- [.NET 10 Runtime](https://dotnet.microsoft.com/download/dotnet/10.0)
- A MIDI keyboard (connected via USB or MIDI interface)
- A SoundFont (.sf2) file — free options include [GeneralUser GS](https://schristiancollins.com/generaluser.php) or [FluidR3](https://github.com/musescore/MuseScore/tree/master/share/sound)
- (Optional) An ASIO driver — [ASIO4ALL](https://asio4all.org/) works as a universal option

## Building

```bash
dotnet build PianoMidiVisualizationApp.sln
```

## Running

```bash
dotnet run --project src/PianoMidiVisualizationApp
```

## Usage

1. Select your MIDI keyboard from the **MIDI** dropdown and click **Connect**
2. Click **...** next to SF2 to browse for a SoundFont file
3. Choose **ASIO** or **WASAPI** and select your audio driver
4. Click **Start Audio**
5. Play your keyboard — you'll hear sound and see keys highlight on the virtual piano

The green dot next to the MIDI controls flashes when MIDI data is received. The MIDI log panel at the bottom shows all incoming messages.

## Tech Stack

- **C# / .NET 10 / WPF** — desktop UI framework
- **NAudio** — MIDI input, ASIO and WASAPI audio output
- **MeltySynth** — pure C# SoundFont synthesizer
- **CommunityToolkit.Mvvm** — MVVM data binding
