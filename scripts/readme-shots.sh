#!/usr/bin/env bash
# Renders the README screenshots (dummy content only) for English and German:
#   docs/images/*.png  and  docs/images/de/*.png
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p docs/images/de
docker compose --profile tools run --rm -T shot bash -c '
  set -e
  npx vite --host 127.0.0.1 --port 1420 --strictPort >/tmp/vite.log 2>&1 &
  for i in $(seq 1 50); do curl -sf http://127.0.0.1:1420/dev-preview.html >/dev/null && break; sleep 0.3; done
  shot() { # dir name w h query
    chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
      --window-size=$3,$4 --virtual-time-budget=5000 --screenshot=/work/docs/images/$1$2.png \
      "http://127.0.0.1:1420/dev-preview.html?$5" >/dev/null 2>&1 || echo "failed: $2"
  }
  for L in en de; do
    D=""; [ "$L" = de ] && D="de/"
    shot "$D" palette-fluent 860 520 "layout=fluent&theme=dark&sel=0&lang=$L"
    shot "$D" palette-board 980 640 "layout=board&theme=light&sel=1&lang=$L"
    shot "$D" palette-compact 680 430 "layout=compact&theme=light&sel=0&lang=$L"
    shot "$D" palette-drawer 452 760 "layout=drawer&theme=dark&sel=2&lang=$L"
    shot "$D" quick-actions 860 520 "layout=fluent&theme=light&sel=5&lang=$L"
    shot "$D" snippets 860 520 "layout=fluent&theme=dark&tab=snippets&lang=$L"
    shot "$D" picker-emoji 452 500 "window=picker&layout=fluent&theme=dark&tone=3&lang=$L"
    shot "$D" picker-gif 452 500 "window=picker&layout=fluent&theme=light&ptab=gif&lang=$L"
    shot "$D" translate 860 480 "window=translate&layout=fluent&theme=light&lang=$L"
    shot "$D" correct 860 480 "window=translate&layout=fluent&theme=dark&tab=correct&lang=$L"
    shot "$D" stats 820 1100 "window=settings&layout=fluent&theme=light&section=stats&lang=$L"
    shot "$D" settings 820 680 "window=settings&layout=fluent&theme=dark&section=paste&lang=$L"
    shot "$D" tour 760 600 "window=tour&layout=fluent&theme=light&lang=$L"
  done
'
ls docs/images docs/images/de | head -40
