FROM node:24-trixie-slim
USER root
# Windows-only builds need LLVM, not Linux GTK/WebKit or a desktop server.
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates curl git openssh-client less ripgrep procps \
        build-essential pkg-config clang lld llvm python3 \
    && rm -rf /var/lib/apt/lists/*
ENV LANG=C.UTF-8 \
    VITE_USE_POLLING=true \
    PATH=/root/.cargo/bin:/root/.local/bin:$PATH \
    CARGO_TARGET_DIR=/root/.cache/klickwerk/target \
    XWIN_CACHE_DIR=/root/.cache/xwin \
    CODEX_HOME=/root/.codex \
    CLAUDE_CONFIG_DIR=/root/.claude
RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain 1.98.1 \
    && rustup target add x86_64-pc-windows-msvc \
    && rustup component add clippy rustfmt \
    && cargo install cargo-xwin --version 0.23.1 --locked \
    && rm /tmp/rustup.sh \
    && rm -rf /root/.cargo/registry /root/.cargo/git
# Keep coding-agent sign-in volumes; install binaries into the image.
RUN mkdir -p /root/.codex /root/.claude \
    && chmod 700 /root/.codex /root/.claude \
    && curl -fsSL https://claude.ai/install.sh -o /tmp/install-claude.sh \
    && bash /tmp/install-claude.sh stable \
    && npm install --global @openai/codex \
    && npm cache clean --force \
    && rm -f /tmp/install-claude.sh \
    && rm -rf /root/.claude/downloads \
    && git config --global init.defaultBranch main \
    && git config --global --add safe.directory /workspaces/klickwerk
WORKDIR /workspaces/klickwerk
CMD ["sleep", "infinity"]
