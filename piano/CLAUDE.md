# CLAUDE.md

## Workflow
- Always commit and push changes after completing work.
- Remote: https://github.com/FrankDrebin893/PianoVisualization.git
- **Always publish to the desktop after completing work**, so the icon runs the new
  code. See Publish below. `dotnet build` alone never updates it.

## Build
```
dotnet build D:\Repos\PianoMidiVisualizationApp\PianoMidiVisualizationApp.sln
```

Note: the project is `SelfContained` with `RuntimeIdentifier=win-x64`, so the build
output is `bin\Debug\net10.0-windows\win-x64\`. A stale exe from before the RID was
added still sits in `bin\Debug\net10.0-windows\` — don't run that one by mistake.

## Publish
`C:\Users\Rasmu\Desktop\PianoMidiVisualizationApp.exe` is the app the user actually
runs. It is a real file copy of the single-file publish output, not a `.lnk` shortcut,
so it must be overwritten explicitly or the user silently keeps running old code.

```
dotnet publish D:\Repos\PianoMidiVisualizationApp\src\PianoMidiVisualizationApp\PianoMidiVisualizationApp.csproj -c Release -r win-x64
```
then copy over the desktop file:
```
Copy-Item "D:\Repos\PianoMidiVisualizationApp\src\PianoMidiVisualizationApp\bin\Release\net10.0-windows\win-x64\publish\PianoMidiVisualizationApp.exe" "C:\Users\Rasmu\Desktop\PianoMidiVisualizationApp.exe" -Force
```
The copy fails while the app is running — close it first (`Get-Process
PianoMidiVisualizationApp`), and verify afterwards that the target's timestamp moved.

## Tech Stack
- C# / .NET 10 / WPF
- NAudio 2.2.1 (MIDI input, ASIO/WASAPI audio output)
- MeltySynth 2.4.1 (SoundFont synthesis)
- CommunityToolkit.Mvvm 8.4.0 (MVVM)
