#!/bin/bash
#
# Rational Reserve Daemon Installation Script
# Installs systemd service files and enables daemons at boot
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Rational Reserve Daemon Installation ==="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Warning: Not running as root. systemd service installation may fail."
    echo "Please run with: sudo $0"
    echo ""
fi

# Copy service files to systemd directory
SYSTEMD_DIR="/etc/systemd/system"

echo "Installing systemd service files..."

if [ -w "$SYSTEMD_DIR" ]; then
    cp "$SCRIPT_DIR/rr-integrity-daemon.service" "$SYSTEMD_DIR/"
    cp "$SCRIPT_DIR/rr-threat-manager.service" "$SYSTEMD_DIR/"
    cp "$SCRIPT_DIR/rr-morpho-maintainer.service" "$SYSTEMD_DIR/"
    cp "$SCRIPT_DIR/rr-morpho-maintainer.timer" "$SYSTEMD_DIR/"
    
    # Reload systemd
    systemctl daemon-reload
    
    # Enable services
    echo "Enabling services..."
    systemctl enable rr-integrity-daemon.service
    systemctl enable rr-threat-manager.service
    systemctl enable rr-morpho-maintainer.timer
    
    # Start services
    echo "Starting services..."
    systemctl start rr-integrity-daemon.service
    systemctl start rr-threat-manager.service
    systemctl start rr-morpho-maintainer.timer
    
    echo ""
    echo "=== Installation Complete ==="
    echo ""
    echo "Services enabled and started:"
    systemctl status rr-integrity-daemon.service --no-pager -l
    echo ""
    systemctl status rr-threat-manager.service --no-pager -l
    echo ""
    systemctl status rr-morpho-maintainer.timer --no-pager -l
else
    echo "Cannot write to $SYSTEMD_DIR"
    echo ""
    echo "Manual installation steps:"
    echo "1. Copy service files to $SYSTEMD_DIR"
    echo "2. Run: systemctl daemon-reload"
    echo "3. Run: systemctl enable rr-integrity-daemon rr-threat-manager rr-morpho-maintainer.timer"
    echo "4. Run: systemctl start rr-integrity-daemon rr-threat-manager rr-morpho-maintainer.timer"
fi
