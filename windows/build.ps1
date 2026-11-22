param(
    [string]$Architecture = "win-x64",
    [string]$DotNet = "dotnet"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=== TouchpadFlow Windows Build ==="
Write-Host "Target Architecture: $Architecture"
Write-Host "Project Directory:   $ScriptDir"
Write-Host ""

& $DotNet publish "$ScriptDir\TouchpadFlow.csproj" `
    -c Release `
    -r $Architecture `
    --self-contained true `
    -p:PublishSingleFile=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -o "$ScriptDir\bin\Release\$Architecture"

$ExePath = "$ScriptDir\bin\Release\$Architecture\TouchpadFlow.exe"
if (Test-Path $ExePath) {
    Copy-Item $ExePath "$ScriptDir\TouchpadFlow.exe" -Force
    Write-Host ""
    Write-Host "[OK] Successfully built TouchpadFlow.exe at:"
    Write-Host "     $ScriptDir\TouchpadFlow.exe"
} else {
    Write-Error "Build output executable not found."
}
