# Tests the installer on a Windows runner (release.yml, pull-request path and tags). Works in
# Windows PowerShell 5.1 and in pwsh. Nothing here touches the machine's plugin folders: every
# install goes to a temp folder (/DIR for the licences, /PLUGINROOT for VST3 and CLAP).
#
#   installer\test-installer.ps1 -Installer dist\HojtDSP-Setup.exe -BundleDir target\bundled `
#       -Version 0.3.0 [-PreviousZipDir prev]
#
# 1. Clean install of everything: every file of every bundle is there and identical, so is each
#    standalone program (with its Start menu entry), the version is registered, the licences are in
#    the app folder.
# 2. Uninstall: the app folder, the registry entry and every installed file are gone.
# 3. Components: only what is ticked is installed (the standalone programs are a component of each plugin).
# 4. Upgrade: the contents of the previous release's zips (or, with none, an older build made up
#    from this one) are in place as if copied by hand, the installer goes over them, and every
#    file is the new build's. Then uninstall again.
param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$BundleDir,
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$StandaloneDir = '',
    [string]$PreviousZipDir = ''
)

$ErrorActionPreference = 'Stop'
$AppId = '{4DFF5B9E-7443-46F9-9E6B-18D7A4889F6E}'
$UninstallKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\${AppId}_is1"

$Installer = (Resolve-Path $Installer).Path
$BundleDir = (Resolve-Path $BundleDir).Path
if (-not $StandaloneDir) { $StandaloneDir = Join-Path (Split-Path -Parent $BundleDir) 'standalone' }
$StandaloneDir = (Resolve-Path $StandaloneDir).Path
$StartMenu = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs'
$bundlerToml = Join-Path (Split-Path -Parent $PSScriptRoot) 'bundler.toml'
$names = @(Get-Content $bundlerToml | ForEach-Object {
        if ($_ -match '^name = "(.*)"$') { $Matches[1] }
    })
if ($names.Count -eq 0) { throw 'no bundle names in bundler.toml' }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("hojt-installer-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$failures = New-Object System.Collections.ArrayList

function Fail([string]$message) {
    Write-Host "FAIL: $message" -ForegroundColor Red
    [void]$failures.Add($message)
}

function Hash([string]$path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash }

# Runs the installer silently with extra arguments; returns the exit code. The log is shown on failure
function Install([string]$app, [string]$pluginRoot, [string]$tag, [string[]]$extra) {
    $log = Join-Path $work "$tag.log"
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$app`"", "/PLUGINROOT=`"$pluginRoot`"", "/LOG=`"$log`"") + $extra
    $p = Start-Process -FilePath $Installer -ArgumentList $arguments -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        Fail "the installer ($tag) exited with $($p.ExitCode)"
        if (Test-Path $log) { Get-Content $log -Tail 40 | ForEach-Object { Write-Host "  $_" } }
    }
    return $p.ExitCode
}

# The uninstaller starts a copy of itself and returns at once: wait until the work is done
function Uninstall([string]$app, [string]$tag) {
    $unins = Join-Path $app 'unins000.exe'
    if (-not (Test-Path $unins)) { Fail "${tag}: no uninstaller at $unins"; return }
    $log = Join-Path $work "$tag-uninstall.log"
    $p = Start-Process -FilePath $unins -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$log`"") -Wait -PassThru
    if ($p.ExitCode -ne 0) { Fail "the uninstaller ($tag) exited with $($p.ExitCode)" }
    $deadline = (Get-Date).AddSeconds(90)
    while ((Test-Path $app) -or (Test-Path $UninstallKey)) {
        if ((Get-Date) -gt $deadline) { break }
        Start-Sleep -Milliseconds 500
    }
    if (Test-Path $app) { Fail "${tag}: $app is still there after the uninstall" }
    if (Test-Path $UninstallKey) { Fail "${tag}: the uninstall entry is still in the registry" }
}

# Every file of the bundles named (VST3 and CLAP) is in the plugin root and is the build's
function Assert-Bundles([string]$pluginRoot, [string[]]$only, [string]$tag) {
    foreach ($name in $only) {
        $vst3 = Join-Path $BundleDir "$name.vst3"
        $count = 0
        foreach ($file in Get-ChildItem -LiteralPath $vst3 -Recurse -File) {
            $relative = $file.FullName.Substring($vst3.Length).TrimStart('\')
            $target = Join-Path (Join-Path $pluginRoot "VST3\$name.vst3") $relative
            if (-not (Test-Path -LiteralPath $target)) { Fail "${tag}: $name.vst3\$relative was not installed" }
            elseif ((Hash $file.FullName) -ne (Hash $target)) { Fail "${tag}: $name.vst3\$relative differs from the build" }
            $count++
        }
        if ($count -eq 0) { Fail "${tag}: the build has no files for $name.vst3" }
        $clap = Join-Path $pluginRoot "CLAP\$name.clap"
        if (-not (Test-Path -LiteralPath $clap)) { Fail "${tag}: $name.clap was not installed" }
        elseif ((Hash (Join-Path $BundleDir "$name.clap")) -ne (Hash $clap)) { Fail "${tag}: $name.clap differs from the build" }
    }
}

# Each standalone program is in the app folder, is the build's, and has a Start menu entry. The
# entries are found by the plugin's name, since the folder and the shortcuts carry the Ø
$DisplayName = @{ HojtPitchShifter = 'Pitch Shifter'; HojtDrumSynth = 'Drum Synth'; HojtGuitarAmp = 'Guitar Amp'; HojtBassSynth = 'Bass Synth'; HojtShimmerPad = 'Shimmer Pad'; HojtGlassBells = 'Glass Bells' }
function Start-MenuShortcuts([string]$name) {
    @(Get-ChildItem -LiteralPath $StartMenu -Recurse -File -Filter "*$($DisplayName[$name]).lnk" -ErrorAction SilentlyContinue)
}
function Assert-Standalones([string]$app, [string[]]$only, [string]$tag) {
    foreach ($name in $only) {
        $installed = Join-Path $app "$name.exe"
        if (-not (Test-Path -LiteralPath $installed)) { Fail "${tag}: $name.exe was not installed"; continue }
        if ((Hash (Join-Path $StandaloneDir "$name.exe")) -ne (Hash $installed)) { Fail "${tag}: $name.exe differs from the build" }
        $shortcuts = Start-MenuShortcuts $name
        if ($shortcuts.Count -eq 0) { Fail "${tag}: $name has no Start menu entry" }
        # In the suite's own folder (HØJT DSP), not one Inno names "(Default)" when the script names none
        elseif (-not ($shortcuts | Where-Object { $_.Directory.Name -like 'H?JT DSP' })) { Fail "${tag}: $name's Start menu entry is in $($shortcuts[0].Directory.Name), not the suite's own folder" }
    }
}
function Assert-No-Standalones([string]$app, [string[]]$only, [string]$tag) {
    foreach ($name in $only) {
        if (Test-Path -LiteralPath (Join-Path $app "$name.exe")) { Fail "${tag}: $name.exe is still installed" }
        if ((Start-MenuShortcuts $name).Count -gt 0) { Fail "${tag}: $name still has a Start menu entry" }
    }
}

# No file is left under the plugin root, or, in a lenient run, none that is in the new build
function Assert-Clean([string]$pluginRoot, [string]$tag) {
    if (-not (Test-Path $pluginRoot)) { return }
    $left = @(Get-ChildItem -LiteralPath $pluginRoot -Recurse -File)
    if ($left.Count -gt 0) {
        Fail "${tag}: the uninstall left $($left.Count) file(s): $(($left | Select-Object -First 5 | ForEach-Object { $_.FullName }) -join ', ')"
    }
}

function Assert-Registered([string]$app, [string]$tag) {
    $entry = Get-ItemProperty -Path $UninstallKey -ErrorAction SilentlyContinue
    if (-not $entry) { Fail "${tag}: the installer registered nothing in the uninstall list"; return }
    if ($entry.DisplayVersion -ne $Version) { Fail "${tag}: registered version is '$($entry.DisplayVersion)', Cargo.toml says '$Version'" }
    foreach ($f in @('unins000.exe', 'LICENSE', 'NOTICE.md')) {
        if (-not (Test-Path (Join-Path $app $f))) { Fail "${tag}: $f is not in the app folder" }
    }
}

try {
    $fileVersion = (Get-Item $Installer).VersionInfo.FileVersion
    if (-not $fileVersion -or -not $fileVersion.StartsWith($Version.Split('-')[0])) {
        Fail "the installer's file version is '$fileVersion', Cargo.toml says '$Version'"
    }
    if (Test-Path $UninstallKey) { throw 'A HOJT DSP install is already registered on this machine: the test would replace it. Run it on a runner.' }

    # 1 and 2. Clean install, then uninstall
    Write-Host '== Clean install'
    $app = Join-Path $work 'app1'
    $root = Join-Path $work 'plugins1'
    if ((Install $app $root 'clean' @()) -eq 0) {
        Assert-Registered $app 'clean'
        Assert-Bundles $root $names 'clean'
        Assert-Standalones $app $names 'clean'
        Uninstall $app 'clean'
        Assert-Clean $root 'clean'
        Assert-No-Standalones $app $names 'clean'
    }

    # 3. Components: one plugin, VST3 only
    Write-Host '== Components'
    $app = Join-Path $work 'app2'
    $root = Join-Path $work 'plugins2'
    $one = $names[0]
    $componentId = @{ HojtPitchShifter = 'pitch_shifter'; HojtDrumSynth = 'drums'; HojtGuitarAmp = 'amp'; HojtBassSynth = 'bass'; HojtShimmerPad = 'pad'; HojtGlassBells = 'bells' }[$one]
    if (-not $componentId) { throw "no component id known for $one" }
    if ((Install $app $root 'one' @("/COMPONENTS=`"$componentId,$componentId\vst3`"")) -eq 0) {
        if (-not (Test-Path (Join-Path $root "VST3\$one.vst3"))) { Fail "components: $one.vst3 was not installed" }
        if (Test-Path (Join-Path $root "CLAP\$one.clap")) { Fail "components: $one.clap was installed though it was not ticked" }
        $others = @(Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object { $_.FullName -notlike "*$one*" })
        if ($others.Count -gt 0) { Fail "components: files of other plugins were installed: $(($others | Select-Object -First 3 | ForEach-Object { $_.Name }) -join ', ')" }
        # Only the VST3 was ticked: no standalone program, and none of the others
        Assert-No-Standalones $app $names 'components'
        Uninstall $app 'one'
        Assert-Clean $root 'one'
    }

    # 4. Upgrade over an older build copied in by hand
    Write-Host '== Upgrade'
    $app = Join-Path $work 'app3'
    $root = Join-Path $work 'plugins3'
    New-Item -ItemType Directory -Path (Join-Path $root 'VST3'), (Join-Path $root 'CLAP') | Out-Null
    $source = 'a made-up older build'
    $zips = @()
    if ($PreviousZipDir -and (Test-Path $PreviousZipDir)) { $zips = @(Get-ChildItem -LiteralPath $PreviousZipDir -Filter '*-windows.zip') }
    if ($zips.Count -gt 0) {
        $source = "the previous release ($($zips.Count) zips)"
        foreach ($zip in $zips) {
            $out = Join-Path $work ("prev-" + $zip.BaseName)
            Expand-Archive -LiteralPath $zip.FullName -DestinationPath $out -Force
            foreach ($item in Get-ChildItem -LiteralPath $out -Recurse -Directory -Filter '*.vst3') { Copy-Item -Recurse -Force $item.FullName (Join-Path $root 'VST3') }
            foreach ($item in Get-ChildItem -LiteralPath $out -Recurse -File -Filter '*.clap') { Copy-Item -Force $item.FullName (Join-Path $root 'CLAP') }
        }
    }
    else {
        foreach ($name in $names) {
            Copy-Item -Recurse -Force (Join-Path $BundleDir "$name.vst3") (Join-Path $root 'VST3')
            Copy-Item -Force (Join-Path $BundleDir "$name.clap") (Join-Path $root 'CLAP')
        }
    }
    # Whatever the source, one file of each is made to differ from the new build, as an older one would
    foreach ($name in $names) {
        $dll = Join-Path $root "VST3\$name.vst3\Contents\x86_64-win\$name.vst3"
        if (Test-Path $dll) { [System.IO.File]::AppendAllText($dll, 'older build') }
        $clap = Join-Path $root "CLAP\$name.clap"
        if (Test-Path $clap) { [System.IO.File]::AppendAllText($clap, 'older build') }
    }
    Write-Host "Older build: $source"
    if ((Install $app $root 'upgrade' @()) -eq 0) {
        Assert-Registered $app 'upgrade'
        Assert-Bundles $root $names 'upgrade'
        Assert-Standalones $app $names 'upgrade'
        Uninstall $app 'upgrade'
        Assert-No-Standalones $app $names 'upgrade'
        # What an older release had and this one has not is not the installer's to remove: only the new files count
        foreach ($name in $names) {
            if (Test-Path (Join-Path $root "CLAP\$name.clap")) { Fail "upgrade: $name.clap is still there after the uninstall" }
            $dll = Join-Path $root "VST3\$name.vst3\Contents\x86_64-win\$name.vst3"
            if (Test-Path $dll) { Fail "upgrade: $name.vst3 is still there after the uninstall" }
        }
    }
}
finally {
    # Whatever a failed step left installed must not stay on the runner
    if (Test-Path $UninstallKey) {
        foreach ($app in @('app1', 'app2', 'app3')) {
            $unins = Join-Path (Join-Path $work $app) 'unins000.exe'
            if (Test-Path $unins) { Start-Process -FilePath $unins -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait | Out-Null; Start-Sleep -Seconds 5 }
        }
    }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failures.Count -gt 0) {
    Write-Host "$($failures.Count) check(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host 'The installer installs, upgrades and uninstalls cleanly.' -ForegroundColor Green
exit 0
