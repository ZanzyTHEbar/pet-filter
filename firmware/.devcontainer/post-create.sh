#!/usr/bin/env bash
set -euo pipefail

source /home/esp/export-esp.sh || true

if [[ -z "${IDF_PATH:-}" ]]; then
  if [[ -d /home/esp/.espressif/esp-idf ]]; then
    IDF_PATH="$(ls -d /home/esp/.espressif/esp-idf/v* 2>/dev/null | head -n1 || true)"
  fi
fi

IDF_PATH="${IDF_PATH:-/opt/esp/idf}"
python3 "$IDF_PATH/tools/idf_tools.py" install qemu-xtensa riscv32-esp-elf

# Install official Wokwi CLI for deterministic in-container runs
curl -fsSL https://wokwi.com/ci/install.sh | sh

rustc --version
cargo --version
flatc --version
/home/esp/.wokwi/bin/wokwi-cli --version
echo "PetFilter devcontainer ready"
