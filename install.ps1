# TouchpadFlow installer for Windows 10/11 (x64 and ARM64). No admin rights needed.
#
#   irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1 | iex
#
# Uninstall:
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1))) -Uninstall
param([switch]$Uninstall)
$ErrorActionPreference = 'Stop'

$Repo      = 'l41n-pr0t0/TouchpadFlow'
$Dir       = Join-Path $env:LOCALAPPDATA 'Programs\TouchpadFlow'
$StartMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'TouchpadFlow'
$RunKey    = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

# Running copies lock their .exe files; stop them before replacing or removing.
Get-Process TouchpadFlow, touchpadflow-app -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

if ($Uninstall) {
    Remove-ItemProperty $RunKey -Name TouchpadFlow -ErrorAction SilentlyContinue
    Remove-Item $Dir, $StartMenu -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host '[OK] TouchpadFlow removed. Settings in %APPDATA%\TouchpadFlow were kept.'
    return
}

$isArm = $env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64'
$arch  = if ($isArm) { 'arm64' } else { 'x64' }
$asset = "touchpadflow-windows-$arch.zip"
$zip   = Join-Path $env:TEMP $asset

Write-Host "=== TouchpadFlow installer (Windows $arch) ==="
Write-Host "Downloading $asset"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest "https://github.com/$Repo/releases/latest/download/$asset" -OutFile $zip -UseBasicParsing

New-Item $Dir -ItemType Directory -Force | Out-Null
Expand-Archive $zip -DestinationPath $Dir -Force
Remove-Item $zip
Get-ChildItem $Dir | Unblock-File
Write-Host "[OK] Installed to $Dir"

$shell = New-Object -ComObject WScript.Shell
New-Item $StartMenu -ItemType Directory -Force | Out-Null
foreach ($entry in @(@('TouchpadFlow', 'touchpadflow-app.exe'), @('TouchpadFlow Engine', 'TouchpadFlow.exe'))) {
    $link = $shell.CreateShortcut((Join-Path $StartMenu "$($entry[0]).lnk"))
    $link.TargetPath = Join-Path $Dir $entry[1]
    $link.WorkingDirectory = $Dir
    $link.Save()
}
Write-Host '[OK] Added Start menu entries'

# Same Run value the engine's own "Start with Windows" tray option uses.
$engine = Join-Path $Dir 'TouchpadFlow.exe'
Set-ItemProperty $RunKey -Name TouchpadFlow -Value "`"$engine`""
Start-Process $engine
Write-Host '[OK] Engine started (tray icon) and set to start with Windows'
Write-Host ''
Write-Host 'Done. Open "TouchpadFlow" from the Start menu to change settings.'
