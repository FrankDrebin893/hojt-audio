# Scripts

What each script in this folder does, and the detail behind the rules in `AGENTS.md`.

- `install.ps1` - Builds the VST3 bundles from this checkout and installs them on Windows (see Install)
- `check.sh` - What CI runs, run before a push (see Testing); `quick` is format and clippy
- `make-art.py` - Makes the pedals' artwork in `crates/suite_common/assets/art/`
- `screenshots.ps1` - Takes the page's screenshots (`site/img/`) of the standalone windows, on Windows
- `package.sh` - Packs the bundles into the zips of a release
- `site-audio.sh` - Makes the page's sound examples from the render tests
- `sound-reports.sh` - The sound reports without their timings, for comparing two builds
- `sync-labels.sh` - The backlog's labels on GitHub
- `move-issue.sh` - Moves an issue to a state: its label and its place on the board
- `weekly-checks.sh` - The weekly run of `weekly.yml`: files `state:idea` issues for stale
  `state:building`/`state:review`, newer dependencies, the nih-plug pin and the pinned Rust, and
  instructions that name missing files, and one summary of the new ideas for `/groom`; tidies the
  state label of a closed issue. `--dry-run` prints what it would do. It needs no model or key
- `health.sh` - The project's health as one line of JSON (Health below); `health-digest.sh` turns the
  last two lines of the history into the Monday digest
- `approvals.sh` - The approval list of `approve-specs.yml`: writes the pinned "Specs to approve" and
  approves what the owner ticked or named
- `board-state.sh` - Sets an issue's State on the board without touching its label (`move-issue.sh`
  and `board-sync.yml` use it)
- `page-check.sh` - Whether `/page` has looked at what changed since `main`, and whether
  the page's links hold. The end-of-session hook runs it

## Install

On Windows run `scripts/install.ps1` (all six, or name some: `scripts/install.ps1 amp drums`). It builds from
this checkout with the build folder pinned to it, refuses a bundle that `cargo xtask check-art` does not pass,
installs, and checks every installed file against the build. Close the DAW first.

Plugins in a DAW once lost their artwork this way: nih-plug's own bundler takes the outermost folder with a
`Cargo.toml` for the workspace, which for a worktree under `.claude/worktrees` is the main checkout, so worktrees
built and installed the main checkout's old sources. `xtask/src/main.rs` now bundles the workspace it belongs to,
and `check-art` stops a build like that before it is installed or released. Install with the script, or run
`cargo xtask check-art` before copying by hand:

Copy VST3 to system folder:
```
xcopy /E /I /Y "target\bundled\HojtPitchShifter.vst3" "C:\Program Files\Common Files\VST3\HojtPitchShifter.vst3"
xcopy /E /I /Y "target\bundled\HojtDrumSynth.vst3" "C:\Program Files\Common Files\VST3\HojtDrumSynth.vst3"
xcopy /E /I /Y "target\bundled\HojtGuitarAmp.vst3" "C:\Program Files\Common Files\VST3\HojtGuitarAmp.vst3"
xcopy /E /I /Y "target\bundled\HojtBassSynth.vst3" "C:\Program Files\Common Files\VST3\HojtBassSynth.vst3"
xcopy /E /I /Y "target\bundled\HojtShimmerPad.vst3" "C:\Program Files\Common Files\VST3\HojtShimmerPad.vst3"
xcopy /E /I /Y "target\bundled\HojtGlassBells.vst3" "C:\Program Files\Common Files\VST3\HojtGlassBells.vst3"
```

## Testing

A pull request that touches the plugins gets a comment from `sound.yml`: every report of
`scripts/sound-reports.sh` before and after, or that nothing changed. A pull request whose sound
changed is `kind:sound` or says why it changed; one that must not change the sound shows
none. The renders before and after are the run's `renders` artifact.

**Always run `scripts/check.sh` before pushing.** It runs every step of `ci.yml`: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings` (on Rust 1.93.0, pinned in `ci.yml`, when rustup has it),
`cargo test --workspace` and `cargo test -p amp --release`. On Linux it first names the packages the build needs
that are missing. In Claude Code a hook (`.claude/settings.json`) runs `scripts/check.sh quick` before every
`git commit` and stops a commit that fails it; an agent tool without hooks runs `scripts/check.sh quick` itself
before committing. A step added to `ci.yml` is added to `scripts/check.sh` too, which
`xtask/tests/check_mirrors_ci.rs` holds.
Lints that clash with DSP code are allowed in the root `Cargo.toml`, each with its reason.

## Health

`health.yml` runs `health.sh --sound` every Monday (and by hand with "Run workflow"), adds the line
to `health/history.jsonl` on the `metrics` branch, and comments `health-digest.sh`'s summary on the
issue "Project health". Nothing in it changes `main`.

| Field | What it is |
|-------|------------|
| `flow.wip` | Open issues per state label |
| `flow.merged_7d`, `merged_28d` | Pull requests merged in the last 7 and 28 days |
| `flow.specs_waiting`, `oldest_spec_wait_h` | Open specs, and the hours the oldest has waited since it became a spec |
| `flow.approval_wait_h_median` | Issues done in the last 28 days: median hours from `state:spec` to `state:approved` |
| `flow.build_h_median` | The same issues: median hours from `state:approved` to closed |
| `flow.scout` | `from:scout` issues filed, approved (or built), declined (closed as not planned), and the hit rate approved ÷ decided |
| `ci.main_pass_pct`, `main_red_now` | The last 30 finished `ci.yml` runs on `main`: share green, and whether the latest is red |
| `ci.pr_pass_pct`, `pr_minutes_median` | The last 60 on pull requests: share green, median minutes a run takes |
| `releases` | The last release, days since, and the entries under `Unreleased` in `CHANGELOG.md` |
| `plugins.cpu` | Every `% of a core` line of the reports that time a block, with its number taken out of `what` so weeks compare |
| `plugins.loudness` | Every render of `render_wavs`: integrated loudness (LUFS) and true peak (dB), by ffmpeg's `ebur128` |
| `plugins.tests` | The number of tests in the workspace |

The digest lists under **Look at** what crossed a line: `main` green under 90% of the time or red
now, a spec waiting over a week, changes unreleased more than 30 days after the last release, a
timed line more than 10% dearer than the week before, a render more than 1 LU louder or quieter.

