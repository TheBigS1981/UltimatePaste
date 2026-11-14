#!/usr/bin/env bash
# Renders the UI with mocked data in headless Chromium → _shots/*.png
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p _shots
docker compose --profile tools run --rm -T shot bash -c '
  set -e
  npx vite --host 127.0.0.1 --port 1420 --strictPort >/tmp/vite.log 2>&1 &
  for i in $(seq 1 50); do curl -sf http://127.0.0.1:1420/dev-preview.html >/dev/null && break; sleep 0.3; done
  shot() { # name w h query
    chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
      --window-size=$2,$3 --virtual-time-budget=4000 --screenshot=/work/_shots/$1.png \
      "http://127.0.0.1:1420/dev-preview.html?$4" >/dev/null 2>&1 || echo "failed: $1"
  }
  for th in light dark; do
    shot fluent-$th 860 520 "layout=fluent&theme=$th&sel=1"
    shot board-$th 980 660 "layout=board&theme=$th&sel=2"
    shot drawer-$th 452 800 "layout=drawer&theme=$th&sel=0"
    shot compact-$th 680 430 "layout=compact&theme=$th&sel=0"
    shot settings-$th 820 680 "window=settings&layout=fluent&theme=$th"
  done
  shot picker-light 452 500 "window=picker&layout=compact&theme=light"
  shot picker-gif-dark 452 500 "window=picker&layout=fluent&theme=dark&ptab=gif"
  shot picker-dark 452 500 "window=picker&layout=fluent&theme=dark&tone=3"
  shot translate-light 860 480 "window=translate&layout=fluent&theme=light"
  shot translate-ai-dark 860 480 "window=translate&layout=board&theme=dark&engine=ai"
  shot correct-light 860 480 "window=translate&layout=fluent&theme=light&tab=correct"
  shot correct-dark 860 480 "window=translate&layout=board&theme=dark&tab=correct"
  shot set-paste 820 1300 "window=settings&layout=fluent&theme=light&section=paste"
  shot set-picker 820 760 "window=settings&layout=fluent&theme=light&section=picker"
  shot set-translate 820 1300 "window=settings&layout=fluent&theme=light&section=translate"
  shot set-stats 820 1250 "window=settings&layout=fluent&theme=light&section=stats"
  shot notice-light 500 260 "window=notice&layout=fluent&theme=light"
  shot set-translate-vi 820 900 "window=settings&layout=fluent&theme=light&section=translate&lang=vi"
  shot correct-fr 860 480 "window=translate&layout=fluent&theme=dark&tab=correct&lang=fr"
  shot tour-1 760 600 "window=tour&layout=fluent&theme=light"
  shot toast-light 420 72 "window=toast&theme=light"
  shot snippets-fluent-dark 860 520 "layout=fluent&theme=dark&tab=snippets"
  shot snippets-board-light 980 660 "layout=board&theme=light&tab=snippets"
  shot files-fluent-light 860 520 "layout=fluent&theme=light&sel=6"
  shot mix-fluent-ocean-dark 860 520 "layout=fluent&design=ocean&theme=dark&sel=1"
  shot mix-board-graphite-light 980 660 "layout=board&design=graphite&theme=light&sel=1"
  shot mix-drawer-rose-dark 452 800 "layout=drawer&design=rose&theme=dark&sel=0"
  shot mix-compact-amber-light 680 430 "layout=compact&design=amber&theme=light&sel=2"
  ls /work/_shots
'
