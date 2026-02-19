# System Architecture: PetFilter

## 1. Overview

PetFilter is a closed-loop water scrubbing system with four interacting subsystems: water path, air path, UVC treatment, and electronic control. This document describes how they integrate into a complete product.

---

## 2. Physical System Architecture

### 2.1 Water Path (Closed Loop)

```
Tank A (Clean Water, 2-3L)
  │
  ▼
Peristaltic Pump (1 L/min, 12V DC stepper)
  │
  ▼
Venturi Nozzle (3mm throat, entrains air)
  │
  ▼
Air-Water Contact Zone (10-15cm column, bubble contact)
  │
  ▼
UVC Exposure Chamber (3s residence, 75-105 mJ/cm² dose)
  │
  ▼
Tank B (Collection, 2-3L)
  │
  ▼ (Overflow / gravity return)
Tank A (recirculation)
```

**Water volume**: 4-6L total across both tanks
**Recirculation rate**: 1 L/min = full volume cycles every 4-6 minutes
**Water change interval**: 7-14 days

### 2.2 Air Path (Open Circuit)

```
Room Air (contaminated, near litter box)
  │
  ▼
Intake Port (housing exterior)
  │
  ▼
HEPA + Carbon Pre-Filter (particulates + some VOCs)
  │
  ▼
Venturi Air Inlet (2mm port, drawn by vacuum)
  │
  ▼
Air-Water Contact Zone (gas transfer to water)
  │
  ▼
Mist Eliminator (catch water droplets)
  │
  ▼
Carbon Post-Filter (polishing, residual VOCs)
  │
  ▼
Exhaust Port (clean air, housing exterior)
```

**Air flow rate**: 0.1-0.3 L/min (driven by venturi vacuum, no fan)
**Treatment per pass**: 80-95% ammonia, 50-70% mercaptans

### 2.3 UVC Treatment Path

```
Water from contact zone
  │
  ▼
UVC Exposure Chamber
├── 5× UVC LEDs (275nm, 1W each)
├── Aluminum heatsink (water-cooled)
├── Sealed chamber (opaque, gasketed)
└── Interlock switch (lid sensor)
  │
  ▼
Treated water → Tank B
```

---

## 3. Electronics Architecture

### 3.1 Block Diagram

```
                    ┌─────────────────────────┐
   12V DC ──────────┤ Power Management        │
   (External PSU)   │ ├─ Fuse (5A)            │
                    │ ├─ TVS diode            │
                    │ ├─ 12V → 5V buck        │
                    │ └─ 12V → 3.3V buck      │
                    └──────┬──────────────────┘
                           │
              ┌────────────┼────────────────┐
              │            │                │
         ┌────┴────┐  ┌───┴───┐      ┌─────┴─────┐
         │  12V    │  │  5V   │      │   3.3V    │
         │ Rail    │  │ Rail  │      │   Rail    │
         └────┬────┘  └───┬───┘      └─────┬─────┘
              │            │                │
    ┌─────────┴──┐   ┌────┴────┐    ┌──────┴──────┐
    │ Pump Motor │   │ UVC LED │    │  ESP32-S3   │
    │ Driver     │   │ Driver  │    │  MCU        │
    │ (H-bridge) │   │ (CC)    │    │             │
    └─────┬──────┘   └────┬────┘    │ GPIO/ADC:   │
          │                │         │ ├─ MQ-137   │
     ┌────┴────┐    ┌─────┴───┐    │ ├─ Flow     │
     │Peristal.│    │ UVC LED │    │ ├─ WtrLvl   │
     │ Pump    │    │ Array   │    │ ├─ Temp     │
     └─────────┘    └─────────┘    │ ├─ Interlock│
                                    │ ├─ WiFi    │
                                    │ └─ BLE     │
                                    └─────────────┘
```

### 3.2 ESP32-S3 Pin Allocation (Preliminary)

| Function | Pin Type | ESP32 Pin | Notes |
|----------|----------|-----------|-------|
| Pump PWM | PWM Output | GPIO1 | LEDC channel 0 |
| Pump Direction | Digital Out | GPIO2 | For bidirectional |
| UVC Enable | Digital Out | GPIO3 | Through relay |
| UVC PWM (dim) | PWM Output | GPIO4 | Optional dimming |
| NH3 Sensor (MQ-137) | ADC Input | GPIO5 (ADC1_CH4) | 0-3.3V analog |
| Flow Sensor | Pulse Input | GPIO6 | Interrupt-driven |
| Water Level 1 | Digital In | GPIO7 | Tank A level |
| Water Level 2 | Digital In | GPIO8 | Tank B level |
| Temperature (NTC) | ADC Input | GPIO9 (ADC1_CH8) | Voltage divider |
| Interlock Switch | Digital In | GPIO10 | UVC safety |
| Status LED (R) | PWM Output | GPIO11 | RGB status |
| Status LED (G) | PWM Output | GPIO12 | RGB status |
| Status LED (B) | PWM Output | GPIO13 | RGB status |
| I2C SDA | I2C | GPIO14 | Future sensors |
| I2C SCL | I2C | GPIO15 | Future sensors |
| UART TX | UART | GPIO17 | Debug |
| UART RX | UART | GPIO18 | Debug |

---

## 4. Firmware Architecture

### 4.1 Task Model (FreeRTOS or Embassy async)

```
Main Task
├── Sensor Reading Task (10Hz)
│   ├── Read NH3 sensor (ADC)
│   ├── Read flow sensor (pulse count)
│   ├── Read water levels (GPIO)
│   └── Read temperature (ADC)
│
├── Control Task (1Hz)
│   ├── State machine evaluation
│   ├── Pump speed control (PID or threshold)
│   ├── UVC enable/disable
│   └── Safety checks
│
├── Communication Task (0.1Hz)
│   ├── WiFi telemetry (MQTT)
│   ├── BLE status broadcast
│   └── OTA check
│
└── Safety Watchdog (independent)
    ├── Hardware watchdog (ESP32 built-in)
    ├── Stack overflow detection
    └── Thermal shutdown
```

### 4.2 State Machine

```
         ┌──────────────────────────────────────┐
         │                                      │
         ▼                                      │
    ┌─────────┐    NH3 > 10ppm     ┌──────────┐ │
    │  IDLE   │ ──────────────────▶│ SENSING  │ │
    └─────────┘                    └────┬─────┘ │
         ▲                              │       │
         │                    Confirmed │       │
         │                    (30s avg) │       │
         │                              ▼       │
         │                        ┌──────────┐  │
         │     NH3 < 5ppm         │  ACTIVE  │  │
         │     (sustained 5min)   └────┬─────┘  │
         │                              │       │
         │                              ▼       │
         │                        ┌──────────┐  │
         └────────────────────────│ PURGING  │──┘
              (pump flush 2min)   └──────────┘

    Any state ──[safety fault]──▶ ┌─────────┐
                                  │  ERROR  │
                                  └────┬────┘
                                       │
                              [fault cleared]
                                       │
                                       ▼
                                  ┌─────────┐
                                  │  IDLE   │
                                  └─────────┘
```

### 4.3 Safety Interlocks (Hardware + Software)

| Check | Type | Action on Failure |
|-------|------|-------------------|
| Water level low | SW + HW | Stop pump, disable UVC, alert user |
| Flow sensor no pulses | SW | Stop pump (dry run protection) |
| UVC interlock open | HW (relay) | De-energize UVC LEDs immediately |
| Temperature >80°C | SW + HW | Disable UVC, reduce pump speed |
| Watchdog timeout | HW | Full system reset |

---

## 5. Physical Layout

### 5.1 Housing Dimensions
- **Footprint**: 350mm × 250mm (14" × 10")
- **Height**: 400mm (16")
- **Volume**: ~35L internal

### 5.2 Internal Layout (Top View)

```
┌───────────────────────────────────┐
│  ┌───────────┐  ┌───────────┐    │
│  │           │  │           │    │
│  │  Tank A   │  │  Tank B   │    │
│  │ (Clean)   │  │ (Collect) │    │
│  │  2-3L     │  │  2-3L     │    │
│  │           │  │           │    │
│  └─────┬─────┘  └─────▲─────┘    │
│        │              │          │
│  ┌─────▼─────┐  ┌─────┴─────┐    │
│  │   Pump    │  │    UVC    │    │
│  │ Compart.  │  │  Chamber  │    │
│  └─────┬─────┘  └─────▲─────┘    │
│        │              │          │
│        └──────┬───────┘          │
│         ┌─────▼─────┐            │
│         │  Venturi  │            │
│         │ + Contact │            │
│         │   Zone    │            │
│         └───────────┘            │
│                                  │
│  ┌──────────┐  ┌──────────────┐  │
│  │ Intake   │  │  Exhaust     │  │
│  │ Filter   │  │  Filter      │  │
│  └──────────┘  └──────────────┘  │
│                                  │
│  ┌──────────────────────────────┐│
│  │         PCB / Electronics    ││
│  └──────────────────────────────┘│
└───────────────────────────────────┘
```

### 5.3 External Features

- **Top**: Water fill port (flip-top lid), status LED ring
- **Front**: Intake filter access door
- **Back**: Exhaust filter access, power input, USB-C (debug/update)
- **Bottom**: Rubber feet, water drain plug, product label
- **Sides**: Ventilation slots (baffled for noise)

---

## 6. Data Flow

### 6.1 Sensor → Decision → Actuator

```
NH3 Sensor (10Hz) ──▶ Running Average (30s) ──▶ Threshold Compare
                                                      │
                                        ┌──────────────┼──────────────┐
                                        ▼              ▼              ▼
                                   > 10 ppm      5-10 ppm       < 5 ppm
                                   START          MAINTAIN        STOP
                                   SCRUBBING      MONITORING      (after delay)
                                        │              │
                                        ▼              │
                                   Set pump RPM   Keep sensing
                                   Enable UVC
```

### 6.2 Telemetry Data Points

| Data Point | Frequency | Purpose |
|-----------|-----------|---------|
| NH3 level (ppm) | 1/min | Air quality trend |
| Water temperature | 1/min | Thermal monitoring |
| Pump RPM | On change | Operating status |
| UVC on/off | On change | Treatment status |
| Water level | 1/hour | Refill reminder |
| Filter age (hours) | 1/day | Replacement reminder |
| Total operating hours | 1/day | Maintenance tracking |

---

## 7. Bill of Materials (System Level)

See dedicated BOM document (`bom-estimate.md`) for detailed component listing with sourcing.

Summary:
- **Electronics**: $25-45
- **Pump**: $15-25
- **UVC**: $40-60
- **Filters**: $6-10
- **Housing**: $15-30
- **Water tanks**: $8-15
- **Venturi**: $3-8
- **Tubing/connectors**: $5-10
- **Power supply**: $5-8
- **Total BOM**: $120-200 (at 500+ units)
