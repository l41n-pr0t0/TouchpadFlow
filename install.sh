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

# Colour only on a real terminal, and never when NO_COLOR is set (https://no-color.org).
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != dumb ]; then
    B=$'\e[1m' D=$'\e[2m' R=$'\e[0m'
    RED=$'\e[31m' GRN=$'\e[32m' YEL=$'\e[33m' CYN=$'\e[36m'
else
    B='' D='' R='' RED='' GRN='' YEL='' CYN=''
fi
step() { printf '\n%s==>%s %s%s%s\n' "$CYN$B" "$R" "$B" "$*" "$R"; }
ok() { printf '  %s✓%s %s\n' "$GRN$B" "$R" "$*"; }
warn() { printf '  %s!%s %s\n' "$YEL$B" "$R" "$*"; }
die() { printf '\n  %s✗ error:%s %s\n' "$RED$B" "$R" "$*" >&2; exit 1; }
banner() {
    printf '\n%s  ┌──────────────────────────────┐%s\n' "$CYN" "$R"
    printf '%s  │%s  %sTOUCHPADFLOW%s %sinstaller%s      %s│%s\n' "$CYN" "$R" "$B" "$R" "$D" "$R" "$CYN" "$R"
    printf '%s  └──────────────────────────────┘%s\n' "$CYN" "$R"
    printf '  %s%s%s\n' "$D" "$*" "$R"
}
has() { command -v "$1" >/dev/null 2>&1; }
as_root() {
    if [ "$(id -u)" -eq 0 ]; then "$@"
    elif has sudo; then sudo "$@"
    elif has doas; then doas "$@"
    else warn "Needs root, run manually: $*"; return 1
    fi
}
# SIGTERM makes the engine exit within ~0.5s; wait so callers see it gone.
stop_engine() {
    pkill -x touchpadflow 2>/dev/null || return 0
    for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -x touchpadflow >/dev/null || return 0; sleep 0.2; done
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
    step "Downloading $asset"
    printf '  %s%s%s\n' "$D" "$url" "$R"
    # Progress bar on a terminal, silent otherwise.
    local progress=-sS
    [ -t 2 ] && progress=--progress-bar
    curl -fL "$progress" --retry 3 -o "$WORK/$asset" "$url" \
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

    step "Installing the app"
    put "$STAGE/touchpadflow" "$BIN/touchpadflow" 755
    put "$STAGE/touchpadflow-app" "$BIN/touchpadflow-app" 755
    sed "s|@BIN@|$BIN/touchpadflow-app|" "$STAGE/touchpadflow.desktop" > "$WORK/app.desktop"
    put "$WORK/app.desktop" "$HOME/.local/share/applications/touchpadflow.desktop"
    ok "Installed touchpadflow and touchpadflow-app to $BIN"

    # Session-only access to the touchpad and uinput (no 'input' group needed).
    step "Touchpad permissions"
    printf '  %ssudo may ask for your password once%s\n' "$D" "$R"
    # Keep a copy so the manual command printed on failure points at a real file.
    put "$STAGE/70-touchpadflow.rules" "$HOME/.local/share/touchpadflow/70-touchpadflow.rules"
    if as_root install -m 644 "$HOME/.local/share/touchpadflow/70-touchpadflow.rules" "$RULES"; then
        as_root modprobe uinput 2>/dev/null || true
        as_root udevadm control --reload-rules >/dev/null 2>&1 || true
        # Re-apply only to the devices the rule covers, quietly.
        as_root udevadm trigger --subsystem-match=input --subsystem-match=misc --subsystem-match=backlight >/dev/null 2>&1 || true
        ok "Installed $RULES"
    fi

    step "Starting the engine"
    if has systemctl && systemctl --user show-environment >/dev/null 2>&1; then
        put "$STAGE/touchpadflow.service" "$HOME/.config/systemd/user/touchpadflow.service"
        systemctl --user daemon-reload
        systemctl --user enable touchpadflow.service >/dev/null 2>&1 || true
        if systemctl --user restart touchpadflow.service; then
            ok "Engine running as systemd user service"
        else
            warn "Engine did not start; see: journalctl --user -u touchpadflow"
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
        stop_engine
        "$BIN/touchpadflow" --daemon >/dev/null 2>&1
        ok "Engine started and added to session autostart"
    fi
}

install_macos() {
    step "Installing the app"
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
    ok "Installed $MAC_APP and $BIN/touchpadflow-macos"

    step "Starting the engine"
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
    ok "Engine running as a LaunchAgent"
    warn "Allow 'touchpadflow-macos' under System Settings > Privacy & Security > Accessibility for brightness control."
}

uninstall() {
    step "Removing TouchpadFlow"
    if [ "$OS" = Darwin ]; then
        launchctl bootout "gui/$(id -u)" "$LAUNCH_AGENT" 2>/dev/null || true
        rm -rf "$LAUNCH_AGENT" "$MAC_APP" "$BIN/touchpadflow-macos" "$BIN/touchpadflow-app"
    else
        if has systemctl; then
            systemctl --user disable --now touchpadflow.service 2>/dev/null || true
            rm -f "$HOME/.config/systemd/user/touchpadflow.service"
            systemctl --user daemon-reload 2>/dev/null || true
        fi
        stop_engine
        rm -rf "$HOME/.local/share/touchpadflow"
        rm -f "$BIN/touchpadflow" "$BIN/touchpadflow-app" \
              "$HOME/.local/share/applications/touchpadflow.desktop" \
              "$HOME/.config/autostart/touchpadflow-engine.desktop"
        [ -n "${XDG_RUNTIME_DIR:-}" ] && rm -f "$XDG_RUNTIME_DIR/touchpadflow.sock"
        if [ -f "$RULES" ]; then
            as_root rm -f "$RULES" && as_root udevadm control --reload-rules || true
        fi
    fi
    ok "TouchpadFlow removed. Your settings file (~/.config/touchpadflow.json) was kept."
}

if [ "$MODE" = uninstall ]; then
    uninstall
    exit 0
fi

banner "$OS $ARCH"
if [ "$MODE" = source ]; then stage_source; else stage_download; fi
case "$OS" in
    Linux) install_linux ;;
    Darwin) install_macos ;;
esac

case ":$PATH:" in *":$BIN:"*) ;; *) warn "Add $BIN to your PATH to run touchpadflow from a terminal." ;; esac
printf '\n%s%s✓ Done.%s Open %sTouchpadFlow%s from your app menu to change settings.\n' "$GRN" "$B" "$R" "$B" "$R"
printf '  %sSlide two fingers on the left edge for brightness, right edge for volume.%s\n\n' "$D" "$R"
