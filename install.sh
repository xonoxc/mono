#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/mono"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
AUTOSTART_DIR="${HOME}/.config/autostart"

ENABLE_TRACKING=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --no-track)
            ENABLE_TRACKING=false
            shift
            ;;
        --force)
            FORCE=true
            shift
            ;;
        *)
            echo "Usage: $0 [--no-track] [--force]"
            echo "  --no-track    Install binaries only, do not enable tracking"
            echo "  --force     Overwrite existing installation"
            exit 1
            ;;
    esac
done

if [[ -f "$INSTALL_DIR/mono" && "${FORCE:-false}" != "true" ]]; then
    echo "Mono is already installed. Run with --force to reinstall."
    exit 1
fi

echo "Installing Mono..."

# Build release binary
echo "Building mono-tracker..."
cd "$SCRIPT_DIR"
cargo build --release

# Create directories
echo "Creating directories..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$SYSTEMD_DIR"
mkdir -p "$AUTOSTART_DIR"

# Stop running mono-tracker before overwriting binary
if [[ "${FORCE:-false}" == "true" ]]; then
    echo "Stopping mono-tracker for upgrade..."
    systemctl --user stop mono-tracker.service 2>/dev/null || true
    pkill -9 mono-tracker 2>/dev/null || true
    sleep 1
fi

# Install binaries
echo "Installing binaries to ~/.local/bin..."
cp target/release/mono "$INSTALL_DIR/"
cp target/release/mono-tracker "$INSTALL_DIR/"
cp target/release/mono-cli "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/mono"
chmod +x "$INSTALL_DIR/mono-tracker"
chmod +x "$INSTALL_DIR/mono-cli"

# Remove old/duplicate services
echo "Cleaning up old services..."
rm -f "$SYSTEMD_DIR/screen-time-tracker.service"
rm -f "$AUTOSTART_DIR/screen-time-tracker.desktop"

# Create XDG autostart (primary method - works across all desktop environments)
echo "Creating XDG autostart entry..."
cat > "$AUTOSTART_DIR/mono-tracker.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Mono Screen Time Tracker
Exec=${INSTALL_DIR}/mono-tracker
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Comment=Privacy-first screen time monitoring daemon
EOF

# Create systemd service (fallback for systems without XDG autostart)
echo "Creating systemd service..."
cat > "$SYSTEMD_DIR/mono-tracker.service" << EOF
[Unit]
Description=Mono Screen Time Tracker
After=graphical-session.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/mono-tracker
Restart=on-failure
RestartSec=10

[Install]
WantedBy=graphical-session.target
EOF

# Enable systemd service (fallback method)
echo "Enabling systemd service..."
systemctl --user daemon-reload
systemctl --user enable mono-tracker.service 2>/dev/null || true

# Verify systemd service is enabled
echo "Verifying systemd service..."
if ! systemctl --user is-enabled mono-tracker.service &>/dev/null; then
    echo "Note: systemd service not enabled (will rely on XDG autostart)"
fi
echo "Systemd service configured"

# Setup tracking if enabled
if [[ "$ENABLE_TRACKING" == "true" ]]; then
    echo "Setting up tracking..."
    echo "1" > "$CONFIG_DIR/consent"
    # Try to start via systemd first, fall back to direct execution
    systemctl --user start mono-tracker.service 2>/dev/null || nohup "$INSTALL_DIR/mono-tracker" &>/dev/null &
fi

echo ""
echo "Mono installed successfully!"
echo ""

if [[ "$ENABLE_TRACKING" == "true" ]]; then
    echo "Tracking is enabled and daemon is running."
    echo ""
    echo "To check status:"
    echo "  systemctl --user status mono-tracker.service"
    echo "  or simply: mono-tracker is-running"
    echo ""
    echo "To disable tracking:"
    echo "  ./uninstall.sh"
else
    echo "Binaries installed but tracking is NOT enabled."
    echo ""
    echo "To enable tracking later, run:"
    echo "  mono-cli setup"
    echo ""
    echo "Or start the daemon manually:"
    echo "  mono-tracker"
fi