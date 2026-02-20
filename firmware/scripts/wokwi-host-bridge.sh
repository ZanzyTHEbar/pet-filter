#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -f /home/esp/export-esp.sh ]]; then
  # Running inside devcontainer
  source /home/esp/export-esp.sh
  cargo build --release --target xtensa-esp32s3-espidf
  mkdir -p .wokwi-host
  cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter
  cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter.elf
else
  # Running on host: execute inside devcontainer so target volume is available
  npx -y @devcontainers/cli exec --workspace-folder "$ROOT_DIR" -- bash -lc     'source /home/esp/export-esp.sh && cargo build --release --target xtensa-esp32s3-espidf && mkdir -p .wokwi-host && cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter && cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter.elf'
fi

cat <<'EOF'
Artifacts exported to .wokwi-host/ for host GUI usage.
Use host-side Wokwi extension with:
  ELF: .wokwi-host/petfilter.elf
  Diagram: diagram.json
Forwarded helper port: 9013 (optional host bridge services)
EOF
