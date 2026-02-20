#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

: "${WOKWI_CLI_TOKEN:?Set WOKWI_CLI_TOKEN from https://wokwi.com/dashboard/ci}"

mkdir -p artifacts/wokwi .wokwi-host

# Build firmware deterministically for Wokwi simulation
cargo build --release --target xtensa-esp32s3-espidf

# Export artifacts onto bind-mounted workspace for host GUI tooling
cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter
cp target/xtensa-esp32s3-espidf/release/petfilter .wokwi-host/petfilter.elf

WOKWI_CMD=(
  wokwi-cli
  --elf .wokwi-host/petfilter.elf
  --diagram-file diagram.json
  --serial-log-file artifacts/wokwi/serial.log
  --timeout "${WOKWI_TIMEOUT_MS:-120000}"
)

if [[ -n "${WOKWI_EXPECT_TEXT:-}" ]]; then
  WOKWI_CMD+=(--expect-text "$WOKWI_EXPECT_TEXT")
fi

"${WOKWI_CMD[@]}"

printf 'Wokwi serial log: %s\n' "artifacts/wokwi/serial.log"
