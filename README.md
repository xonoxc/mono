# Mono - Screen Time Tracker for Linux

<p align="center">
  <img src="public/ss.png" alt="Mono TUI Dashboard" width="800"/>
</p>

A privacy-first screen time tracking application for Linux with a polished terminal-based user interface (TUI). Mono runs silently in the background to track your application usage while providing a beautiful, htop-inspired dashboard for visualization.

---

## Features

- **Privacy-First**: All data stored locally in SQLite, never leaves your machine
- **TUI Dashboard**: Beautiful terminal interface inspired by htop, btop, and lazygit
- **Background Tracking**: Runs as a daemon on system startup
- **Real-Time Updates**: Live tracking of active applications
- **Weekly Overview**: Visual bar chart of daily screen time
- **Application Stats**: Detailed breakdown of time spent per application
- **Hyprland Support**: Integrated with Hyprland window manager

---

## Requirements

- **Linux only** (tested on Arch Linux with Hyprland)
- **Rust** 1.70 or later
- **Cargo** (included with Rust)
- **SQLite** (bundled with the application)
- **Hyprland** (optional, for window title tracking)

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/anomalyco/mono.git
cd mono

# Build the application
cargo build --release

# Install binaries
sudo cp target/release/mono /usr/local/bin/
sudo cp target/release/mono-daemon /usr/local/bin/

# Run the application
mono
```

### Quick Start

```bash
# Build in debug mode for testing
cargo build

# Run the TUI dashboard
cargo run --bin mono

# The daemon runs automatically on first launch
# when you click "Enable Tracking"
```

---

## Usage

### TUI Controls

| Key | Action |
|-----|--------|
| `j` / `Down` | Scroll down (applications list) |
| `k` / `Up` | Scroll up (applications list) |
| `h` / `Left` | Previous day |
| `l` / `Right` | Next day |
| `g` | Go to today |
| `r` | Refresh data |
| `Tab` | Switch between sections |
| `q` | Quit |

### First Run

On first launch, Mono displays a consent prompt:

- **Enable Tracking**: Starts the background daemon and enables autostart
- **Skip for Now**: Opens the dashboard without tracking

The daemon automatically:
- Registers with XDG autostart (`~/.config/autostart/`)
- Creates a systemd service (optional)

---

## Architecture

### Binaries

| Binary | Purpose |
|--------|---------|
| `mono` | TUI Dashboard (terminal interface) |
| `mono-daemon` | Background tracking daemon |

### Components

```
src/
├── main.rs          # Daemon entry point
├── lib.rs           # Core library
├── tracker.rs       # Active window tracking
├── session_manager.rs  # Session management
├── storage.rs       # SQLite database
├── autostart.rs     # Autostart registration
├── window_manager.rs # Window manager integration
├── tui/
│   ├── main.rs     # TUI dashboard
│   ├── db.rs      # Database queries
│   └── consent.rs # Consent handling
```

### Data Storage

- **Database**: `~/.local/share/mono/mono.db` (SQLite)
- **Config**: `~/.config/mono/`
- **Consent**: `~/.config/mono/consent`

---

## Development

### Building

```bash
# Build debug version
cargo build

# Build release version
cargo build --release

# Run TUI
cargo run --bin mono

# Run daemon
cargo run --bin mono-daemon
```

### Project Structure

```
mono/
├── Cargo.toml      # Rust package definition
├── src/            # Source code
│   ├── tui/       # Terminal UI
��   ├── *.rs        # Core components
├── public/        # Static assets
│   └── ss.png     # Screenshot
├── AGENTS.md       # Development guidelines
└── README.md      # This file
```

### Key Dependencies

- **ratatui**: Terminal UI framework
- **rusqlite**: SQLite bindings
- **sysinfo**: System information
- **x11**: X11 window bindings
- **chrono**: Date/time handling

---

## Configuration

### Manual Autostart

To enable autostart manually:

```bash
# Create autostart directory
mkdir -p ~/.config/autostart

# Create desktop entry
cat > ~/.config/autostart/mono.desktop << EOF
[Desktop Entry]
Type=Application
Name=Mono
Exec=mono-daemon
EOF
```

### Database Location

```bash
# View database
sqlite3 ~/.local/share/mono/mono.db

# Check schema
sqlite3 ~/.local/share/mono/mono.db ".schema"
```

---



## Troubleshooting

### No Display

If running in a non-interactive terminal:

```bash
# Check if running in TTY
tty

# Redirect output to file
mono > output.log 2>&1
```

### Permission Denied

```bash
# Fix database permissions
chmod 755 ~/.local/share/mono
chmod 644 ~/.local/share/mono/mono.db
```

### Hyprland Not Detected

The daemon will fallback to basic tracking if Hyprland is not detected. Window titles require Hyprland.


