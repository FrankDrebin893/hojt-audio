# Builds the VST3 bundles from THIS checkout and installs them over the old ones.
# Usage (from anywhere, in PowerShell):  scripts\install.ps1 [plugin ...]
#   plugins: pitch_shifter drums amp bass pad bells   (none given = all six)
#
# It pins the build folder to this checkout's own target\ (xtask bundles this checkout's own
# sources, even in a git worktree inside another checkout), then checks every installed file against the
# build byte for byte and says which commit it came from. Close the DAW first: a host holds
# the files it has loaded open, and Windows refuses to overwrite them.
param([string[]]$Plugins = @('pitch_shifter', 'drums', 'amp', 'bass', 'pad', 'bells'))

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:CARGO_TARGET_DIR = Join-Path $root 'target'
$dest = 'C:\Program Files\Common Files\VST3'

$bundleNames = @{
    pitch_shifter = 'HojtPitchShifter'; drums = 'HojtDrumSynth'; amp = 'HojtGuitarAmp'
    bass = 'HojtBassSynth'; pad = 'HojtShimmerPad'; bells = 'HojtGlassBells'
}

$hosts = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^(reaper|bitwig|cubase|studioone|ableton|live|fl64|reason)' }
if ($hosts) {
    Write-Host "Close these first, they keep the plugins open: $(($hosts.Name | Sort-Object -Unique) -join ', ')" -ForegroundColor Yellow
    exit 1
}

$commit = (git rev-parse --short HEAD).Trim()
$dirty = if (git status --porcelain) { ' (with uncommitted changes)' } else { '' }
Write-Host "Building $($Plugins -join ', ') from $commit$dirty into $env:CARGO_TARGET_DIR"

foreach ($plugin in $Plugins) {
    $name = $bundleNames[$plugin]
    if (-not $name) { throw "Unknown plugin '$plugin'" }
    cargo xtask bundle $plugin --release
    if ($LASTEXITCODE -ne 0) { throw "Build of $plugin failed" }

    $built = Join-Path $env:CARGO_TARGET_DIR "bundled\$name.vst3"
    if (-not (Test-Path $built)) { throw "$built was not made: the bundler wrote somewhere else" }
    Copy-Item -Recurse -Force $built $dest

    $file = "Contents\x86_64-win\$name.vst3"
    $same = (Get-FileHash (Join-Path $built $file)).Hash -eq (Get-FileHash (Join-Path $dest "$name.vst3\$file")).Hash
    if (-not $same) { throw "$name is installed but differs from the build" }
    Write-Host "$name installed ($commit)" -ForegroundColor Green
}
Write-Host 'Done. Rescan plugins in the host.'
