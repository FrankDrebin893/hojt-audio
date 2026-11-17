# Downloads the pinned Mesa software OpenGL build (mesa-dist-win) and unpacks its x64 DLLs
# (opengl32.dll, libgallium_wgl.dll and the rest of the folder), for the smoke test of the standalone
# programs on a runner without a GPU (scripts\smoke-standalones.ps1 -MesaDir). Windows only
# (release.yml, the Windows job):
#
#   scripts\fetch-mesa.ps1 [-Out target\mesa]
#
# The SHA-256 is checked after the download; on a mismatch the computed hash is printed and the
# script fails, never swapping the value in silently. The build is used for the smoke step only,
# and is never shipped. To bump it: change $Version and $Sha256 together.
param([string]$Out = 'target/mesa')

$ErrorActionPreference = 'Stop'
$Version = '26.2.4'
$Sha256 = '351fc8c8b695878ffb3eaa044b3ead08672a48b1a045e3c3e3975811df0f6695'
$Url = "https://github.com/pal1000/mesa-dist-win/releases/download/$Version/mesa3d-$Version-release-msvc.7z"

$work = Join-Path ([System.IO.Path]::GetTempPath()) "mesa-$Version"
New-Item -ItemType Directory -Force -Path $work, $Out | Out-Null
$archive = Join-Path $work 'mesa.7z'
Invoke-WebRequest -Uri $Url -OutFile $archive

$got = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
if ($got -ne $Sha256) {
    Write-Host "FAIL: SHA-256 of $Url is $got, expected $Sha256" -ForegroundColor Red
    exit 1
}
Write-Host "Mesa $Version verified ($got)"

$listing = & 7z l -ba $archive
$listing | ForEach-Object { Write-Host $_ }
if (-not ($listing -match 'x64[\\/]opengl32\.dll')) { throw 'no x64/opengl32.dll in the Mesa archive' }
& 7z e -y "-o$Out" $archive 'x64/*.dll' | Out-Null
if ($LASTEXITCODE -ne 0) { throw "7z failed with $LASTEXITCODE" }
if (-not (Test-Path -LiteralPath (Join-Path $Out 'opengl32.dll'))) { throw "opengl32.dll was not extracted to $Out" }
Write-Host "Mesa DLLs in ${Out}:"
Get-ChildItem -LiteralPath $Out | ForEach-Object { Write-Host "  $($_.Name)" }
