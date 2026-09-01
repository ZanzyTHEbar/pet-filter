# PetFilter

![Firmware CI](https://github.com/ZanzyTHEbar/pet-filter/actions/workflows/firmware.yml/badge.svg)

**A compact, intelligent air scrubber that uses venturi-based water-air contact to dissolve and destroy pet odor compounds.**

## What Is This?

PetFilter is a hardware product that eliminates pet odors (specifically litter box ammonia, mercaptans, and VOCs) using a novel approach: water scrubbing. No consumer pet product currently exploits the fact that ammonia -- the #1 cat litter odor compound -- is *extremely* water-soluble.

### How It Works

1. A **peristaltic pump** drives water through a **venturi nozzle**
2. The venturi creates negative pressure that **entrains room air** into the water stream
3. Air breaks into fine bubbles (0.5-2mm) with massive surface area
4. Odor molecules **dissolve into the water** on contact (ammonia: 80-95% per pass)
5. **UVC light** breaks down the dissolved odor compounds
6. **Carbon + HEPA filtration** on intake/exhaust catches particulates and non-soluble VOCs

### Target Market

Cat owners, rodent owners, and anyone dealing with litter box odors. Positioned as a premium device ($349-449) that *actually works*, between cheap sprays and $500+ smart litter boxes.

## Repository Structure

```
pet-filter/
├── docs/
│   ├── research/         # Market analysis, odor chemistry, technology validation
│   ├── engineering/      # System architecture, subsystem specs, BOM, power/noise
│   ├── business/         # Business case, unit economics, manufacturing strategy
│   └── branding/         # Brand identity, ICA, go-to-market, pricing
├── firmware/             # ESP32-S3 firmware (Rust via esp-rs)
│   └── src/
│       ├── sensors/      # NH3 (MQ-137), flow, water level, temperature
│       ├── control/      # PID control
│       ├── fsm/          # State machine (Idle→Sensing→Active→Purging→Error)
│       ├── app/          # Core application service (host-testable)
│       ├── adapters/     # WiFi, BLE, NVS, TLS, mDNS, cert store
│       ├── rpc/          # FlatBuffers RPC over TLS 1.3 / BLE (no REST/MQTT)
│       └── drivers/      # Pump, UVC, LED, button, watchdog, timers
├── hardware/
│   ├── pcb/              # KiCad project files
│   ├── cad/              # Housing CAD files
│   └── datasheets/       # Component datasheets
└── tools/                # KiCad symbol/schematic generators (Python)
```

## Technical Specs (Target)

| Parameter | Value |
|-----------|-------|
| Ammonia removal | 85-95% per pass |
| Power consumption | 42-70W |
| Noise level | <45 dB(A) @ 1m |
| Water capacity | 4-6L (lasts 1-2 weeks) |
| Filter life | 2-3 months |
| Footprint | ~14" x 10" x 16" |
| Connectivity | WiFi + BLE (ESP32-S3) |

## Development

### Firmware (Rust)

Requires the `esp` rust toolchain (xtensa) and stable for host tests. [`just`](https://github.com/casey/just) wraps all commands — see `firmware/justfile`:

```bash
cd firmware
just build     # ESP32-S3 cross-build (xtensa-esp32s3-espidf)
just test      # host test suite (x86_64, no-default-features)
just lint      # clippy -D warnings
just ci        # fmt + lint + test + audit
just flash     # espflash to device
```

Work is tracked on GitHub: issues #1–#23 under Gate 1/2/3 milestones (`Gate 1: POC-Ready Firmware`, `Gate 2: Prototype Greenlight`, `Gate 3: Pre-Production`).

### PCB (KiCad 8)

Open `hardware/pcb/petfilter.kicad_pro` in KiCad 8. ERC/DRC/BOM/gerbers via KiBot in Docker — see `hardware/pcb/Makefile` (`make validate`, `make gerbers`).

## License

Proprietary. All rights reserved.
