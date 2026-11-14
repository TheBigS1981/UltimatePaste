# Builder + headless Chromium for UI screenshots of the dev preview harness.
FROM ultimatepaste-builder:latest
USER root
RUN apt-get update && apt-get install -y --no-install-recommends chromium fonts-dejavu-core fonts-noto-color-emoji && rm -rf /var/lib/apt/lists/*
