# Governed Claude Code — a containerized "system under test" (SUT) for ai2rules
# (PLAN.md E13 / E8). Refreshed for the Rust hook (D37): the deep-half governance
# is `harness cc-hook` against .claude/cc-world.yaml — NOT the retired Python
# world-gate.py / cc-world.json (now under .claude/hooks/superseded/).
#
# Why this exists: run the agent *under test* (with the harness's PreToolUse
# governance) OUT of your host dev session — so an experiment that restricts tools
# or empties settings.json can't brick the Claude you're driving — AND provide the
# OS-level isolation floor (E8) the in-agent decisions need to be *enforced*
# (network policy, write confinement, non-root, dropped caps; see run.sh / compose).
#
# The workspace (your repo, incl. its .claude/ governance) is MOUNTED at runtime;
# auth comes from $ANTHROPIC_API_KEY at runtime — never baked into the image.
#
#   Build:  docker build -t governed-claude -f docker/Dockerfile .
#   Run:    ./docker/run.sh                              # offline shell
#           MODE=replace NET=bridge ./docker/run.sh claude   # replace-mode experiment

# --- build stage: the Rust kernel binary (harness) -------------------------------
FROM rust:1-bookworm AS build
WORKDIR /build
# Only what the Rust workspace needs (context trimmed by the repo-root .dockerignore,
# honored by both the classic and BuildKit builders) — keeps repos/, target/,
# blog/node_modules out of the build.
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
RUN cargo build --release --locked -p cli-harness

# --- runtime stage: node + Claude Code + the harness binary ----------------------
FROM node:22-bookworm-slim

# Runtime deps: git + ripgrep + jq back Claude Code and the hook tooling; python3
# runs the optional SubagentStop taint-notify hook (the PreToolUse gate is Rust).
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 git ripgrep jq ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The Claude Code CLI.
RUN npm install -g @anthropic-ai/claude-code && npm cache clean --force

# The governance kernel (D37): `harness cc-hook` is the PreToolUse decider.
COPY --from=build /build/target/release/harness /usr/local/bin/harness

# Non-root: reuse the base image's `node` user (uid 1000) — its uid usually matches
# the host user, avoiding permission friction on the mounted workspace + taint state.
USER node
WORKDIR /workspace

# Pin the project dir so the hook resolves .claude/cc-world.yaml + .claude/state
# however Claude Code invokes it; HARNESS_BIN makes world-gate.sh pick the image's
# kernel binary deterministically (not a stale mounted target/ build).
ENV CLAUDE_PROJECT_DIR=/workspace \
    HARNESS_BIN=/usr/local/bin/harness

CMD ["bash"]
