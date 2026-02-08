#!/bin/bash
set -e

# Get container hostname
HOSTNAME=$(hostname)

# Disable broken ZeroName plugin, but ENABLE BootStrapper for peer discovery
mkdir -p /app/plugins/disabled
mv /app/plugins/Zeroname /app/plugins/disabled/ 2>/dev/null || true

# Enable BootStrapper plugin for better peer discovery
if [ -d "/app/plugins/Bootstrapper" ] && [ ! -f "/app/plugins/disabled/Bootstrapper" ]; then
    echo "BootStrapper plugin enabled for peer discovery"
fi

# Create trackers file with CURRENT working trackers (updated Feb 8, 2026)
# Source: https://github.com/ngosang/trackerslist (updated daily)
mkdir -p /app/data
cat > /app/data/trackers.txt << 'EOF'
# Working UDP trackers from ngosang/trackerslist (Feb 8, 2026)
udp://tracker.opentrackr.org:1337/announce
udp://open.demonii.com:1337/announce
udp://open.tracker.cl:1337/announce
udp://open.stealth.si:80/announce
udp://tracker.torrent.eu.org:451/announce
udp://wepzone.net:6969/announce
udp://tracker1.myporn.club:9337/announce
udp://tracker.theoks.net:6969/announce
udp://tracker.srv00.com:6969/announce
udp://tracker.qu.ax:6969/announce
udp://tracker.corpscorp.online:80/announce
udp://tracker.bittor.pw:1337/announce
udp://tracker.1h.is:1337/announce
udp://tracker-udp.gbitt.info:80/announce
udp://t.overflow.biz:6969/announce
udp://opentracker.io:6969/announce
udp://leet-tracker.moe:1337/announce
udp://zer0day.ch:1337/announce
udp://utracker.ghostchu-services.top:6969/announce
udp://tracker.zupix.online:6969/announce
udp://tracker.tryhackx.org:6969/announce
udp://tracker.torrust-demo.com:6969/announce
udp://tracker.theoks.net:6969/announce
udp://tracker.opentorrent.top:6969/announce
udp://tracker.gmi.gd:6969/announce
udp://tracker.fnix.net:6969/announce
udp://tracker.dler.org:6969/announce
udp://explodie.org:6969/announce
udp://evan.im:6969/announce
# HTTPS trackers (best)
https://tracker.zhuqiy.com:443/announce
https://tracker.pmman.tech:443/announce
https://tracker.moeking.me:443/announce
EOF

# Start ZeroNet with enhanced peer discovery and aggressive settings
exec python3 zeronet.py \
    --ui-ip 0.0.0.0 \
    --ui-host "${HOSTNAME}:43110" \
    --ui-port 43110 \
    --fileserver-port 15441 \
    --log-dir /app/data/logs \
    --trackers-file /app/data/trackers.txt \
    --tor disable \
    --threads-db 8 \
    --threads-crypt 8 \
    --threads-fs-read 8 \
    --max-files-opened 1024
