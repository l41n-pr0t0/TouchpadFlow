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

## Building on Windows

Prerequisites: [.NET 8 SDK or newer](https://dotnet.microsoft.com/download).

Run from PowerShell:

```powershell
.\build.ps1
```

For ARM64 devices (such as Surface Pro / Surface Laptop Copilot+ PCs):
```powershell
.\build.ps1 -Architecture win-arm64
```

This compiles a single-file, self-contained `TouchpadFlow.exe` that runs without requiring any installation or administrative privileges.

## Running

1. Double-click `TouchpadFlow.exe`.
2. It minimizes to the Windows notification tray.
3. Slide two fingers vertically along the:
   - **Left edge (outer 18%):** Adjusts screen brightness.
   - **Right edge (outer 18%):** Adjusts sound volume.
4. Right-click the tray icon to switch between Two-Finger and One-Finger mode, enable Start with Windows, or exit.
