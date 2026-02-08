FROM python:3.9-slim

# Install system dependencies including netcat for healthchecks
RUN apt-get update && apt-get install -y \
    git \
    libssl-dev \
    pkg-config \
    build-essential \
    libffi-dev \
    netcat-openbsd \
    wget \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Clone ZeroNet from maintained conservancy fork (original is abandoned)
RUN git clone https://github.com/zeronet-conservancy/zeronet-conservancy.git . \
    && git checkout master

# Install ALL Python dependencies
RUN pip install --no-cache-dir \
    gevent==24.2.1 \
    msgpack==1.0.8 \
    PySocks \
    base58 \
    merkletools \
    pyasn1 \
    pyelliptic \
    coincurve \
    bencode.py \
    maxminddb \
    pyaes \
    rsa \
    setuptools \
    rich \
    defusedxml

# Create data directory and set permissions
RUN mkdir -p /app/data /app/log && \
    chmod -R 755 /app/data /app/log

# Pre-download bootstrap data to improve initial peer discovery
RUN mkdir -p /app/data/1HELLo4uzjaLetFx6NH3PMwFP3qbRbTf3D || true

# Expose ports (43110 for UI/HTTP proxy, 26552 for fileserver)
EXPOSE 43110 26552/udp

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Health check to ensure ZeroNet is responding
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD nc -z 127.0.0.1 43110 || exit 1

ENTRYPOINT ["/entrypoint.sh"]
