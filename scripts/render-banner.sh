#!/usr/bin/env bash
# Renders docs/banner/banner.html → docs/banner.png (2560×1280) and docs/social-preview.png (1280×640).
set -euo pipefail
cd "$(dirname "$0")/.."
docker compose --profile tools run --rm -T shot bash -c '
  chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars --allow-file-access-from-files --force-device-scale-factor=2 \
    --window-size=1280,640 --virtual-time-budget=3000 --screenshot=/work/docs/banner.png file:///work/docs/banner/banner.html >/dev/null 2>&1
  chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars --allow-file-access-from-files --force-device-scale-factor=1 \
    --window-size=1280,640 --virtual-time-budget=3000 --screenshot=/work/docs/social-preview.png file:///work/docs/banner/banner.html >/dev/null 2>&1
'
ls -la docs/banner.png docs/social-preview.png
