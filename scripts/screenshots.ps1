# Takes the page's screenshots (site/img) of the plugins as they are in this checkout: builds
# each plugin's standalone window, opens it, waits for it to draw, and saves its inside, the
# pedal and nothing else, at the window's own size.
# Usage (from anywhere, in PowerShell):  scripts\screenshots.ps1 [plugin ...]
#   plugins: pitch_shifter drums amp bass pad bells   (none given = all six)
#
# Run it on a display at 100 % scaling, or the windows are larger than their own size and it
# stops. The window is brought to the front and captured from the screen, so leave the
# desktop alone while it runs.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Plugins = @('pitch_shifter', 'drums', 'amp', 'bass', 'pad', 'bells'))
# Names may be given apart (amp drums) or as one list (amp,drums)
$Plugins = @("$Plugins" -split '[,\s]+' | Where-Object { $_ })

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:CARGO_TARGET_DIR = Join-Path $root 'target'

# The file in site/img and the window's size, from WINDOW_WIDTH and WINDOW_HEIGHT in each editor
$shots = @{
    pitch_shifter = @('pitch-shifter.png', 400, 560); drums = @('drum-synth.png', 660, 600)
    amp = @('guitar-amp.png', 1830, 650); bass = @('bass-synth.png', 660, 600)
    pad = @('shimmer-pad.png', 660, 600); bells = @('glass-bells.png', 660, 600)
}

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Shot {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
}
'@
[Shot]::SetProcessDPIAware() | Out-Null

# Cargo writes its progress to stderr. Windows PowerShell turns that into errors, which stop
# the script when its output is redirected, so cargo runs under cmd with stderr merged
foreach ($plugin in $Plugins) {
    $shot = $shots[$plugin]
    if (-not $shot) { throw "Unknown plugin '$plugin'" }
    $file, $width, $height = $shot

    cmd /c "cargo build -p $plugin --features standalone --release 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "Build of $plugin failed" }
    $exe = Join-Path $env:CARGO_TARGET_DIR "release\$($plugin)_standalone.exe"

    $process = Start-Process $exe -ArgumentList '--backend', 'dummy' -PassThru
    try {
        $deadline = (Get-Date).AddSeconds(20)
        while ($process.MainWindowHandle -eq 0) {
            if ((Get-Date) -gt $deadline) { throw "$plugin opened no window" }
            Start-Sleep -Milliseconds 200
            $process.Refresh()
        }
        $window = $process.MainWindowHandle
        [Shot]::ShowWindow($window, 9) | Out-Null
        [Shot]::SetForegroundWindow($window) | Out-Null
        # Long enough for the lit surfaces and the art to be rendered and drawn
        Start-Sleep -Seconds 3

        $rect = New-Object Shot+RECT
        [Shot]::GetClientRect($window, [ref]$rect) | Out-Null
        if ($rect.Right -ne $width -or $rect.Bottom -ne $height) {
            throw "$plugin's window is $($rect.Right) x $($rect.Bottom), not $width x ${height}: is the display at 100 % scaling?"
        }
        $corner = New-Object Shot+POINT
        [Shot]::ClientToScreen($window, [ref]$corner) | Out-Null

        $bitmap = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($corner.X, $corner.Y, 0, 0, $bitmap.Size)
        $out = Join-Path $root "site\img\$file"
        $bitmap.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose()
        $bitmap.Dispose()
        Write-Host "$file ($width x $height)" -ForegroundColor Green
    } finally {
        Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
    }
}
