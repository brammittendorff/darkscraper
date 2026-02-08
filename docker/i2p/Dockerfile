FROM alpine:latest

# Install i2pd and certificates
RUN apk add --no-cache \
    i2pd \
    ca-certificates \
    curl \
    openssl

# Create data directory
RUN mkdir -p /var/lib/i2pd && chown i2pd:i2pd /var/lib/i2pd

# Expose ports
EXPOSE 4444 4445 7070 7656

USER i2pd

CMD ["i2pd", "--conf=/var/lib/i2pd/i2pd.conf"]
