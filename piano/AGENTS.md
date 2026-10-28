# Piano MIDI Visualization

The piano viz: a Windows desktop app for practising piano with a MIDI keyboard (C# / WPF). It
lives in `piano/` of the HØJT repository, beside the HØJT DSP plugins, and has its own build,
release and licence (MIT, `piano/LICENSE`). The repository's rules in the root `AGENTS.md` hold
here too: work goes in on a branch through a pull request, since `main` is protected. Product
truth is in `PRODUCT.md`, the look in `DESIGN.md`. Its history before it moved in is in this
folder's `git log`; the old repository, FrankDrebin893/hojt-audio-piano-viz, has its releases up
to v1.0.0.

## Workflow
- Commit, push the branch and open a pull request after completing work.
- **Always publish to the desktop after completing work**, so the icon runs the new
  code. See Publish below. `dotnet build` alone never updates it.

## Build
From the repository root:
```
dotnet build piano\PianoMidiVisualizationApp.sln
```

Note: the project is `SelfContained` with `RuntimeIdentifier=win-x64`, so the build
output is `bin\Debug\net10.0-windows\win-x64\`.

## Publish
`C:\Users\Rasmu\Desktop\PianoMidiVisualizationApp.exe` is the app the user actually
runs. It is a real file copy of the single-file publish output, not a `.lnk` shortcut,
so it must be overwritten explicitly or the user silently keeps running old code.

From the repository root:
```
dotnet publish piano\src\PianoMidiVisualizationApp\PianoMidiVisualizationApp.csproj -c Release -r win-x64
```
then copy over the desktop file:
```
Copy-Item "piano\src\PianoMidiVisualizationApp\bin\Release\net10.0-windows\win-x64\publish\PianoMidiVisualizationApp.exe" "C:\Users\Rasmu\Desktop\PianoMidiVisualizationApp.exe" -Force
```
The copy fails while the app is running — close it first (`Get-Process
PianoMidiVisualizationApp`), and verify afterwards that the target's timestamp moved.

After changing `app.ico`, delete `obj\Release\net10.0-windows\win-x64\apphost.exe` (and the
Debug one) before publishing: the SDK reuses the cached apphost and the exe keeps the old
icon even though the dll has the new one. Then run `ie4uinit.exe -ClearIconCache` so
Explorer stops showing the cached one.

## Releasing
`.github/workflows/piano.yml` builds the release zip (exe + `LICENSE` +
`THIRD-PARTY-NOTICES.txt`) on every pull request and push to main that touches `piano/`, as a
workflow artifact. Pushing a tag `piano-vX.Y.Z` also publishes it as a public GitHub Release,
never marked latest (the plugins' `vX.Y.Z` releases are, and the page links their zips through
`releases/latest`). Only tag when the user asks, and do it with `/piano-release [patch|minor|major]`
(`.agents/skills/piano-release/`). Artifacts need a GitHub sign-in, so only a release can be
shared. `release-notes.md` is put at the top of the generated notes.

Never commit SoundFonts or samples (`.sf2`/`.sfz`/`.wav`, ...): they're third-party
downloads with their own licenses, and the workflow fails if any are tracked under `piano/`.
When adding or removing a NuGet package, update `THIRD-PARTY-NOTICES.txt`.

## Page
The piano's page is `site/piano/` in the repository (https://hojte.net/hojt-audio/piano/),
published with the rest of `site/` by the Page workflow. It wears the site's shared look
(`site/hojt.css`: the plugins' bench, silk and fonts) with the app's scarlet as its paint; its own
parts are in `site/piano/styles.css`. Its screenshots in `site/piano/screenshots/` are the
README's too.

## README
Use `/piano-readme` (`.agents/skills/piano-readme/`) to refresh the README. Its screenshots are
rendered off-screen from the real UI by the harness in `tools/readme-screenshots/` — never take
desktop screenshots of the app.

## Tech Stack
- C# / .NET 10 / WPF
- NAudio 2.2.1 (MIDI input, ASIO/WASAPI audio output)
- MeltySynth 2.4.1 (SoundFont synthesis)
- Melanchall.DryWetMidi 8.0.3 (MIDI files, chord naming)
- CommunityToolkit.Mvvm 8.4.0 (MVVM)
