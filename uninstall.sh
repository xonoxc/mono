#!/bin/bash
set -e

INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/mono"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
AUTOSTART_DIR="${HOME}/.config/autostart"

echo "Uninstalling Mono..."

# Stop and disable systemd service
echo "Stopping systemd service..."
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

echo ""
echo "Mono uninstalled successfully!"
echo ""
echo "Note: Your data is preserved at ~/.local/share/mono/"
echo "To remove all data including database:"
echo "  rm -rf ~/.local/share/mono"