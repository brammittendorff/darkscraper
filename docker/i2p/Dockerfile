FROM geti2p/i2p:latest

# The base image already includes all necessary tools and default configuration
# We use the default I2P configuration which includes all client apps

# Expose ports:
# 4444 - HTTP Proxy (for crawling .i2p sites)
# 4445 - HTTPS Proxy
# 7657 - Router Console (web UI)
# 7654 - I2CP (I2P Client Protocol)
# 7656 - SAM Bridge
EXPOSE 4444 4445 7657 7654 7656

# The entrypoint is already configured in the base image
# It will start the I2P router automatically with all client apps
