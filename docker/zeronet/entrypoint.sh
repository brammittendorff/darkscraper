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

# Create comprehensive trackers file with known working ZeroNet trackers
mkdir -p /app/data
cat > /app/data/trackers.txt << 'EOF'
# ZeroNet-specific trackers
zero://boot3rdez4rzn36x.onion:15441
zero://zero.booth.moe#f36ca555bee6ba216b14d10f38c16f7769ff064e0e37d887603548cc2e64191d:15441
# UDP trackers (most reliable for BitTorrent DHT)
udp://tracker.coppersurfer.tk:6969
udp://9.rarbg.com:2710
udp://tracker.opentrackr.org:1337/announce
udp://tracker.internetwarriors.net:1337/announce
udp://tracker.leechers-paradise.org:6969/announce
udp://exodus.desync.com:6969/announce
udp://tracker.cyberia.is:6969/announce
udp://open.stealth.si:80/announce
udp://tracker.torrent.eu.org:451/announce
udp://tracker.tiny-vps.com:6969/announce
udp://open.demonii.com:1337/announce
udp://tracker.openbittorrent.com:6969/announce
udp://tracker.moeking.me:6969/announce
udp://explodie.org:6969/announce
udp://tracker1.bt.moack.co.kr:80/announce
udp://tracker.uw0.xyz:6969/announce
udp://tracker.dler.org:6969/announce
udp://retracker.lanta-net.ru:2710/announce
udp://denis.stalker.upeer.me:6969/announce
# HTTP trackers (fallback)
http://tracker.opentrackr.org:1337/announce
http://explodie.org:6969/announce
http://tracker2.itzmx.com:6961/announce
http://tracker1.itzmx.com:8080/announce
http://open.acgnxtracker.com:80/announce
http://t.nyaatracker.com:80/announce
http://retracker.mgts.by:80/announce
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
