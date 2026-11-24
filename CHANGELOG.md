# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-11-24

### Added
- **Unix Domain Socket IPC:** Added dedicated IPC socket at `$XDG_RUNTIME_DIR/touchpadflow.sock` supporting `pause`, `resume`, `toggle`, `status`, and `reload` commands.
- **CLI Commands:** Added `--pause`, `--resume`, `--status`, and `--reload` command-line flags.
- **Synthetic Touchpad Test Suite:** Added comprehensive in-memory test suite (`tests/synthetic_gestures.rs`) testing raw evdev gesture streams without hardware dependencies.
- **GitHub Actions CI Workflow:** Added `.github/workflows/ci.yml` with automated testing, linting, formatting, and cross-platform compilation.
- **Transactional Config & Live Reload:** Control Center now loads, validates, safely merges, and atomically writes configuration files, signaling the active daemon via IPC to hot-reload without restart.
- **Control Center Security:** Added cryptographic session token authentication, strict `Host` and `Origin` header validation, and Content-Length parsing.
- **Dedicated Uninstaller:** Added `uninstall.sh` to cleanly stop services, remove binaries, clean runtime sockets, and remove udev rules.

### Fixed
- **Emergency Toggle Bug:** Replaced one-way boolean flag with atomic XOR toggle (`paused.fetch_xor(true)`) and dedicated IPC control, resolving permanent pause on toggle.
- **Left Edge Brightness Trigger:** Switched from requiring all fingers within the edge ratio to natural contact centroid tracking (`avg_x`), resolving missed gestures caused by natural hand posture.
- **Dual-GPU Backlight Control:** Added auto-discovery of all backlight controllers, synchronizing active panel backlights (e.g., `nvidia_0` and `intel_backlight`).
- **`edge_trigger_threshold` Deadband:** Implemented the previously unused `edge_trigger_threshold` configuration as an activation deadband in `track_motion()`.
- **Zero-Allocation Hot Path:** Replaced dynamic `Vec` allocations and slot cloning with fixed-capacity stack arrays (`[SlotData; 16]`).
- **Systemd Backlight Sandboxing:** Resolved `ProtectKernelTunables` conflict with `ReadWritePaths=/sys/class/backlight` and `DeviceAllow=/dev/null rw`.
- **KDE Plasma 6 OSD Integration:** Added detection for `qdbus-qt6` alongside `qdbus6` and `qdbus`.
