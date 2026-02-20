#!/usr/bin/env bash
# QEMU ESP32-S3 boot verification test.
#
# Builds the firmware, launches QEMU, and checks that the system
# prints "FSM starting" within a timeout (indicating successful boot
# through ESP-IDF init, FreeRTOS start, and application entry).
#
# Exit codes:
#   0 — boot verified
#   1 — timeout or build failure

set -euo pipefail

TIMEOUT_SECS=${1:-15}
BUILD_TARGET="xtensa-esp32s3-espidf"
FIRMWARE="target/${BUILD_TARGET}/release/petfilter"
MARKER="FSM starting"

cd "$(dirname "$0")/../.."

echo "=== Building firmware ==="
cargo +esp build --release --target "${BUILD_TARGET}" -Z build-std=std,panic_abort

if [ ! -f "${FIRMWARE}" ]; then
    echo "ERROR: firmware binary not found at ${FIRMWARE}"
    exit 1
fi

echo "=== Launching QEMU (timeout: ${TIMEOUT_SECS}s) ==="
# Create a named pipe for QEMU serial output
PIPE=$(mktemp -u)
mkfifo "${PIPE}"

# Start QEMU in background, serial output to pipe
qemu-system-xtensa \
    -nographic \
    -machine esp32s3 \
    -drive "file=${FIRMWARE},if=mtd,format=raw" \
    -serial "pipe:${PIPE}" \
    -no-reboot &
QEMU_PID=$!

# Monitor the serial output for the boot marker
FOUND=false
if timeout "${TIMEOUT_SECS}" grep -q -m1 "${MARKER}" "${PIPE}"; then
    FOUND=true
fi

# Clean up
kill "${QEMU_PID}" 2>/dev/null || true
wait "${QEMU_PID}" 2>/dev/null || true
rm -f "${PIPE}" "${PIPE}.in" "${PIPE}.out"

if [ "${FOUND}" = true ]; then
    echo "=== PASS: Boot verified ('${MARKER}' found) ==="
    exit 0
else
    echo "=== FAIL: Boot marker '${MARKER}' not found within ${TIMEOUT_SECS}s ==="
    exit 1
fi
