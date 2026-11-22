# TouchpadFlow

TouchpadFlow is a low-latency touchpad gesture daemon written in Rust for Linux and KDE Plasma 6 (Wayland). It intercepts physical multitouch events directly from `/dev/input/event*`, routes edge slide gestures to hardware volume and display brightness controls, passes normal pointer and scrolling motions through a virtual UInput device, and executes 3- and 4-finger window/media gestures.

## Platforms

- **Linux (Rust Daemon):** High-performance Rust daemon for KDE Plasma 6, Hyprland, Sway, and GNOME utilizing `evdev`, `uinput`, `wpctl`, and `brightnessctl`. See instructions below.
- **macOS (Rust Daemon):** Native Apple Silicon / Intel daemon in `macos/` written in pure Rust, interfacing with Apple's `MultitouchSupport` framework. See [macos/README.md](macos/README.md).
- **Windows (PTP Tool):** Portable tray utility in `windows/` inspired by Wintego's SurfaceEdge architecture, using Windows Precision Touchpad HID Raw Input, Core Audio, WMI, and native Windows 11 OSD flyouts. See [windows/README.md](windows/README.md).

## TouchpadFlow Control Center (GUI)

TouchpadFlow includes a built-in interactive Control Center. Launch it with:

```bash
touchpadflow --gui
```

| Feature | Purpose |
| :--- | :--- |
| **Gesture editor** | Interactive sliders to configure left and right edge boundaries (5% to 35%) |
| **Sensitivity** | Adjust volume/brightness step scaling, velocity multipliers, and contact settling |
| **Live preview** | Visualise real-time multi-finger movement and touch coordinates on a canvas |
| **Action mapping** | Assign volume, brightness, media keys, or custom shortcuts to edges & gestures |
| **Device profiles** | Save, load, and switch between profiles (e.g. Ultra-Responsive, Wide Edges) |
| **Gesture testing** | Live test bench with interactive gauges to test gestures without altering hardware |
| **Service status** | Live engine status indicator with Start, Stop, and Restart controls |

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
4. **Stuck Contact Watchdog:** A 400ms idle watchdog timer audits multi-finger slots. If an incomplete release sequence occurs at the hardware level, the watchdog cancels orphaned contact states.
5. **Emergency Disable Mechanism:** Run `touchpadflow --toggle` or send `SIGUSR1` to immediately pause gesture processing and release the hardware grab, without killing the background service.

---

## Prerequisites

- Rust toolchain (1.85+)
- Linux with `uinput` kernel support
- Audio: PipeWire (`wpctl`), PulseAudio (`pactl`), or ALSA (`amixer`)
- Backlight: `brightnessctl`, `light`, or direct sysfs backlight permissions
- Graphical desktop: KDE Plasma 6, Hyprland, Sway, GNOME, or any Wayland/X11 environment

## Installation

```bash
git clone git@github.com:l41n-pr0t0/TouchpadFlow.git
cd TouchpadFlow
./install.sh
```

The installer compiles the release binary and installs the session-based uaccess udev rules and systemd service:

```bash
# Enable and start the sandboxed user service:
systemctl --user enable --now touchpadflow.service

# Or run in background on non-systemd distros:
touchpadflow --daemon
```

Inspect live status:
```bash
journalctl --user -u touchpadflow.service -f
```

## CLI Usage

```text
Usage: touchpadflow [OPTIONS]

Options:
  -d, --device <DEVICE>  Path to the touchpad evdev device (e.g. /dev/input/event9)
  -c, --config <CONFIG>  Path to JSON configuration file
      --grab             Exclusively grab the physical touchpad device [default: true]
      --monitor          Monitor events without grabbing or filtering
      --daemon           Run in background as a daemon (portable across non-systemd distros)
      --gui              Launch the interactive TouchpadFlow Control Center GUI
      --port <PORT>      Port for the Control Center GUI [default: 8123]
      --toggle           Emergency toggle: pauses or resumes the running daemon
  -h, --help             Print help
```

## Configuration

Custom configuration files can be placed at `~/.config/touchpadflow.json`:

```json
{
  "left_edge_ratio": 0.22,
  "right_edge_ratio": 0.22,
  "edge_trigger_threshold": 35.0,
  "settle_window_ms": 25,
  "volume_step": 0.02,
  "brightness_step": 0.02,
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
