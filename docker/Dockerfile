# Governed Claude Code — a containerized "system under test" (SUT) for the
# CLI Agent Harness (PLAN.md E13 / E8).
#
# Why this exists: it separates the agent *under test* (running the harness's
# PreToolUse governance hooks) from your host dev session, AND provides the
# OS-level isolation floor (E8) that the in-agent hooks' decisions need to be
# actually enforced — network policy, write confinement, non-root, dropped caps —
# chosen at `docker run` time (see docker/run.sh).
#
# Nothing project-specific is baked in: the workspace (your repo, including its
# .claude/ governance) is MOUNTED at runtime, and auth comes from
# $ANTHROPIC_API_KEY at runtime — never the image.
#
#   Build:  docker build -t governed-claude -f docker/Dockerfile .
#   Run:    ./docker/run.sh            (offline shell)   |   see docker/run.sh
FROM node:22-bookworm-slim

# Runtime deps: python3 runs the PreToolUse hooks; git + ripgrep + jq back
# Claude Code and the hook tooling.
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 \
        git \
        ripgrep \
        jq \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The Claude Code CLI.
RUN npm install -g @anthropic-ai/claude-code \
    && npm cache clean --force

# Non-root: the contained agent must never run as root.
RUN useradd --create-home --uid 1000 agent
USER agent
WORKDIR /workspace

# Hooks resolve state under $CLAUDE_PROJECT_DIR/.claude/state; pin it to the
# mounted workspace so it works regardless of how Claude Code invokes the hook.
ENV CLAUDE_PROJECT_DIR=/workspace

# Default to an interactive shell so you can drive `claude` and inspect taint
# state; override with `claude` (see docker/run.sh) for a direct agent session.
CMD ["bash"]
