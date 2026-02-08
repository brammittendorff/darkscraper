FROM debian:bookworm-slim

ARG DEBIAN_FRONTEND=noninteractive

# Install lokinet from official oxen repo + microsocks (tiny SOCKS5 proxy)
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl lsb-release iproute2 netcat-openbsd \
      gcc libc6-dev git make dnsutils \
    && curl -so /etc/apt/trusted.gpg.d/oxen.gpg https://deb.oxen.io/pub.gpg \
    && echo "deb https://deb.oxen.io $(lsb_release -sc) main" > /etc/apt/sources.list.d/oxen.list \
    && echo "resolvconf resolvconf/linkify-resolvconf boolean false" | debconf-set-selections \
    && apt-get update && apt-get install -y --no-install-recommends lokinet \
    && git clone https://github.com/rofl0r/microsocks.git /tmp/microsocks \
    && cd /tmp/microsocks && make && cp microsocks /usr/local/bin/ \
    && rm -rf /tmp/microsocks \
    && apt-get purge -y gcc libc6-dev git make && apt-get autoremove -y \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Stage config OUTSIDE /var/lib/lokinet (which is a volume mount point).
# The entrypoint copies it into place at runtime so it always wins over
# stale data persisted in the Docker volume from previous builds.
COPY lokinet-docker.ini /etc/lokinet-docker.ini
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 1080

ENTRYPOINT ["/entrypoint.sh"]
