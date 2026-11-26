# Changelog

All notable changes to this project will be documented in this file.

## [0.4.1] - 2026-11-27

### Added
- **Repeating macro swipes:** a swipe bound to a macro keeps firing every swipe-distance step while you slide, and sliding back fires the opposite direction. Bind three-finger right to `ctrl+tab` and left to `ctrl+shift+tab` to glide through browser tabs. Built-in shortcuts (media keys, desktops) still fire once.
- The settings app warns when per-app macros cannot work on KDE because `kdotool` is missing.

### Changed
- The **Update** button moved to the top right, next to Gestures.
- **Material theme replaces Sumi:** Material 3 from Sung (tonal surfaces, Google Sans Flex, 40px pill buttons, filled cards, thick tonal sliders) in neutral greys, following the system light/dark setting. Configs saved with `"sumi"` or `"glass"` load as Material.
- **System theme now works on Linux:** System and Material follow the desktop's light/dark preference (read from the settings portal, KDE and GNOME), switching live; before, Linux always fell back to dark.
- The "Ink page" toggle is now **Background** (print themes only).
- Unknown theme names in the config fall back to System instead of failing the whole load. (A 0.4.0 engine rejected the whole file when the app saved a newer theme name, silently keeping old zone and macro settings.)
- The focused window is looked up once per gesture instead of on every action.

## [0.4.0] - 2026-11-26

### Added
- **Installable apps:** `TouchpadFlow-Setup-x64.exe` / `-arm64.exe` for Windows and `TouchpadFlow-macos-universal.dmg` for macOS (the app registers its bundled engine at login).
- **Look settings:** System, Light, Dark and the new **Sumi** theme (washi paper, sumi ink, vermilion seal accent), plus an "Ink page" on/off toggle (`ink_page` in the config).
- **Ink page:** a procedurally rendered ink background (drips, halftone, hatching) fixed behind the settings window.
- **Update button** in the settings app, and a coloured, animated Windows installer.

### Fixed
- **Cursor jumps / lag on battery:** removed the 400 ms "stuck contact" watchdog. Idle pads (common on battery power saving) send no frames for a resting finger, so the watchdog lifted real touches mid-gesture.
- **Finger-lift frames were never forwarded:** the session-end frame was discarded before passthrough, so libinput kept a phantom finger down.
- **Virtual touchpad drift:** swallowed edge/gesture sessions now still forward positions (minus touches and clicks), and forwarded batches re-select their slot, so the next touch no longer starts from a stale position.
- **Lost positions:** slot X/Y are kept across touches (the kernel only re-sends values that changed).
- **Pointer latency:** single-finger touches outside the edge zones go to the pointer immediately instead of waiting 25 ms.
- **Edge taps lost:** a session that ends while settling is now delivered.
- **Settings ignored by the service:** the daemon now reads `~/.config/touchpadflow.json` by default.
- **Invalid IPC status JSON** (device path was debug-quoted).
- **Shutdown hang:** SIGTERM no longer waits for the next touch.
- **Tests changed real volume/brightness:** tests now use detached controllers.
- **macOS crate did not build** (missing `libc` dependency).

### Changed
- Settings app restyled to the MNEMOS-ZERO monochrome print look: four-tone ink/paper palette, bundled IBM Plex Mono + Space Mono, square ink buttons with hard offset shadows, halftone touchpad map. Follows the system theme; `TOUCHPADFLOW_THEME=light|dark` pins one.
- README rewritten for users, with screenshots in `docs/images/`.
- One-command install for every OS: `install.sh` (Linux any distro + macOS, `--uninstall`, `--from-source`) and `install.ps1` (Windows). A tag-triggered release workflow publishes prebuilt Linux x86_64/aarch64, macOS universal and Windows x64/arm64 downloads.
- Removed dead code and files: `uninstall.sh` (now `install.sh --uninstall`), `windows/build.ps1`, `config.example.json`, unused error variants, unused IPC `stop`, never-fired `pinch3_*` shortcuts, unused slot/engine fields, macOS `clap` dependency.
- macOS engine checks every MultitouchSupport symbol before calling it instead of crashing.
- Replaced the localhost web Control Center (`--gui`, `--port`) with the native `touchpadflow-app`.
- Edge zones are configurable per side: `left_action` / `right_action` (`brightness`, `volume`, `none`) and `left_sensitivity` / `right_sensitivity` (normalised to pad height). `volume_step` / `brightness_step` are gone.
- New `enabled` switch, honoured by the Linux, macOS and Windows engines; macOS and Windows reload the shared settings file automatically.


### Added
- **Unix Domain Socket IPC:** Added dedicated IPC socket at `$XDG_RUNTIME_DIR/touchpadflow.sock` supporting `pause`, `resume`, `toggle`, `status`, and `reload` commands.
- **CLI Commands:** Added `--pause`, `--resume`, `--status`, and `--reload` command-line flags.
- **Synthetic Touchpad Test Suite:** Added comprehensive in-memory test suite (`tests/synthetic_gestures.rs`) testing raw evdev gesture streams without hardware dependencies.
- **GitHub Actions CI Workflow:** Added `.github/workflows/ci.yml` with automated testing, linting, formatting, and cross-platform compilation.
- **Transactional Config & Live Reload:** Control Center now loads, validates, safely merges, and atomically writes configuration files, signaling the active daemon via IPC to hot-reload without restart.
- **Control Center Security:** Added cryptographic session token authentication, strict `Host` and `Origin` header validation, and Content-Length parsing.

### Fixed
- **Emergency Toggle Bug:** Replaced one-way boolean flag with atomic XOR toggle (`paused.fetch_xor(true)`) and dedicated IPC control, resolving permanent pause on toggle.
- **Left Edge Brightness Trigger:** Switched from requiring all fingers within the edge ratio to natural contact centroid tracking (`avg_x`), resolving missed gestures caused by natural hand posture.
- **Dual-GPU Backlight Control:** Added auto-discovery of all backlight controllers, synchronizing active panel backlights (e.g., `nvidia_0` and `intel_backlight`).
- **`edge_trigger_threshold` Deadband:** Implemented the previously unused `edge_trigger_threshold` configuration as an activation deadband in `track_motion()`.
- **Zero-Allocation Hot Path:** Replaced dynamic `Vec` allocations and slot cloning with fixed-capacity stack arrays (`[SlotData; 16]`).
- **Systemd Backlight Sandboxing:** Resolved `ProtectKernelTunables` conflict with `ReadWritePaths=/sys/class/backlight` and `DeviceAllow=/dev/null rw`.
- **KDE Plasma 6 OSD Integration:** Added detection for `qdbus-qt6` alongside `qdbus6` and `qdbus`.
