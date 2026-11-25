# TouchpadFlow for macOS

Native macOS trackpad edge gesture engine written in **100% Rust**, inspired by IslamovMukhammad's TrackpadEdge and Apple MultitouchSupport research.

## Features

- **Zero-Latency Two-Finger Edge Sliders:**
  - **Left Edge:** Adjusts display brightness with native macOS brightness feedback.
  - **Right Edge:** Adjusts system audio output volume.
- **Apple MultitouchSupport.framework Integration:**
  - Connects directly to Apple's private `MultitouchSupport` C ABI to receive raw, normalized multi-touch frames without needing high-overhead accessibility polling or cursor-hijacking.
- **Safe, Self-Contained Rust:**
  - Compiled directly into a standalone release binary.
  - Zero third-party dependencies outside standard macOS system frameworks.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.sh | bash
```

This installs `touchpadflow-macos` to `~/.local/bin`, `TouchpadFlow.app` (settings) to `~/Applications`, and a LaunchAgent (`com.touchpadflow.engine`) that starts the engine at login.
Settings are read from `~/.config/touchpadflow.json` and reloaded within a second whenever the app saves them.

## Building from source

```bash
cargo build --release --manifest-path macos/Cargo.toml   # engine
cargo build --release --bin touchpadflow-app             # settings app
```
