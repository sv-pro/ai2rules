# Egress allowlist proxy for the governed Claude Code stack (PLAN.md E13 / E8).
# A minimal tinyproxy built from a base image (no third-party proxy image) that
# tunnels HTTPS (CONNECT :443) only to hosts matching /etc/tinyproxy/filter.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        tinyproxy \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY tinyproxy.conf /etc/tinyproxy/tinyproxy.conf
COPY filter /etc/tinyproxy/filter

EXPOSE 8888
# Foreground (-d); tinyproxy drops to the tinyproxy user per the config.
CMD ["tinyproxy", "-d", "-c", "/etc/tinyproxy/tinyproxy.conf"]
