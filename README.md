# TouchpadFlow

TouchpadFlow is a low-latency touchpad gesture daemon written in Rust for Linux and KDE Plasma 6 (Wayland). It intercepts physical multitouch events directly from `/dev/input/event*`, routes edge slide gestures to hardware volume and display brightness controls, passes normal pointer and scrolling motions through a virtual UInput device, and executes 3- and 4-finger window/media gestures.

## Install

Every download lives on one page: **https://github.com/l41n-pr0t0/TouchpadFlow/releases/latest**

Or install in one command:

**Linux (any distro) and macOS**, in a terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.sh | bash
```

**Windows 10/11**, in PowerShell (no admin needed):
```powershell
irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1 | iex
```

The installer picks the right build for your system (x86_64 or ARM), installs the engine and the **TouchpadFlow** settings app, adds it to your app launcher / Start menu, and starts the engine at login.
On Linux it asks for `sudo` once, to install the udev rule that gives your login session access to the touchpad. On macOS, allow `touchpadflow-macos` under *Privacy & Security > Accessibility* so it can change brightness.

| | Uninstall |
| :--- | :--- |
| Linux / macOS | `curl -fsSL https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.sh \| bash -s -- --uninstall` |
| Windows | `& ([scriptblock]::Create((irm https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.ps1))) -Uninstall` |

Build from source instead (needs Rust 1.95+): `git clone https://github.com/l41n-pr0t0/TouchpadFlow && cd TouchpadFlow && ./install.sh --from-source`.
Linux builds target glibc 2.35+ (Ubuntu 22.04, Debian 12, Fedora 36 and newer); older distros should use `--from-source`.

## Platforms

- **Linux (Rust Daemon):** High-performance Rust daemon for KDE Plasma 6, Hyprland, Sway, and GNOME utilizing `evdev`, `uinput`, `wpctl`, and `brightnessctl`. See instructions below.
- **macOS (Rust Daemon):** Native Apple Silicon / Intel daemon in `macos/` written in pure Rust, interfacing with Apple's `MultitouchSupport` framework. See [macos/README.md](macos/README.md).
- **Windows (PTP Tool):** Portable tray utility in `windows/` inspired by Wintego's SurfaceEdge architecture, using Windows Precision Touchpad HID Raw Input, Core Audio, WMI, and native Windows 11 OSD flyouts. See [windows/README.md](windows/README.md).

## TouchpadFlow app (native settings window)

`touchpadflow-app` is a native Rust desktop app (egui). It has no browser, no web server and no network access, and it runs on KDE, GNOME and other Linux desktops, plus Windows and macOS.

```bash
touchpadflow-app        # or open "TouchpadFlow" from your app launcher
```

| Setting | What it does |
| :--- | :--- |
| **Gestures on/off** | Off hands the touchpad straight back to the system |
| **Touchpad map** | Drag the zone lines to set how much of the pad each edge zone takes (5–45%) |
| **Controls** | What each edge changes: Brightness, Volume, or Nothing. Defaults: left = brightness, right = volume |
| **Sensitivity** | How much one full top-to-bottom slide changes the value (20–400%) |
| **Advanced** | Dead zone before sliding starts, and how long an edge touch waits for a second finger |

Changes save automatically to the shared settings file and apply immediately:
Linux and macOS use `~/.config/touchpadflow.json`, Windows uses `%APPDATA%\TouchpadFlow\touchpadflow.json`.
The app shows whether the Linux engine is running, paused or stopped, and reports save or config errors inline.

## Gesture Layout

```
+-------------------+-----------------------------+-------------------+
|     LEFT ZONE     |         CENTER ZONE         |    RIGHT ZONE     |
|   (Brightness)    |      (Normal Scrolling)     |     (Volume)      |
|                   |                             |                   |
|  [F1] [F2]  ▲     |   Pass-through to KWin/     |     ▲  [F1] [F2]  |
|             │     |   libinput pointer engine   |     │             |
|             ▼     |                             |     ▼             |
+-------------------+-----------------------------+-------------------+
        22%                       56%                         22%
```

| Gesture | Action | Implementation |
| :--- | :--- | :--- |
| **Two fingers slide (Right Edge)** | Volume Up / Down | `wpctl` sink adjustment + KDE OSD notification |
| **Two fingers slide (Left Edge)** | Brightness Up / Down | `brightnessctl` adjustment + KDE OSD notification |
| **Two fingers slide (Center)** | Standard Scroll | Forwarded transparently to compositor |
| **Three fingers swipe Up / Down** | Volume Up / Down | Continuous delta tracking |
| **Three fingers swipe Left / Right** | Previous / Next Song | Simulated media scancodes (`KEY_PREVIOUSSONG`, `KEY_NEXTSONG`) |
| **Three fingers tap** | Middle Click | Emits `BTN_MIDDLE` |
| **Four fingers swipe Up / Down** | Overview / Show Desktop | KWin shortcut invocation |
| **Four fingers swipe Left / Right** | Previous / Next Virtual Desktop | KWin shortcut invocation |

## Architecture & Mechanics

### 1. Zero-Allocation Event Loop
The daemon opens the raw physical touchpad device with exclusive access (`EVIOCGRAB`). The native loop reads events via `evdev` into stack buffers without heap allocations in the hot path.

### 2. Edge Zone Normalization
Touchpad coordinate spaces differ between hardware vendors. TouchpadFlow reads the physical axis limits (`ABS_MT_POSITION_X` min/max) directly from the device capabilities and maps incoming coordinates to normalized values $x \in [0.0, 1.0]$.
Touches whose initial contact falls within the outermost 22% of the pad lock the session into Edge Mode. Center touches pass to the virtual touchpad clone without latency.

### 3. Settle Window
Multi-finger contacts stagger physically over 10–30 ms scan intervals. TouchpadFlow buffers raw events during a 25 ms settling window. If a multi-finger gesture or edge gesture is identified, the buffer is dropped and virtual slots are cleanly cleared, preventing false single-finger clicks or cursor jumps.

### 4. Asynchronous Controller Workers
Subprocess calls to system utilities (`wpctl`, `brightnessctl`, `qdbus`) run on dedicated worker threads with condition variables. Changes are coalesced over 25 ms intervals so hardware event processing never blocks.

## Security & Least-Privilege Architecture

TouchpadFlow strictly adheres to the principle of least privilege across all supported platforms. The daemon **never runs as root or administrator**.

### 1. Linux Permissions & Sandboxing
- **Active Session Device Permissions (`uaccess`):** Instead of adding the user to the broad `input` group—which grants unconstrained read access to all system keyboards and creates a major security hazard—TouchpadFlow uses a dedicated udev rule (`udev/70-touchpadflow.rules`). This rule tags physical touchpads and `/dev/uinput` with `TAG+="uaccess"`, granting access dynamically only to the currently active graphical seat via `systemd-logind`.
- **Systemd Sandboxing:** The provided systemd user unit enforces strict sandboxing:
  - `NoNewPrivileges=yes`: Disallows privilege escalation.
  - `IPAddressDeny=any`: Blocks all outbound and inbound network sockets.
  - `DevicePolicy=closed` & `DeviceAllow`: Restricts hardware access exclusively to `/dev/uinput` and character input devices.
  - `ProtectSystem=strict` & `ProtectHome=read-only`: Hardens the filesystem.
- **Rootless Helper Fallback:** Non-systemd distributions (Alpine, Void, Gentoo) can run `touchpadflow --daemon` under the user session without root.

### 2. Windows & macOS Least Privilege
- **Windows:** Runs completely in user space without requiring administrator rights or third-party kernel drivers. Communicates with touchpads using standard Precision Touchpad (PTP) HID Raw Input.
- **macOS:** Interacts cleanly through Apple's native `MultitouchSupport` API, operating under standard user permissions with explicit macOS Input Monitoring access.

---

## Input Privacy

- **No Keyboard Capture:** TouchpadFlow binds exclusively to absolute multitouch coordinate axes (`ABS_MT_POSITION_X/Y`). It does not capture, read, or intercept keyboard strokes.
- **Zero Movement Storage:** Touch coordinates are evaluated purely in ephemeral stack memory for edge detection and delta accumulation. No trace histories or raw events are saved to disk or persistent storage.
- **Zero Network Transmission:** The core engine does not establish internet connections or upload telemetry. Complete network isolation is guaranteed at both application and systemd container levels.
- **Ephemeral Processing:** Coordinates and contact states are discarded immediately upon finger release (`ABS_MT_TRACKING_ID == -1`).

---

## Fault Tolerance & Crash Recovery

Input daemon failures must never compromise the user's primary pointer device. TouchpadFlow provides comprehensive fault tolerance:

1. **RAII Device Grab (`GrabGuard`):** Physical touchpad exclusivity is managed by an RAII guard. When the daemon terminates, drops out of scope, or encounters an interrupt (`SIGINT`, `SIGTERM`), the hardware grab is released immediately.
2. **Panic Hook Restoration:** A global `std::panic::set_hook` intercepts unexpected panics, immediately ungrabbing the device and restoring native hardware control before unwinding.
3. **Hot-Plug & Disconnect Tolerance:** Touchpad disconnections (`ENODEV`, device unplug) are caught gracefully. The engine flushes virtual slots, releases virtual state, and enters an automatic reconnect loop until the device returns.
4. **Dropped-event recovery:** the kernel `SYN_DROPPED` resync (handled by `evdev`) restores slot state, so no timeout is needed. A timeout-based watchdog was removed because idle pads (e.g. on battery) stop reporting resting fingers and it lifted real touches.
5. **Emergency Disable Mechanism:** Run `touchpadflow --toggle` or send `SIGUSR1` to immediately pause gesture processing and release the hardware grab, without killing the background service.

---

## Runtime requirements (Linux)

- `uinput` kernel support (standard on every distro)
- Audio: PipeWire (`wpctl`), PulseAudio (`pactl`), or ALSA (`amixer`)
- Backlight: `brightnessctl`, `light`, or direct sysfs backlight access (granted by the udev rule)

Live engine log: `journalctl --user -u touchpadflow -f`

## CLI Usage

```text
Usage: touchpadflow [OPTIONS]

Options:
  -d, --device <DEVICE>  Path to the touchpad evdev device (e.g. /dev/input/event9)
  -c, --config <CONFIG>  Path to JSON configuration file
      --grab             Exclusively grab the physical touchpad device [default: true]
      --monitor          Monitor events without grabbing or filtering
      --daemon           Run in background as a daemon (portable across non-systemd distros)
      --toggle           Emergency toggle: pauses or resumes the running daemon
  -h, --help             Print help
```

## Configuration

The app writes `~/.config/touchpadflow.json` (or `$XDG_CONFIG_HOME/touchpadflow.json`); the daemon reads it on start and on `touchpadflow --reload`. You can also edit it by hand. Out-of-range values are clamped, and a broken file is rejected on reload so the running settings stay in place:

```json
{
  "enabled": true,
  "left_edge_ratio": 0.22,
  "right_edge_ratio": 0.22,
  "left_action": "brightness",
  "right_action": "volume",
  "left_sensitivity": 1.5,
  "right_sensitivity": 1.5,
  "edge_trigger_threshold": 35.0,
  "settle_window_ms": 25,
  "tap_threshold": 150.0,
  "tap_duration_limit_s": 0.30,
  "shortcuts": {
    "three_finger_tap": "key:BTN_MIDDLE",
    "swipe3_left": "key:KEY_PREVIOUSSONG",
    "swipe3_right": "key:KEY_NEXTSONG",
    "swipe4_up": "shortcut:Overview",
    "swipe4_down": "shortcut:Show Desktop",
    "swipe4_left": "shortcut:Switch to Previous Desktop",
    "swipe4_right": "shortcut:Switch to Next Desktop"
  }
}
```

## License

Apache License 2.0. See [LICENSE](LICENSE) for details.
