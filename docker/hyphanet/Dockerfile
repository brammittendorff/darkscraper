FROM 5nafu/freenet

USER root

# Install curl for wizard completion script
RUN apk add --no-cache curl

# Copy our complete configuration
COPY freenet-defaults.ini /defaults/freenet.ini

COPY --chmod=755 entrypoint.sh /entrypoint.sh
RUN chown fred:fred /entrypoint.sh

USER fred

CMD ["/entrypoint.sh"]
