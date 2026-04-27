#!/bin/bash
set -e

INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/mono"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
AUTOSTART_DIR="${HOME}/.config/autostart"
DATA_DIR="${HOME}/.local/share/screen-time-tracker"

echo "Uninstalling Mono..."

# Stop running mono-tracker (both systemd and manual)
echo "Stopping mono-tracker..."
pkill -9 mono-tracker 2>/dev/null || true
systemctl --user stop mono.service 2>/dev/null || true
systemctl --user disable mono.service 2>/dev/null || true

# Remove binaries
echo "Removing binaries from ~/.local/bin..."
rm -f "$INSTALL_DIR/mono"
rm -f "$INSTALL_DIR/mono-tracker"
rm -f "$INSTALL_DIR/mono-cli"

# Remove systemd service
echo "Removing systemd service..."
rm -f "$SYSTEMD_DIR/mono.service"

# Remove XDG autostart
echo "Removing XDG autostart entry..."
rm -f "$AUTOSTART_DIR/mono.desktop"

# Remove config directory (includes consent file)
echo "Removing config..."
rm -rf "$CONFIG_DIR"

# Reload systemd
systemctl --user daemon-reload 2>/dev/null || true

# Remove database
echo "Removing database..."
rm -rf "$DATA_DIR"

echo ""
echo "Mono uninstalled successfully!"