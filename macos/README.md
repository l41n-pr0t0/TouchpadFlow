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

## Building on macOS

Prerequisites: Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

```bash
cd macos
cargo build --release
```

The resulting binary will be at `macos/target/release/touchpadflow-macos`.

## Running

```bash
./macos/target/release/touchpadflow-macos
```

Options:
```text
Options:
      --left-edge <LEFT_EDGE>    Left edge strip width ratio (0.0 to 0.4) [default: 0.20]
      --right-edge <RIGHT_EDGE>  Right edge strip width ratio (0.0 to 0.4) [default: 0.20]
  -h, --help                     Print help
```

## Running as a macOS LaunchAgent (Background Daemon)

To start automatically when you log into macOS:

1. Copy the binary to `/usr/local/bin`:
   ```bash
   sudo cp target/release/touchpadflow-macos /usr/local/bin/
   ```

2. Create `~/Library/LaunchAgents/com.touchpadflow.daemon.plist`:
   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.touchpadflow.daemon</string>
       <key>ProgramArguments</key>
       <array>
           <string>/usr/local/bin/touchpadflow-macos</string>
       </array>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
   </dict>
   </plist>
   ```

3. Load the service:
   ```bash
   launchctl load ~/Library/LaunchAgents/com.touchpadflow.daemon.plist
   ```
