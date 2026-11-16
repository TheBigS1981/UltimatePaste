#!/usr/bin/env bash
# Renders screenshots for the release notes of a version (dummy content only, English):
#   docs/images/release/<version>/*.png
set -euo pipefail
cd "$(dirname "$0")/.."
V="${1:?usage: release-shots.sh <version>}"
mkdir -p "docs/images/release/$V"
docker compose --profile tools run --rm -T -e V="$V" shot bash -c '
  set -e
  npx vite --host 127.0.0.1 --port 1420 --strictPort >/tmp/vite.log 2>&1 &
  for i in $(seq 1 50); do curl -sf http://127.0.0.1:1420/dev-preview.html >/dev/null && break; sleep 0.3; done
  shot() { # name w h query
    chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
      --window-size=$2,$3 --virtual-time-budget=5000 --screenshot=/work/docs/images/release/$V/$1.png \
      "http://127.0.0.1:1420/dev-preview.html?$4" >/dev/null 2>&1 || echo "failed: $1"
  }
  shot snippet-choice 760 300 "window=fill&layout=fluent&theme=light&lang=en"
  shot snippet-choice-dark 760 300 "window=fill&layout=fluent&theme=dark&lang=en"
  shot snippet-input 760 260 "window=fill&fill=multi&layout=fluent&theme=dark&lang=en"
  shot unsaved-changes 860 520 "layout=fluent&theme=dark&tab=snippets&discard=1&lang=en"
  shot window-position 820 470 "window=settings&layout=fluent&theme=light&section=picker&lang=en"
'
ls -la "docs/images/release/$V"
