# TouchpadFlow Project Roadmap

## Phase 1: Reliability, IPC & Security (Current)
- [x] Bidirectional emergency pause/resume toggle with atomic state switching.
- [x] Unix Domain Socket IPC daemon control (`--pause`, `--resume`, `--toggle`, `--status`, `--reload`).
- [x] Zero-allocation hot path event processing in `gesture.rs`.
- [x] Natural finger centroid tracking for left and right edges.
- [x] Enforcement of `edge_trigger_threshold` as an activation deadband.
- [x] Systemd sandboxing hardening (`ReadWritePaths=/sys/class/backlight` and `ProtectKernelTunables=no`).
- [x] Hardened Control Center GUI: random session token authentication, Origin/Host validation, and streaming Content-Length parsing.
- [x] Safe transactional configuration updating with atomic file renaming.
- [x] Dual-GPU / hybrid backlight auto-discovery and synchronization (Intel + NVIDIA).

## Phase 2: Testing Infrastructure & Automation
- [x] In-memory Synthetic Touchpad Event testing framework (`tests/synthetic_gestures.rs`).
- [x] Multi-platform GitHub Actions CI pipeline (Linux, macOS, Windows).
- [ ] Property-based testing for contact settling and jitter rejection (`proptest`).
- [ ] Standalone TouchpadFlow Hardware Diagnostic & Trace Replay CLI.

## Phase 3: Cross-Platform Architecture
- [ ] Extraction of shared core gesture state machine into a `touchpadflow-core` platform-agnostic crate.
- [ ] Common configuration schema and action mapping representation across Linux, macOS, and Windows.
- [ ] Unified platform adapter trait (`PlatformInput`, `PlatformOutput`, `PlatformBacklight`, `PlatformAudio`).

## Phase 4: Distribution & Packaging
- [x] Dedicated uninstaller script (`uninstall.sh`).
- [ ] Arch Linux AUR package (`PKGBUILD`).
- [ ] NixOS Flake & Home Manager module.
- [ ] Debian/Ubuntu (`.deb`) and Fedora/RHEL (`.rpm`) packaging.
- [ ] Signed Windows installer (`.msi`) & macOS signed DMG distribution.

## Phase 5: Hardware Compatibility Database
- [ ] Public hardware compatibility matrix categorized by laptop vendor (Lenovo, Dell, ASUS, HP, Apple, Framework).
- [ ] Diagnostic reporter command (`touchpadflow --diagnose`) generating anonymized device capability reports.
