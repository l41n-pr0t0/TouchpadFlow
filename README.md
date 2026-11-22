# TouchpadFlow

TouchpadFlow is a low-latency touchpad gesture daemon written in Rust for Linux and KDE Plasma 6 (Wayland). It intercepts physical multitouch events directly from `/dev/input/event*`, routes edge slide gestures to hardware volume and display brightness controls, passes normal pointer and scrolling motions through a virtual UInput device, and executes 3- and 4-finger window/media gestures.

## Platforms

- **Linux (Rust Daemon):** High-performance Rust daemon for KDE Plasma 6, Hyprland, Sway, and GNOME utilizing `evdev`, `uinput`, `wpctl`, and `brightnessctl`. See instructions below.
- **Windows (PTP Tool):** Portable tray utility in `windows/` inspired by Wintego's SurfaceEdge architecture, using Windows Precision Touchpad HID Raw Input, Core Audio, WMI, and native Windows 11 OSD flyouts. See [windows/README.md](windows/README.md).

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

## Prerequisites

- Rust toolchain (1.85+)
- Linux with `uinput` kernel support
- PipeWire (`wpctl`) for audio
- `brightnessctl` for backlight control
- User in the `input` group

## Installation

```bash
git clone git@github.com:l41n-pr0t0/TouchpadFlow.git
cd TouchpadFlow
./install.sh
```

Ensure permissions are active:
```bash
sudo usermod -aG input $USER
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Start the systemd user service:
```bash
systemctl --user enable --now touchpadflow.service
```

Inspect live output:
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
