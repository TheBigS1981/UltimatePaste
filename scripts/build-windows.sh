#!/usr/bin/env bash
# Cross-compiles UltimatePaste for Windows x64 inside the builder container and copies
# the portable exe to ./_out. Pass --installer to also build the NSIS installer.
set -euo pipefail
cd "$(dirname "$0")/.."
bundle=--no-bundle
[[ "${1:-}" == "--installer" ]] && bundle="--bundles nsis"
docker compose run --rm -T -e BUNDLE="$bundle" builder bash -c '
  set -euo pipefail
  npm ci --no-audit --no-fund
  cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc $BUNDLE
  ver=$(node -p "require(\"./src-tauri/tauri.conf.json\").version")
  rel=$CARGO_TARGET_DIR/x86_64-pc-windows-msvc/release
  mkdir -p /work/_out
  cp "$rel/ultimatepaste.exe" "/work/_out/UltimatePaste-$ver-portable.exe"
  if [[ "$BUNDLE" != --no-bundle ]]; then cp "$rel/bundle/nsis/UltimatePaste_${ver}_x64-setup.exe" /work/_out/; fi
  ls -la /work/_out | tail -3
'
