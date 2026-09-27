---
name: update-readme
description: Refresh README.md for the Piano MIDI Visualization app. Re-renders the README screenshots from the real WPF UI with an off-screen harness (never a desktop capture), and brings the feature list, shortcuts, requirements, usage and tech stack back in line with the code. Use when asked to update, refresh or rewrite the README, its screenshots or feature docs, or after features were added, removed or renamed.
---

# Update the README

The README is the app's shop window: a hero screenshot, what it does, how to get it running.
It goes stale because features land faster than docs. This skill rebuilds it from the code,
with fresh screenshots rendered from the real `MainWindow`.

## Ground rules

- **Never capture the screen.** No `CopyFromScreen`, no window screenshots, no computer-use
  screenshots. Windows won't bring a background process's window to the front, so a screen
  grab captures whatever the user has open. That has already leaked private content once.
  The harness below renders the WPF visual tree to a PNG, which can only ever contain the app.
- **Never load the user's settings.** `%LOCALAPPDATA%\PianoMidiVisualizationApp\settings.json`
  holds their device names, file paths and Gemini API key. The harness starts every scene from
  `new AppSettings()` with fake devices, and its demo song is generated, not a file from disk.
- **Describe the app as it is.** Every claim in the README should be checkable in the code.
  If you're unsure a feature exists (sustain pedal? velocity-shaded keys?), grep before you
  write it. If a render shows a bug, such as a blank icon or clipped text, the real app has it
  too. Report it, don't paint over it in the harness.
- **Screenshots live in `docs/screenshots/`**, one PNG per scene, named after the scene. Don't
  hand-edit them. Regenerate with the harness.

## 1. Find out what changed

Start from what the README last knew:

```bash
git log --oneline $(git log -1 --format=%H -- README.md)..HEAD
```

A commit that only touched the Download or License section resets that range without the
features having been reviewed. Check `git log -p -- README.md` for when **Features** last
really changed, and start from there.

Then read the sources of truth. The README should match these, not the other way round:

| README section | Source of truth |
|---|---|
| Panels and features | `MainWindow.xaml`: the **View** menu lists every panel with its shortcut. The `<summary>` docs on each class in `ViewModels/` and `Services/**` describe what it does. |
| Keyboard shortcuts | `MainWindow.xaml.cs` `Window_KeyDown`, plus `InputGestureText` in the View menu. |
| Keyboard range | `PianoKeyboardViewModel` constructor defaults (currently C2–C7, 61 keys). |
| Scales | `Services/ScaleType.cs` and `SettingsViewModel.ScaleOptions`. |
| Setup steps | `Views/SettingsPanel.xaml` (labels such as **SF2:**, **Start Audio**). |
| Download / install text | `.github/release-notes.md`. Keep the two consistent. |
| Tech stack | `PackageReference`s in `src/PianoMidiVisualizationApp/PianoMidiVisualizationApp.csproj`. `THIRD-PARTY-NOTICES.txt` must list the same packages. |
| AI assistant | `Services/ChatService.cs`: which provider and model it calls, and what context it sends. |

## 2. Update the scenes

Scenes live in `screenshots/Scenes.cs` in this skill's folder. Each one gets a fresh
`MainWindow` over a real `MainViewModel` with fake MIDI and audio (`Fakes.cs`), shown
off-screen, and sets up only what it shows:

- Drive the app the way a user would: `host.Press(...)` / `host.Release(...)` go through the
  fake MIDI keyboard, so the MIDI log, status bar, recorder and song scoring all see real
  input. Use view model properties and commands (`vm.IsRecorderVisible = true`,
  `vm.SaveCurrentChordCommand`) for everything else.
- **Wait on conditions, not guessed delays.** Use `Host.PumpUntil(() => ..., timeout, what)`.
  `CompositionTarget.Rendering` fires for the off-screen window, so song practice and playback
  run in real time. Never `Thread.Sleep`, which starves the dispatcher.
- Keep windows about 1280 DIPs wide. GitHub shows README images at roughly 880px, so a wider
  window makes the text unreadably small.
- Demo content must be generated or public domain. `DemoSong.cs` writes "Ode to Joy" with
  DryWetMIDI. Device and file names are made up ("Digital Piano", `C:\SoundFonts\...`).
- A new headline feature earns a scene. A small one can usually be staged inside the
  `main-window` hero shot instead.

Current scenes: `main-window` (hero), `song-practice`, `recorder`, `settings`, `zen-mode`.

## 3. Render

From the repository root:

```bash
dotnet run --project .claude/skills/update-readme/screenshots -- main-window song-practice
```

With no scene names it renders all of them. Output goes to `docs/screenshots/` (`--out DIR`
to change it), at 1.5x (`--scale N`), so the images stay sharp on HiDPI screens. It exits
non-zero if a scene throws or logs a WPF binding error, and prints each file's size.

- **Re-render only scenes whose UI changed.** Each render differs byte-wise (the MIDI log has
  real timestamps), so re-rendering everything bloats git history for nothing.
- **Look at every image you render** with the Read tool. That's safe, because it's a render of
  the app, not the screen. Check that the readout says what the scene intended, nothing is
  clipped, and no panel sits empty or shows leftover state.

Harness mechanics worth knowing if it breaks:
- `new App(); app.InitializeComponent();` loads the real `App.xaml` resources without running
  `OnStartup`, which would open real devices and load the real settings.
- The window must be `Show()`n. An unshown window has no visual tree. It sits at -32000,-32000
  with `ShowInTaskbar=false`, so it never appears on the desktop.
- The project must stay `SelfContained` with `RuntimeIdentifier=win-x64`, like the app, or the
  build fails with NETSDK1151.

## 4. Edit the README

Keep this shape unless there's a reason to change it:

1. Title and a two-sentence pitch, then the `main-window` hero image with a caption that says
   what it shows.
2. **Features**, grouped by what a player is doing (hear and see what you play, theory
   readout, progressions, song practice, recorder, metronome, layout). Put one screenshot
   next to the group it illustrates. Bold the feature name, then say what it does for the
   player, not how it's implemented.
3. **Keyboard shortcuts** as a table.
4. **Download**, **Requirements**, **Getting started** (with the `settings` screenshot),
   **Building**, **Running**, **Tech stack**, **License**.

Style: plain, concrete sentences. Name the actual UI labels in bold (**View → Settings…**,
**Start Audio**). Image paths are relative (`docs/screenshots/recorder.png`) with real alt
text. Don't promise features that are only planned.

## 5. Check before committing

```bash
grep -o 'docs/screenshots/[a-z-]*\.png' README.md | sort -u | while read f; do [ -f "$f" ] || echo "missing: $f"; done
```

- Every image the README links exists, and every PNG in `docs/screenshots/` is linked. Delete
  orphans.
- Every shortcut in the README appears in `Window_KeyDown` or the View menu.
- `git status` shows only the README, the screenshots and anything you deliberately touched.
  No `.sf2`, `.sfz` or `.wav` files. The release workflow fails if any are tracked.

Then follow `CLAUDE.md`'s workflow: commit, push, and publish to the desktop.
