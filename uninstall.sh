#!/usr/bin/env bash
set -euo pipefail

BIN_PATH="$HOME/.local/bin/touchpadflow"
SERVICE_PATH="$HOME/.config/systemd/user/touchpadflow.service"
RULES_PATH="/etc/udev/rules.d/70-touchpadflow.rules"

echo "=== TouchpadFlow Uninstaller ==="

# 1. Stop and disable systemd user service if running
if command -v systemctl >/dev/null 2>&1; then
    if systemctl --user is-active --quiet touchpadflow.service 2>/dev/null; then
        echo "Stopping systemd user service..."
        systemctl --user stop touchpadflow.service || true
    fi
    if systemctl --user is-enabled --quiet touchpadflow.service 2>/dev/null; then
        echo "Disabling systemd user service..."
        systemctl --user disable touchpadflow.service || true
    fi
    if [ -f "$SERVICE_PATH" ]; then
        rm -f "$SERVICE_PATH"
        systemctl --user daemon-reload || true
        echo "[OK] Removed user service: $SERVICE_PATH"
    fi
fi

# 2. Terminate running process if any
if pgrep -x touchpadflow >/dev/null 2>&1; then
    echo "Terminating running touchpadflow processes..."
    pkill -x touchpadflow || true
fi

# 3. Remove binaries and launcher entry
for f in "$BIN_PATH" "$HOME/.local/bin/touchpadflow-app" "$HOME/.local/share/applications/touchpadflow.desktop"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        echo "[OK] Removed $f"
    fi
done

# 4. Clean runtime IPC sockets
if [ -n "${XDG_RUNTIME_DIR:-}" ] && [ -S "$XDG_RUNTIME_DIR/touchpadflow.sock" ]; then
    rm -f "$XDG_RUNTIME_DIR/touchpadflow.sock"
fi

# 5. Remove udev rules if requested
if [ -f "$RULES_PATH" ]; then
    echo ""
    read -p "Remove system udev rule ($RULES_PATH)? Requires sudo [y/N]: " -r confirm
    if [[ "$confirm" =~ ^[Yy]$ ]]; then
        if [ "$EUID" -eq 0 ]; then
            rm -f "$RULES_PATH"
            udevadm control --reload-rules && udevadm trigger
            echo "[OK] Removed $RULES_PATH"
        elif command -v sudo >/dev/null 2>&1; then
            sudo rm -f "$RULES_PATH"
            sudo udevadm control --reload-rules && sudo udevadm trigger
            echo "[OK] Removed $RULES_PATH via sudo"
        fi
    fi
fi

echo ""
echo "=== TouchpadFlow successfully uninstalled ==="
