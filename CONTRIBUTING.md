# Contributing to TouchpadFlow

Thank you for your interest in contributing to TouchpadFlow! We welcome contributions across all supported platforms (Linux, macOS, and Windows).

## Development Setup

### Linux
Prerequisites: Rust 1.85+, `libudev-dev` / `systemd-devel`.
```bash
cargo check
cargo test
cargo clippy --all-targets
```

### Testing Without Touchpad Hardware
TouchpadFlow includes an in-memory **Synthetic Touchpad Gesture Test Suite** that simulates low-level physical touch contact sequences without requiring hardware access:
```bash
cargo test --test synthetic_gestures
```

### Code Style & Guidelines
- **Pure Safe Rust:** The codebase enforces `#![forbid(unsafe_code)]`.
- **Zero-Allocation Hot Path:** Event frame handling in `gesture.rs` must not allocate on the heap. Avoid constructing dynamic vectors or cloning in `process_frame()` / `track_motion()`.
- **Formatting:** Run `cargo fmt --check` before opening pull requests.
- **Clippy:** Run `cargo clippy --all-targets -- -D warnings`.

## Submitting Pull Requests
1. Fork the repository and create a feature branch (`git checkout -b feature/my-improvement`).
2. Include automated unit or synthetic tests covering new gesture patterns or edge cases.
3. Ensure CI passes cleanly.
4. Submit your pull request with a descriptive summary of changes and tested devices.
