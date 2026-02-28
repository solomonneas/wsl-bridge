# wsl-bridge

Auto-forward WSL ports to Windows. No more broken netsh rules after WSL restarts.

## The Problem

Every WSL restart gives your instance a new IP address. Windows port forwarding (`netsh interface portproxy`) uses hardcoded IPs. Rules break silently. You manually fix them. Repeat.

## The Solution

A single Rust binary that:
- **Monitors** WSL IP changes (5-second polling)
- **Auto-detects** ports from PM2 processes and Caddy config
- **Updates** Windows netsh portproxy rules automatically
- **Persists** manual port configs in `~/.config/wsl-port-forwarder/ports.toml`

## Install

```bash
curl -L https://github.com/solomonneas/wsl-bridge/releases/latest/download/wsl-port -o ~/.local/bin/wsl-port
chmod +x ~/.local/bin/wsl-port
```

Or build from source:
```bash
git clone https://github.com/solomonneas/wsl-bridge.git
cd wsl-bridge
cargo build --release
cp target/release/wsl-port ~/.local/bin/
```

## Usage

```bash
wsl-port status          # Show current IP, ports, and netsh mappings
wsl-port add 5178        # Add a port to forward
wsl-port remove 5178     # Remove a port
wsl-port sync            # Force immediate re-sync of all rules
wsl-port daemon          # Run background daemon
```

## Auto-start with systemd

```bash
# Enable user service (recommended)
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/wsl-port.service << 'SERVICE'
[Unit]
Description=WSL Port Forwarder
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/wsl-port daemon
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
SERVICE

systemctl --user daemon-reload
systemctl --user enable wsl-port
systemctl --user start wsl-port
```

## How It Works

1. **Detection**: Scans `pm2 jlist` and `http://localhost:2019/config/` (Caddy admin API)
2. **Monitoring**: Polls `hostname -I` for IP changes every 5 seconds
3. **Action**: Runs `netsh interface portproxy` via PowerShell interop when IP changes
4. **Cleanup**: Deletes old rules before adding new ones to avoid conflicts

## Config

File: `~/.config/wsl-port-forwarder/ports.toml`

```toml
manual_ports = [5173, 8080]
auto_detect_pm2 = true
auto_detect_caddy = true
```

## Requirements

- WSL2 with Windows 10/11
- `powershell.exe` accessible from WSL (standard WSL install)
- systemd (optional, for auto-start service)

## License

MIT
