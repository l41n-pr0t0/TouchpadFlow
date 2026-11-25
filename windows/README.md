# TouchpadFlow for Windows

Inspired by Wintego's SurfaceEdge architecture, TouchpadFlow for Windows brings native edge-swipe controls to any Windows 10/11 laptop equipped with a Precision Touchpad (Surface Laptop, Dell XPS, Lenovo ThinkPad, HP Spectre, ASUS ZenBook, etc.).

## Key Features

- **Two-Finger Edge Slide (Default):**
  - Left edge vertical slide: Controls display backlight brightness.
  - Right edge vertical slide: Controls default audio playback volume.
- **One-Finger Mode Support:** Toggleable directly from the system tray menu.
- **Real Windows 11 OSD:** Invokes the native Windows 11 volume and brightness flyouts via the internal ImmersiveShell COM service.
- **Direct Precision Touchpad HID Reports:** Uses Windows Raw Input (`Page 0x0D`, `Usage 0x05`) to directly track multi-finger coordinates with zero driver installation.
- **WMI & Core Audio Integration:**
  - Volume via `IAudioEndpointVolume` (with automatic unmute when adjusting upward).
  - Backlight via `WmiMonitorBrightnessMethods` (`WmiSetBrightness`).
- **Coalesced Background Worker:** Audio and brightness I/O runs asynchronously off the input thread so touchpad tracking remains 100% fluid.
- **System Tray Management:** Easy toggling for Start with Windows, pausing, and mode switching.

## Install

In PowerShell (no admin rights needed):

```powershell
irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1 | iex
```

This installs `TouchpadFlow.exe` (engine, tray icon) and `touchpadflow-app.exe` (settings) to `%LOCALAPPDATA%\Programs\TouchpadFlow`, adds Start menu entries, and starts the engine with Windows.
Zone widths, actions, sensitivity and on/off come from `%APPDATA%\TouchpadFlow\touchpadflow.json` (written by the settings app) and reload within a second.

## Building from source

Prerequisites: [.NET 8 SDK](https://dotnet.microsoft.com/download) and Rust.

```powershell
dotnet publish windows/TouchpadFlow.csproj -c Release -r win-x64 --self-contained -p:PublishSingleFile=true   # or win-arm64
cargo build --release --bin touchpadflow-app
```

## Tray menu

Right-click the tray icon to switch between Two-Finger and One-Finger mode, pause, toggle Start with Windows, or exit.
