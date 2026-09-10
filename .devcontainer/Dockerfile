FROM node:24-trixie-slim
USER root
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
# Windows-only builds need LLVM, not Linux GTK/WebKit or a desktop server.
RUN set -eu; apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates curl git openssh-client less ripgrep procps ncurses-bin \
        build-essential pkg-config clang lld llvm python3 \
    && rm -rf /var/lib/apt/lists/*
ENV LANG=C.UTF-8 \
    VITE_USE_POLLING=true \
    PATH=/root/.cargo/bin:/root/.local/bin:$PATH \
    CARGO_TARGET_DIR=/root/.cache/klickwerk/target \
    XWIN_CACHE_DIR=/root/.cache/xwin \
    CODEX_HOME=/root/.codex \
    CLAUDE_CONFIG_DIR=/root/.claude
RUN set -eu; curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain 1.98.1 \
    && rustup target add x86_64-pc-windows-msvc \
    && rustup component add clippy rustfmt \
    && cargo install cargo-xwin --version 0.23.1 --locked \
    && rm /tmp/rustup.sh \
    && rm -rf /root/.cargo/registry /root/.cargo/git
# Installers detect amd64/arm64. Keep binaries and symlink targets outside data volumes.
# Codex uses /opt/codex only during installation; runtime data stays in CODEX_HOME.
RUN set -eu; \
    install -d -m 700 -o root -g root \
        /root/.claude /root/.codex /root/.agents /root/.config/anthropic; \
    curl -fsSL https://claude.ai/install.sh | bash -o pipefail -s -- stable; \
    claude --version; \
    curl -fsSL https://chatgpt.com/codex/install.sh \
        | CODEX_HOME=/opt/codex CODEX_INSTALL_DIR=/usr/local/bin CODEX_NON_INTERACTIVE=1 bash -o pipefail; \
    chown -R root:root /opt/codex; \
    codex --version; \
    curl -fsSL https://codehamr.com/install.sh | TERM=xterm PREFIX=/usr/local bash -o pipefail; \
    codehamr --version; \
    rm -rf /root/.claude/downloads; \
    git config --global init.defaultBranch main; \
    git config --global --add safe.directory /workspaces/klickwerk
WORKDIR /workspaces/klickwerk
CMD ["sleep", "infinity"]
