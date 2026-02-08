#!/bin/bash
set -e

# Get container hostname
HOSTNAME=$(hostname)

# Disable broken ZeroName plugin
mkdir -p /app/plugins/disabled
mv /app/plugins/Zeroname /app/plugins/disabled/ 2>/dev/null || true

# Start ZeroNet with UI accessible from Docker network
exec python3 zeronet.py \
    --ui_ip 0.0.0.0 \
    --ui_host "${HOSTNAME}:43110" \
    --ui_port 43110 \
    --fileserver_port 26552 \
    --disable_zeronamelocal \
    --log_dir /app/data/logs
