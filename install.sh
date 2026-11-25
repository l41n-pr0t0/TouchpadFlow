#!/usr/bin/env bash
# TouchpadFlow installer for Linux (any distro) and macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/l41n-pr0t0/TouchpadFlow/main/install.sh | bash
#
# Options (append after "bash -s --" when piping):
#   --from-source   build with cargo from this checkout instead of downloading
#   --uninstall     remove TouchpadFlow
set -euo pipefail

REPO="l41n-pr0t0/TouchpadFlow"
BIN="$HOME/.local/bin"
MODE=download
for arg in "$@"; do
    case "$arg" in
        --from-source) MODE=source ;;
        --uninstall) MODE=uninstall ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$ARCH" in amd64) ARCH=x86_64 ;; arm64) ARCH=aarch64 ;; esac

say() { printf '%s\n' "$*"; }
die() { say "error: $*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }
as_root() {
    if [ "$(id -u)" -eq 0 ]; then "$@"
    elif has sudo; then sudo "$@"
    else say "[!] Needs root, run manually: $*"; return 1
    fi
}
# Copy via temp file + rename so a running binary is replaced safely.
put() { install -d "$(dirname "$2")"; cp "$1" "$2.tmp"; chmod "${3:-644}" "$2.tmp"; mv -f "$2.tmp" "$2"; }

LAUNCH_AGENT="$HOME/Library/LaunchAgents/com.touchpadflow.engine.plist"
MAC_APP="$HOME/Applications/TouchpadFlow.app"
RULES=/etc/udev/rules.d/70-touchpadflow.rules

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/touchpadflow"

stage_download() {
    has curl || die "curl is required"
    local asset
    case "$OS" in
        Linux) asset="touchpadflow-linux-$ARCH.tar.gz" ;;
        Darwin) asset="touchpadflow-macos-universal.tar.gz" ;;
        *) die "unsupported system $OS (on Windows use install.ps1)" ;;
    esac
    local url="https://github.com/$REPO/releases/latest/download/$asset"
    say "Downloading $url"
    curl -fsSL --retry 3 -o "$WORK/$asset" "$url" \
        || die "no prebuilt download for $OS/$ARCH. Clone the repo and run: ./install.sh --from-source"
    tar -xzf "$WORK/$asset" -C "$WORK"
}

stage_source() {
    local src
    src="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    [ -f "$src/Cargo.toml" ] || die "--from-source must be run from a TouchpadFlow checkout"
    has cargo || die "cargo is required (https://rustup.rs)"
    mkdir -p "$STAGE"
    cargo build --release --bins --manifest-path "$src/Cargo.toml"
    cp "$src/target/release/touchpadflow-app" "$STAGE/"
    if [ "$OS" = Darwin ]; then
        cargo build --release --manifest-path "$src/macos/Cargo.toml"
        cp "$src/macos/target/release/touchpadflow-macos" "$STAGE/"
    else
        cp "$src/target/release/touchpadflow" "$src/udev/70-touchpadflow.rules" \
           "$src/systemd/touchpadflow.service" "$src/linux/touchpadflow.desktop" "$STAGE/"
    fi
}

install_linux() {
    # Catches a too-old glibc before anything is changed.
    "$STAGE/touchpadflow" --help >/dev/null 2>&1 \
        || die "the prebuilt binary does not run on this distro; clone the repo and run ./install.sh --from-source"

    put "$STAGE/touchpadflow" "$BIN/touchpadflow" 755
    put "$STAGE/touchpadflow-app" "$BIN/touchpadflow-app" 755
    sed "s|@BIN@|$BIN/touchpadflow-app|" "$STAGE/touchpadflow.desktop" > "$WORK/app.desktop"
    put "$WORK/app.desktop" "$HOME/.local/share/applications/touchpadflow.desktop"
    say "[OK] Installed touchpadflow and touchpadflow-app to $BIN"

    # Session-only access to the touchpad and uinput (no 'input' group needed).
    say "Installing device permissions (sudo may ask for your password)..."
    if as_root install -m 644 "$STAGE/70-touchpadflow.rules" "$RULES"; then
        as_root modprobe uinput 2>/dev/null || true
        as_root udevadm control --reload-rules || true
        as_root udevadm trigger || true
        say "[OK] Installed $RULES"
    fi

    if has systemctl && systemctl --user show-environment >/dev/null 2>&1; then
        put "$STAGE/touchpadflow.service" "$HOME/.config/systemd/user/touchpadflow.service"
        systemctl --user daemon-reload
        systemctl --user enable touchpadflow.service >/dev/null 2>&1 || true
        if systemctl --user restart touchpadflow.service; then
            say "[OK] Engine running as systemd user service"
        else
            say "[!] Engine did not start; see: journalctl --user -u touchpadflow"
        fi
    else
        # Non-systemd distros: start with the desktop session instead.
        cat > "$WORK/autostart.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TouchpadFlow Engine
Exec=$BIN/touchpadflow
NoDisplay=true
X-GNOME-Autostart-enabled=true
EOF
        put "$WORK/autostart.desktop" "$HOME/.config/autostart/touchpadflow-engine.desktop"
        pkill -x touchpadflow 2>/dev/null || true
        "$BIN/touchpadflow" --daemon >/dev/null
        say "[OK] Engine started and added to session autostart"
    fi
}

install_macos() {
    put "$STAGE/touchpadflow-macos" "$BIN/touchpadflow-macos" 755

    # Minimal .app bundle so the settings app shows up in Launchpad/Spotlight.
    rm -rf "$MAC_APP"
    put "$STAGE/touchpadflow-app" "$MAC_APP/Contents/MacOS/touchpadflow-app" 755
    cat > "$MAC_APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>TouchpadFlow</string>
  <key>CFBundleIdentifier</key><string>com.touchpadflow.app</string>
  <key>CFBundleExecutable</key><string>touchpadflow-app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
    xattr -dr com.apple.quarantine "$MAC_APP" "$BIN/touchpadflow-macos" 2>/dev/null || true
    codesign --force -s - "$MAC_APP" >/dev/null 2>&1 || true
    ln -sf "$MAC_APP/Contents/MacOS/touchpadflow-app" "$BIN/touchpadflow-app"
    say "[OK] Installed $MAC_APP and $BIN/touchpadflow-macos"

    mkdir -p "$(dirname "$LAUNCH_AGENT")"
    cat > "$LAUNCH_AGENT" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.touchpadflow.engine</string>
  <key>ProgramArguments</key><array><string>$BIN/touchpadflow-macos</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict></plist>
EOF
    launchctl bootout "gui/$(id -u)" "$LAUNCH_AGENT" 2>/dev/null || true
    launchctl bootstrap "gui/$(id -u)" "$LAUNCH_AGENT"
    say "[OK] Engine running as a LaunchAgent"
    say "[!] Allow 'touchpadflow-macos' under System Settings > Privacy & Security > Accessibility for brightness control."
}

uninstall() {
    if [ "$OS" = Darwin ]; then
        launchctl bootout "gui/$(id -u)" "$LAUNCH_AGENT" 2>/dev/null || true
        rm -rf "$LAUNCH_AGENT" "$MAC_APP" "$BIN/touchpadflow-macos" "$BIN/touchpadflow-app"
    else
        if has systemctl; then
            systemctl --user disable --now touchpadflow.service 2>/dev/null || true
            rm -f "$HOME/.config/systemd/user/touchpadflow.service"
            systemctl --user daemon-reload 2>/dev/null || true
        fi
        pkill -x touchpadflow 2>/dev/null || true
        rm -f "$BIN/touchpadflow" "$BIN/touchpadflow-app" \
              "$HOME/.local/share/applications/touchpadflow.desktop" \
              "$HOME/.config/autostart/touchpadflow-engine.desktop"
        [ -n "${XDG_RUNTIME_DIR:-}" ] && rm -f "$XDG_RUNTIME_DIR/touchpadflow.sock"
        if [ -f "$RULES" ]; then
            as_root rm -f "$RULES" && as_root udevadm control --reload-rules || true
        fi
    fi
    say "[OK] TouchpadFlow removed. Your settings file (~/.config/touchpadflow.json) was kept."
}

if [ "$MODE" = uninstall ]; then
    uninstall
    exit 0
fi

say "=== TouchpadFlow installer ($OS $ARCH) ==="
if [ "$MODE" = source ]; then stage_source; else stage_download; fi
case "$OS" in
    Linux) install_linux ;;
    Darwin) install_macos ;;
esac

case ":$PATH:" in *":$BIN:"*) ;; *) say "[!] Add $BIN to your PATH to run touchpadflow from a terminal." ;; esac
say ""
say "Done. Open \"TouchpadFlow\" from your app launcher (or run touchpadflow-app) to change settings."
