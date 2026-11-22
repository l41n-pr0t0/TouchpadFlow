#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DEST="$HOME/.local/bin"
SERVICE_DEST="$HOME/.config/systemd/user"

echo "Building TouchpadFlow release binary..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

mkdir -p "$BIN_DEST"
cp "$SCRIPT_DIR/target/release/touchpadflow" "$BIN_DEST/touchpadflow"
chmod +x "$BIN_DEST/touchpadflow"
echo "Installed binary to $BIN_DEST/touchpadflow"

mkdir -p "$SERVICE_DEST"
cp "$SCRIPT_DIR/systemd/touchpadflow.service" "$SERVICE_DEST/touchpadflow.service"
echo "Installed systemd unit to $SERVICE_DEST/touchpadflow.service"

systemctl --user daemon-reload

echo ""
echo "Permission check:"
if ! groups | grep -q '\binput\b'; then
    echo "  [!] User '$USER' is not in the 'input' group."
    echo "      Run: sudo usermod -aG input $USER"
    echo "      Then log out and log back in."
fi

if [ ! -f /etc/udev/rules.d/99-uinput.rules ]; then
    echo "  [!] Missing /etc/udev/rules.d/99-uinput.rules."
    echo "      Create it with:"
    echo '      echo '"'"'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"'"'"' | sudo tee /etc/udev/rules.d/99-uinput.rules'
    echo "      sudo udevadm control --reload-rules && sudo udevadm trigger"
fi

echo ""
echo "Setup complete. Start TouchpadFlow with:"
echo "  systemctl --user enable --now touchpadflow.service"
