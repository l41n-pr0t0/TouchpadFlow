#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DEST="$HOME/.local/bin"
SERVICE_DEST="$HOME/.config/systemd/user"
RULES_DEST="/etc/udev/rules.d/70-touchpadflow.rules"

echo "=== TouchpadFlow Installer (Least Privilege & Secure) ==="
echo "Building release binary with Rust..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

mkdir -p "$BIN_DEST"
cp "$SCRIPT_DIR/target/release/touchpadflow" "$BIN_DEST/touchpadflow"
chmod 755 "$BIN_DEST/touchpadflow"
echo "[OK] Installed binary to $BIN_DEST/touchpadflow"

# Install udev rule for session-based uaccess (avoids broad 'input' group security hazard)
if [ -d "/etc/udev/rules.d" ]; then
    echo ""
    echo "Configuring session-based device permissions (uaccess)..."
    if [ "$EUID" -eq 0 ]; then
        cp "$SCRIPT_DIR/udev/70-touchpadflow.rules" "$RULES_DEST"
        udevadm control --reload-rules && udevadm trigger
        echo "[OK] Installed $RULES_DEST"
    elif command -v sudo >/dev/null 2>&1; then
        sudo cp "$SCRIPT_DIR/udev/70-touchpadflow.rules" "$RULES_DEST"
        sudo udevadm control --reload-rules && sudo udevadm trigger
        echo "[OK] Installed $RULES_DEST via sudo"
    else
        echo "[!] To enable access without adding your user to the broad 'input' group, copy:"
        echo "    sudo cp $SCRIPT_DIR/udev/70-touchpadflow.rules $RULES_DEST"
        echo "    sudo udevadm control --reload-rules && sudo udevadm trigger"
    fi
fi

# Install systemd user service if systemd is available
if command -v systemctl >/dev/null 2>&1; then
    mkdir -p "$SERVICE_DEST"
    cp "$SCRIPT_DIR/systemd/touchpadflow.service" "$SERVICE_DEST/touchpadflow.service"
    systemctl --user daemon-reload
    echo "[OK] Registered sandboxed systemd user service: $SERVICE_DEST/touchpadflow.service"
fi

echo ""
echo "=== Installation complete ==="
echo "Start daemon with systemd:"
echo "  systemctl --user enable --now touchpadflow.service"
echo ""
echo "Or run standalone in background (non-systemd distros):"
echo "  touchpadflow --daemon"
