# Build environment for cross-compiling UltimatePaste (Tauri 2) to Windows x64 from Linux.
FROM rust:1-bookworm

RUN apt-get update \
 && apt-get install -y --no-install-recommends clang lld llvm nsis ca-certificates \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev pkg-config \
 && rm -rf /var/lib/apt/lists/* \
 && ln -sf /usr/lib/llvm-14/bin/llvm-lib /usr/local/bin/llvm-lib \
 && ln -sf /usr/lib/llvm-14/bin/llvm-dlltool /usr/local/bin/llvm-dlltool \
 && ln -sf /usr/lib/llvm-14/bin/clang /usr/local/bin/clang-cl

# Debian's NSIS 3.08 lacks headers the Tauri installer template needs (e.g. Win\RestartManager.nsh):
# overlay the include files from the official NSIS 3.10 release.
RUN apt-get update && apt-get install -y --no-install-recommends unzip curl && rm -rf /var/lib/apt/lists/* \
 && curl -fsSL -o /tmp/nsis.zip "https://downloads.sourceforge.net/project/nsis/NSIS%203/3.10/nsis-3.10.zip" \
 && unzip -q /tmp/nsis.zip -d /tmp/nsis \
 && cp -rn /tmp/nsis/nsis-3.10/Include/. /usr/share/nsis/Include/ \
 && rm -rf /tmp/nsis /tmp/nsis.zip \
 && test -f /usr/share/nsis/Include/Win/RestartManager.nsh

# Node.js for the frontend build (copied from the official image)
COPY --from=node:22-bookworm-slim /usr/local/bin/node /usr/local/bin/node
COPY --from=node:22-bookworm-slim /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx

RUN rustup target add x86_64-pc-windows-msvc \
 && rustup component add clippy rustfmt \
 && cargo install --locked cargo-xwin \
 && cargo install --locked tauri-cli --version "^2" \
 && rm -rf /usr/local/cargo/registry

# Writable cache root for the non-root build user (named volume inherits this ownership)
RUN mkdir -p /cache && chown 1000:1000 /cache
ENV CARGO_HOME=/cache/cargo \
    CARGO_TARGET_DIR=/cache/target \
    XWIN_CACHE_DIR=/cache/xwin \
    npm_config_cache=/cache/npm \
    HOME=/cache/home \
    PATH=/usr/local/cargo/bin:$PATH
