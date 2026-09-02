#!/bin/bash
#
# Rational Reserve Daemon Startup Script
# Launches all system daemon agents
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(dirname "$SCRIPT_DIR")"
BIN="$APP_DIR/target/release/morphlex"
LOG_DIR="$APP_DIR/logs"
DATA_DIR="$APP_DIR/data"

if [ "${RR_START_NON_PRODUCTION_SCAFFOLDING:-0}" != "1" ]; then
    echo "Refusing to start RR daemon scaffolding without RR_START_NON_PRODUCTION_SCAFFOLDING=1"
    echo "These daemons are not production security controls."
    exit 1
fi

if [ "${RR_ALLOW_SYSTEM_PATH_MONITORING:-0}" = "1" ]; then
    MONITOR_PATHS=("$APP_DIR" "/etc" "/var")
else
    MONITOR_PATHS=("$APP_DIR")
fi

# Create directories
mkdir -p "$LOG_DIR" "$DATA_DIR"

echo "=== Rational Reserve Daemon Startup ==="
echo "Time: $(date)"
echo ""

# Check if binary exists
if [ ! -x "$BIN" ]; then
    echo "Error: morphlex binary not found at $BIN"
    echo "Please run: cargo build --release"
    exit 1
fi

# Function to start a daemon
start_daemon() {
    local name=$1
    shift
    echo "Starting $name..."
    nohup "$BIN" "$@" >> "$LOG_DIR/$name.log" 2>&1 &
    local pid=$!
    echo "  PID: $pid"
    echo $pid > "$DATA_DIR/$name.pid"
}

# Start System Integrity Daemon
integrity_args=(daemon integrity)
for monitor_path in "${MONITOR_PATHS[@]}"; do
    integrity_args+=(--monitor "$monitor_path")
done
integrity_args+=(--check-interval 60)
start_daemon "integrity-daemon" "${integrity_args[@]}"

# Start Threat Intelligence Manager
start_daemon "threat-manager" \
    daemon threat \
    --scan-interval 30

# Start Convergence Manager (runs as needed, not continuously)
echo "Convergence Manager: On-demand (not started as daemon)"

# Check if it's 4 AM for morphogenetic maintenance
current_hour=$(date +%H)
if [ "$current_hour" = "04" ]; then
    echo "Running morphogenetic maintenance (4 AM)..."
    "$BIN" daemon maintenance --run >> "$LOG_DIR/morpho-maintenance.log" 2>&1
else
    echo "Morphogenetic Maintainer: Scheduled for 4 AM daily"
fi

echo ""
echo "=== All Daemons Started ==="
echo ""
echo "Logs directory: $LOG_DIR"
echo "Data directory: $DATA_DIR"
echo ""
echo "To check status:"
echo "  systemctl status rr-integrity-daemon"
echo "  systemctl status rr-threat-manager"
echo "  systemctl status rr-morpho-maintainer.timer"
echo ""
echo "To view logs:"
echo "  journalctl -u rr-integrity-daemon -f"
echo "  journalctl -u rr-threat-manager -f"
echo "  journalctl -u rr-morpho-maintainer -f"
