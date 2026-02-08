FROM poullorca/hyphanet-node:v0.15.04

USER root

# Install curl for testing
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Copy our configuration
COPY freenet-defaults.ini /opt/hyphanet/freenet.ini

COPY --chmod=755 entrypoint.sh /entrypoint.sh

USER hyphanet

CMD ["/entrypoint.sh"]
