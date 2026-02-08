FROM python:3.9-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    libssl-dev \
    pkg-config \
    build-essential \
    libffi-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Clone ZeroNet from maintained conservancy fork (original is abandoned)
RUN git clone https://github.com/zeronet-conservancy/zeronet-conservancy.git . \
    && git checkout master

# Install ALL Python dependencies manually
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
    setuptools

# Create data directory
RUN mkdir -p /app/data

# Expose ports
EXPOSE 43110 26552/udp

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
