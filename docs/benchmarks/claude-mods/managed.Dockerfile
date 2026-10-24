FROM node:22-bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN npm install --prefix /opt/probe-tools --no-audit --no-fund \
    @anthropic-ai/claude-code@2.1.288 ai2rules-harness@0.6.0
COPY probe.py managed_probe.py managed_hook.py /opt/probe/
COPY managed-settings.json /etc/claude-code/managed-settings.json
RUN chmod 755 /etc/claude-code /opt/probe \
    && chmod 644 /etc/claude-code/managed-settings.json /opt/probe/*.py \
    && mkdir /evidence && chown node:node /evidence
USER node
WORKDIR /evidence
ENV PYTHONDONTWRITEBYTECODE=1
ENTRYPOINT ["python3", "/opt/probe/managed_probe.py"]
