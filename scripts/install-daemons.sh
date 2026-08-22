#!/usr/bin/env bash
#
# Rational Reserve Daemon Installation Script
# Installs non-production scaffolding as user systemd units only.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(dirname "$SCRIPT_DIR")"
USER_CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
SYSTEMD_USER_DIR="$USER_CONFIG_HOME/systemd/user"

echo "=== Rational Reserve Daemon Installation ==="
echo "Status: NON-PRODUCTION SCAFFOLDING"
echo ""

if [ "${RR_INSTALL_NON_PRODUCTION_SCAFFOLDING:-}" != "1" ]; then
    echo "Refusing to install placeholder RR daemons."
    echo "Set RR_INSTALL_NON_PRODUCTION_SCAFFOLDING=1 only for local scaffolding tests."
    exit 1
fi

if [ "$EUID" -eq 0 ]; then
    echo "Refusing root install. These placeholder daemons may only be installed as user units."
    exit 1
fi

echo "Installing user systemd service files under $SYSTEMD_USER_DIR"
mkdir -p "$SYSTEMD_USER_DIR"
install -m 0644 "$SCRIPT_DIR/rr-integrity-daemon.service" "$SYSTEMD_USER_DIR/"
install -m 0644 "$SCRIPT_DIR/rr-threat-manager.service" "$SYSTEMD_USER_DIR/"
install -m 0644 "$SCRIPT_DIR/rr-morpho-maintainer.service" "$SYSTEMD_USER_DIR/"
install -m 0644 "$SCRIPT_DIR/rr-morpho-maintainer.timer" "$SYSTEMD_USER_DIR/"

systemctl --user daemon-reload
systemctl --user enable rr-integrity-daemon.service
systemctl --user enable rr-threat-manager.service
systemctl --user enable rr-morpho-maintainer.timer

if [ "${RR_START_NON_PRODUCTION_SCAFFOLDING:-}" = "1" ]; then
    systemctl --user start rr-integrity-daemon.service
    systemctl --user start rr-threat-manager.service
    systemctl --user start rr-morpho-maintainer.timer
fi

echo "Installed user units. They are non-production scaffolding and are not root system services."
