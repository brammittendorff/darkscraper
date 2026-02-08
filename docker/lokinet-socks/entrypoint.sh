#!/bin/bash
set -e

# Ensure TUN device exists
mkdir -p /dev/net
if [ ! -c /dev/net/tun ]; then
    mknod /dev/net/tun c 10 200
fi

# Point DNS at lokinet's resolver
echo 'nameserver 127.3.2.1' > /etc/resolv.conf

# --- First-run init ---
# /var/lib/lokinet is a Docker volume. On first run it's empty;
# on subsequent runs it has persisted keys + nodedb.
# We always (re)deploy our config override and generate the base
# config if missing, so image updates take effect immediately.
mkdir -p /var/lib/lokinet/conf.d
cp /etc/lokinet-docker.ini /var/lib/lokinet/conf.d/00-docker.ini

if [ ! -f /var/lib/lokinet/lokinet.ini ]; then
    echo "First run: generating lokinet base config..."
    lokinet -g /var/lib/lokinet/lokinet.ini
fi

# Re-bootstrap on every start to get fresh router list
lokinet-bootstrap 2>/dev/null || echo "WARN: bootstrap fetch failed, using cached"

# Start lokinet in background
echo "Starting lokinet daemon..."
/usr/bin/lokinet &
LOKINET_PID=$!

# Wait for lokitun0 interface (max 120s)
echo "Waiting for lokitun0 interface..."
ELAPSED=0
while [ ! -d "/sys/class/net/lokitun0" ]; do
    sleep 1
    ELAPSED=$((ELAPSED + 1))
    if [ "$ELAPSED" -ge 120 ]; then
        echo "ERROR: lokitun0 did not appear within 120s"
        exit 1
    fi
    if ! kill -0 "$LOKINET_PID" 2>/dev/null; then
        echo "ERROR: lokinet process died during startup"
        exit 1
    fi
done
echo "lokitun0 is up after ${ELAPSED}s"

# Wait for lokinet DNS socket to actually resolve .loki domains (max 180s)
# The TUN interface comes up before the DHT is ready
echo "Waiting for .loki DNS resolution on 127.3.2.1:53..."
DNS_ELAPSED=0
DNS_TIMEOUT=180
while true; do
    if dig +short +timeout=5 +tries=1 @127.3.2.1 exit.loki A 2>/dev/null | grep -q '[0-9]'; then
        echo ".loki DNS resolution working after ${DNS_ELAPSED}s"
        break
    fi
    sleep 3
    DNS_ELAPSED=$((DNS_ELAPSED + 3))
    if [ "$DNS_ELAPSED" -ge "$DNS_TIMEOUT" ]; then
        echo "WARN: .loki DNS not resolving after ${DNS_TIMEOUT}s, starting proxy anyway"
        break
    fi
    if ! kill -0 "$LOKINET_PID" 2>/dev/null; then
        echo "ERROR: lokinet process died while waiting for DNS"
        exit 1
    fi
done

echo "Starting microsocks SOCKS5 proxy on 0.0.0.0:1080..."
exec /usr/local/bin/microsocks -i 0.0.0.0 -p 1080
