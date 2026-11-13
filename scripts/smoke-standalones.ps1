# Starts each standalone program of target/standalone with the dummy audio backend (no sound
# card needed), waits a few seconds, and checks that it is still running, so that its window
# opened rather than the program dying at start, and that its exe has an icon resource (the HØJT badge). Then it asks the window to close and expects the program to exit. Windows only
# (release.yml, the Windows job):
#
#   scripts\smoke-standalones.ps1 [-Dir target\standalone] [-Seconds 3]
#
# An exe that exits at once, or has no icon, fails the run. One that ignores the close request is
# stopped and only reported, since a runner's desktop session may not deliver it.
param(
    [string]$Dir = 'target/standalone',
    [int]$Seconds = 3
)

$ErrorActionPreference = 'Stop'
# ExtractIconEx with index -1 counts the icon resources of a file: 0 for an exe without one
Add-Type -Namespace Hojt -Name Icons -MemberDefinition '[System.Runtime.InteropServices.DllImport("shell32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)] public static extern uint ExtractIconEx(string file, int index, System.IntPtr[] large, System.IntPtr[] small, uint count);'
$failures = 0
$exes = @(Get-ChildItem -LiteralPath $Dir -Filter '*.exe')
if ($exes.Count -lt 6) { throw "expected six standalone programs in $Dir, found $($exes.Count)" }

foreach ($exe in $exes) {
    Write-Host "== $($exe.Name)"
    if ([Hojt.Icons]::ExtractIconEx($exe.FullName, -1, $null, $null, 0) -eq 0) {
        Write-Host "FAIL: $($exe.Name) has no icon resource" -ForegroundColor Red
        $failures++
    }

    $p = Start-Process -FilePath $exe.FullName -ArgumentList @('--backend', 'dummy') -PassThru
    Start-Sleep -Seconds $Seconds
    if ($p.HasExited) {
        Write-Host "FAIL: $($exe.Name) exited by itself with $($p.ExitCode)" -ForegroundColor Red
        $failures++
        continue
    }
    [void]$p.CloseMainWindow()
    if (-not $p.WaitForExit(10000)) {
        Write-Host "NOTE: $($exe.Name) did not close when asked, stopped" -ForegroundColor Yellow
        $p.Kill()
    }
    elseif ($p.ExitCode -ne 0) {
        Write-Host "FAIL: $($exe.Name) exited with $($p.ExitCode) when closed" -ForegroundColor Red
        $failures++
    }
}
if ($failures -gt 0) { Write-Host "$failures check(s) failed" -ForegroundColor Red; exit 1 }
Write-Host 'Every standalone program starts and has its icon.' -ForegroundColor Green
exit 0
