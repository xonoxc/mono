#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/mono"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
AUTOSTART_DIR="${HOME}/.config/autostart"

echo "Installing Mono..."

# Build release binary
echo "Building mono-tracker..."
cd "$SCRIPT_DIR"
cargo build --release

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$SYSTEMD_DIR"
mkdir -p "$AUTOSTART_DIR"

# Install binaries
echo "Installing binaries to ~/.local/bin..."
cp target/release/mono "$INSTALL_DIR/"
cp target/release/mono-tracker "$INSTALL_DIR/"
cp target/release/mono-cli "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/mono"
chmod +x "$INSTALL_DIR/mono-tracker"
chmod +x "$INSTALL_DIR/mono-cli"

# Create systemd service
echo "Creating systemd service..."
cat > "$SYSTEMD_DIR/mono.service" << EOF
[Unit]
Description=Mono Screen Time Tracker
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/mono-tracker
Restart=on-failure
RestartSec=10

[Install]
WantedBy=graphical-session.target
EOF

# Create XDG autostart (fallback)
echo "Creating XDG autostart entry..."
cat > "$AUTOSTART_DIR/mono.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Mono Screen Time Tracker
Exec=${INSTALL_DIR}/mono-tracker
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
EOF

# Enable systemd service
echo "Enabling systemd service..."
systemctl --user daemon-reload
systemctl --user enable mono.service

echo ""
echo "Mono installed successfully!"
echo ""
echo "To start tracking now, run:"
echo "  systemctl --user start mono.service"
echo ""
echo "To check status:"
echo "  systemctl --user status mono.service"
echo ""
echo "To uninstall, run:"
echo "  ./uninstall.sh"