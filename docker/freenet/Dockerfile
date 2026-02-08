FROM 5nafu/freenet

USER root

# Install curl for wizard completion script
RUN apk add --no-cache curl

# Patch the defaults config to enable opennet and skip the setup wizard.
RUN sed -i '/^End$/d' /defaults/freenet.ini && \
    echo 'node.opennet.enabled=true' >> /defaults/freenet.ini && \
    echo 'security-levels.networkThreatLevel=LOW' >> /defaults/freenet.ini && \
    echo 'End' >> /defaults/freenet.ini

COPY --chmod=755 entrypoint.sh /entrypoint.sh
RUN chown fred:fred /entrypoint.sh

USER fred

CMD ["/entrypoint.sh"]
