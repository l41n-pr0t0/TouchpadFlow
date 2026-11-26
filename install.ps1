# TouchpadFlow installer for Windows 10/11 (x64 and ARM64). No admin rights needed.
#
#   irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1 | iex
#
# Uninstall:
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1))) -Uninstall
param([switch]$Uninstall)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # we draw our own bar; the built-in one is slow and ugly
try { [Console]::OutputEncoding = [Text.UTF8Encoding]::new() } catch {}

function Step($msg) { Write-Host ''; Write-Host '==> ' -NoNewline -ForegroundColor Cyan; Write-Host $msg -ForegroundColor White }
function Ok($msg)   { Write-Host '  v ' -NoNewline -ForegroundColor Green; Write-Host $msg }
function Download($url, $out) {
    Add-Type -AssemblyName System.Net.Http
    $client = New-Object System.Net.Http.HttpClient
    $client.Timeout = [TimeSpan]::FromMinutes(10)
    $resp = $client.GetAsync($url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    $null = $resp.EnsureSuccessStatusCode()
    $total = [long]($resp.Content.Headers.ContentLength)
    $in = $resp.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
    $file = [IO.File]::Create($out)
    $buf = New-Object byte[] 65536
    $spin = '|/-\'; $i = 0; $got = 0L; $width = 24
    try {
        while (($n = $in.Read($buf, 0, $buf.Length)) -gt 0) {
            $file.Write($buf, 0, $n); $got += $n; $i++
            if ($i % 4 -eq 0) {
                $mb = '{0:N1}' -f ($got / 1MB)
                if ($total -gt 0) {
                    $pct = [int](100 * $got / $total); $f = [int]($width * $pct / 100)
                    $bar = ('#' * $f) + ('-' * ($width - $f))
                    [Console]::Write("`r  $($spin[$i % 4]) [$bar] $pct%  $mb / $('{0:N1}' -f ($total / 1MB)) MB ")
                } else { [Console]::Write("`r  $($spin[$i % 4]) $mb MB ") }
            }
        }
    } finally { $file.Dispose(); $in.Dispose(); $client.Dispose() }
    [Console]::Write("`r" + (' ' * 70) + "`r")
}


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

Write-Host ''
Write-Host '  +------------------------------+' -ForegroundColor Cyan
Write-Host '  |  TOUCHPADFLOW installer      |' -ForegroundColor Cyan
Write-Host '  +------------------------------+' -ForegroundColor Cyan
Write-Host "  Windows $arch" -ForegroundColor DarkGray

Step "Downloading $asset"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Download "https://github.com/$Repo/releases/latest/download/$asset" $zip
Ok ('Downloaded ({0:N1} MB)' -f ((Get-Item $zip).Length / 1MB))

Step 'Installing'
New-Item $Dir -ItemType Directory -Force | Out-Null
Expand-Archive $zip -DestinationPath $Dir -Force
Remove-Item $zip
Get-ChildItem $Dir | Unblock-File
Ok "Installed to $Dir"

$shell = New-Object -ComObject WScript.Shell
New-Item $StartMenu -ItemType Directory -Force | Out-Null
foreach ($entry in @(@('TouchpadFlow', 'touchpadflow-app.exe'), @('TouchpadFlow Engine', 'TouchpadFlow.exe'))) {
    $link = $shell.CreateShortcut((Join-Path $StartMenu "$($entry[0]).lnk"))
    $link.TargetPath = Join-Path $Dir $entry[1]
    $link.WorkingDirectory = $Dir
    $link.Save()
}
Ok 'Added Start menu entries'

Step 'Starting the engine'
# Same Run value the engine's own "Start with Windows" tray option uses.
$engine = Join-Path $Dir 'TouchpadFlow.exe'
Set-ItemProperty $RunKey -Name TouchpadFlow -Value "`"$engine`""
Start-Process $engine
Ok 'Engine started (tray icon) and set to start with Windows'
Write-Host ''
Write-Host 'Done. ' -NoNewline -ForegroundColor Green
Write-Host 'Open "TouchpadFlow" from the Start menu to change settings.'
