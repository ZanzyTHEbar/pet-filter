# Legacy Project Analysis: Purbright Controller (Smart-Air-Filter-Project)

> Deep decomposition of the original 2022 Purbright project at
> [github.com/ZanzyTHEbar/Smart-Air-Filter-Project](https://github.com/ZanzyTHEbar/Smart-Air-Filter-Project)
> to identify reusable components, design patterns, and lessons learned for PetFilter.

---

## 1. Project Overview

The Purbright Controller was an automated cat litter air purification system built across **two hardware platforms**:

1. **Seeeduino Nano** (ATmega168, 8-bit AVR) -- the initial prototype
2. **ESP32-S3** (Xtensa dual-core, WiFi/BLE) -- the evolved version

The ESP32-S3 version had significantly more features: WiFi web server, MQTT Home Assistant integration, persistent JSON configuration, NTP time sync, mDNS discovery, NeoPixel LED feedback, and a web-based control interface served from SPIFFS.

### File Count
- ESP32-S3 firmware: ~30 source files (C++ library structure)
- Seeeduino firmware: ~3 source files (monolithic)
- KiCad PCB: SparkFun ESP32 Thing Eagle import (~50 files including footprints)
- Web interface: 5 files (HTML/CSS/JSON)
- Documentation: Minimal (README + functional description docx)

---

## 2. Architecture Decomposition

### 2.1 ESP32-S3 Firmware Architecture

```
PurBrightLib/
├── data/
│   ├── accumulatedata   -- Sensor data aggregation (incomplete)
│   ├── config           -- JSON config on SPIFFS (hostname, MQTT, WiFi, relay states)
│   └── timedtasks       -- Periodic tasks (1s, 5s, 10s, 30s, 1m, 5m intervals)
├── io/
│   ├── Buttons          -- 4× touch buttons with edge detection
│   ├── Neopixel         -- 12-pixel WS2812B status LEDs
│   ├── Pump             -- Relay-controlled pump (manual/automatic/scheduled modes)
│   ├── Relays           -- Generic relay abstraction (up to 5 relays)
│   └── speaker          -- Buzzer RTTTL player (stub, not implemented)
├── sensors/
│   └── PIR              -- Motion sensor (simple digital read)
└── network/
    ├── network          -- WiFi STA + AP fallback + AsyncWebServer
    └── ntp              -- NTP time sync with timezone support
extras/mqtt/
├── Basic/               -- Basic MQTT (broken, wrong file content)
├── HASSIO/              -- Home Assistant auto-discovery (partial)
└── mDNS_Discovery/      -- MQTT broker discovery via mDNS
```

### 2.2 State Machine (Both Platforms)

```
States:
  S_OnOff   -- Power on/off
  S_ManAut  -- Manual (false) / Automatic (true)
  S_Menu    -- Settings menu active
  S_Error   -- Error state

Modes:
  Manual:    Pump runs continuously on button press
  Automatic: Pump triggered by PIR motion sensor with configurable timeout
  Settings:  NeoPixel shows pump duration (3-12 steps × 2 minutes each)
```

### 2.3 Pin Mapping (ESP32-S3 Version)

| Function | GPIO | Type | Notes |
|----------|------|------|-------|
| Touch OnOff | 32 | Input | Power toggle |
| Touch ManAut | 35 | Input | Mode switch |
| Touch Plus | 33 | Input | Settings +  |
| Touch Minus | 27 | Input | Settings - |
| NeoPixel Data | 12 | Output | 12-pixel WS2812B strip |
| Pump Relay | 14 | Output | Configurable via platformio.ini |
| PIR Motion | 21 | Input | Configurable via platformio.ini |
| Red LED | 37 | Output | Status indicator |
| Green LED | 32 | Output | Status indicator |
| Buzzer | 10 | Output | RTTTL (not implemented) |

### 2.4 Seeeduino Nano Pin Mapping

| Function | Pin | Notes |
|----------|-----|-------|
| Touch OnOff | D2 | Power button |
| Touch ManAut | D3 | Mode button |
| Touch Plus | D4 | Settings + |
| Touch Minus | D5 | Settings - |
| Flow Sensor | D6 | **Defined but unused** |
| Motion Sensor | D7 | PIR trigger |
| Pump Relay | D9 | Output |
| Buzzer | D10 | RTTTL |
| NeoPixel | D12 | 12 pixels |

---

## 3. PCB Analysis

### 3.1 Board Design
- **Base design**: SparkFun ESP32 Thing (Eagle import to KiCad)
- **ESP32 variant**: ESP3212 raw chip (QFN48-0.4mm) -- NOT a module
- **Board size**: ~25.4mm × 58.9mm (1.0" × 2.3")
- **Layers**: 2-layer PCB

### 3.2 Power Management (Well-Designed)
The power section is the most polished part of the PCB:
- **Battery charger**: MCP73831 single-cell LiPo (500mA, SOT23-5)
- **Voltage regulator**: AP2112K-3.3V LDO (600mA, SOT23-5)
- **Power path**: P-channel MOSFET (DMG2307L) for USB/battery switching
- **Protection**: PTC fuse (500mA), Schottky diode (BAT20J)
- **Decoupling**: Multiple ESP32 power domains properly decoupled

### 3.3 Connectivity
- FT231XS USB-to-Serial (SSOP20)
- Micro-USB connector
- JST-2 LiPo battery connector
- 2× 20-pin headers (all GPIO exposed)
- Trace antenna (2.4GHz, 25.7mm tuned)
- 26MHz + 32.768kHz crystals

### 3.4 Component Footprints
Mostly 0603 passives, SOT23 ICs, SOIC-8 flash. Full footprint library included in `.pretty` directories.

---

## 4. Web Interface Analysis

### 4.1 Main Dashboard (index.html)
- GPIO ON/OFF control buttons
- Timer form (configurable off-delay in seconds, default 3600)
- Links to WiFi manager and schedule pages
- Server-side template variables (`%MDNSNAME%`, `%STATE%`)
- No JavaScript -- pure HTML with template placeholders

### 4.2 WiFi Manager (wifimanager.html)
- SSID/password input
- DHCP/static IP toggle (JavaScript enables/disables IP fields)
- mDNS hostname configuration with live URL preview
- OTA update section (placeholder, not implemented)

### 4.3 Schedule Page (schedule.html)
- **Empty** -- not implemented

### 4.4 Configuration (config.json)
Key fields: hostname, MQTT settings (broker, port, credentials, topics, secure flag), WiFi credentials, 5-element relay state array, timestamps for connection tracking.

### 4.5 Styling (style.css)
Dark theme: navy background (#0A1128), teal accents (#1282A2), card-based layout with shadows, responsive CSS grid.

---

## 5. Quality Assessment

### 5.1 What Was Done Well

| Aspect | Details |
|--------|---------|
| **Modular library structure** | `PurBrightLib/` with clear subsystem separation (io, sensors, network, data) |
| **Persistent configuration** | JSON on SPIFFS with load/save/reset/dirty-flag pattern |
| **AP fallback** | Falls back to AP mode when WiFi credentials missing or connection fails |
| **Multi-mode operation** | Manual, automatic, and scheduled pump control |
| **Visual feedback** | 12-pixel NeoPixel with color-coded states (yellow=manual, blue=auto, green=pump) |
| **Home Assistant integration** | MQTT auto-discovery for HA (partial but well-structured) |
| **mDNS discovery** | Both for device itself and MQTT broker discovery |
| **PCB power management** | Battery charging, LDO regulation, protection -- professional quality |
| **Edge detection buttons** | Proper positive-negative edge detection for touch inputs |
| **Periodic task scheduling** | Timer-based tasks at 1s, 5s, 10s, 30s, 1m, 5m intervals |

### 5.2 Critical Issues

| Issue | Impact | Root Cause |
|-------|--------|-----------|
| **Global mutable state** | Race conditions, hard to reason about | C++ globals everywhere (`S_OnOff`, `S_ManAut`, etc.) |
| **Manual memory management** | Potential leaks, dangling pointers | `heapStr` in config, `StringtoChar()` malloc without free |
| **Blocking delays** | Missed events, poor responsiveness | `my_delay()` busy-wait, `delay()` in main loop |
| **No error recovery** | System hangs on failure | No try/catch, no watchdog handling, no fallback |
| **Incomplete features** | Dead code, confusion | Speaker (stub), AccumulateData (broken), BasicMQTT (wrong file content) |
| **Hardcoded credentials** | Security risk | WiFi credentials in platformio.ini, MQTT "admin" in config |
| **No authentication** | Anyone on network can control device | Web server has zero auth, CORS wide open |
| **Legacy PCB format** | Can't use directly in KiCad 8 | Eagle import, legacy `.lib` format |

### 5.3 Seeeduino vs. ESP32-S3 Delta

| Feature | Seeeduino Nano | ESP32-S3 |
|---------|---------------|----------|
| CPU | ATmega168 (8-bit, 16MHz) | Xtensa (32-bit, 240MHz) |
| WiFi | No | Yes |
| MQTT | No | Yes (HA integration) |
| Web server | No | Yes (AsyncWebServer) |
| NTP | No | Yes |
| Config storage | None | SPIFFS JSON |
| State machine | Same | Same (enhanced) |
| Pump control | Same logic | Same + scheduling |
| LED feedback | Same (NeoPixel 12px) | Same |
| Buttons | Same (4× touch) | Same |
| Motion sensor | Same (PIR) | Same |

---

## 6. What to Reuse in PetFilter

### 6.1 HIGH VALUE -- Directly Applicable

| Component | From | Adapt How |
|-----------|------|-----------|
| **State machine pattern** | Both platforms | Already implemented in Rust (`state_machine.rs`). Purbright's IDLE→MANUAL/AUTO→ACTIVE pattern maps to PetFilter's IDLE→SENSING→ACTIVE→PURGING |
| **AP fallback WiFi** | ESP32-S3 `network.cpp` | Implement in Rust using `esp-idf-svc::wifi`. Fall back to AP mode for provisioning when no credentials stored |
| **JSON config on flash** | ESP32-S3 `config.cpp` | Use `serde_json` + NVS or LittleFS in Rust. Keep dirty-flag pattern for write optimization |
| **Periodic timer architecture** | ESP32-S3 `timedtasks.cpp` | Use embassy async timers or FreeRTOS software timers in Rust. Keep multi-interval concept (sensor=10Hz, control=1Hz, telemetry=0.1Hz) |
| **Edge-detection button logic** | Both platforms | Port debounce logic to Rust. Use interrupt-based GPIO with edge detection instead of polling |
| **mDNS hostname** | ESP32-S3 `network.cpp` | Already in ESP-IDF. Advertise PetFilter as `petfilter.local` |
| **WiFi provisioning portal** | `wifimanager.html` | Adapt HTML for BLE-based provisioning or keep captive portal approach. Fix security (password field, HTTPS) |
| **NeoPixel status LED** | Both platforms `neopixel.cpp` | Port color-coding concept: teal=idle, blue=sensing, green=active, amber=attention, red=error. Use non-blocking animations |
| **Power management PCB circuit** | KiCad PCB | MCP73831 + AP2112K circuit is proven and portable. **However**: PetFilter uses external 12V PSU, so battery charging is not needed. LDO circuit still useful for 3.3V rail |
| **MQTT HA auto-discovery** | `hassmqtt.cpp` | Excellent pattern. Implement in Rust for PetFilter: publish air quality data, pump state, filter life to Home Assistant |

### 6.2 MEDIUM VALUE -- Take Inspiration

| Component | Lesson | Apply How |
|-----------|--------|-----------|
| **Pump manual/auto modes** | Two operation modes with distinct behaviors | PetFilter already has this: gas-sensor-triggered (auto) or manual override via app/button |
| **Motion-triggered activation** | PIR sensor wakes system | PetFilter uses gas sensor instead of PIR, but the trigger→timeout→deactivate pattern is identical |
| **Web dashboard design** | Card-based responsive grid | Adapt for PetFilter status dashboard. Add WebSocket for real-time updates (Purbright lacked this) |
| **CSS dark theme** | Navy/teal color scheme | PetFilter brand uses white/teal -- adapt the teal accent color (#1282A2 is close to PetFilter's #00B894) |
| **Config.json structure** | JSON fields for MQTT, WiFi, device state | PetFilter's `config.rs` already covers this. Add MQTT fields when implementing HA integration |
| **5-relay state array** | Multi-actuator state tracking | PetFilter has pump + UVC as two actuators. Array approach generalizes well |
| **Settings menu with timeout** | 4-second auto-exit from settings | Apply to any physical button interface on PetFilter (if adding buttons) |
| **Device ID from MAC** | `generateDeviceID()` using ESP32 MAC | Use for unique device identification in telemetry and HA integration |

### 6.3 LOW VALUE / DROP

| Component | Why Drop |
|-----------|---------|
| **Speaker/RTTTL** | Never implemented. Audio feedback is not a PetFilter requirement |
| **AccumulateData** | Broken/incomplete. Replace with proper telemetry pipeline |
| **BasicMQTT** | Wrong file content (contains unrelated Arduino code). Use `rumqtt` or `esp-mqtt` in Rust |
| **Eagle-imported PCB** | Legacy format, uses raw ESP32 chip. PetFilter uses ESP32-S3-WROOM module -- completely different schematic. Footprint library has some reusable component footprints but KiCad 8 libraries are better |
| **PlatformIO build system** | Replaced by Cargo + esp-rs |
| **Arduino framework** | Replaced by esp-idf-hal in Rust |
| **Seeeduino platform** | 8-bit AVR is far too limited for PetFilter requirements |
| **Schedule.html** | Never implemented. Build from scratch if scheduling is needed |
| **Flow sensor pin** | Defined but unused in original. PetFilter DOES need flow sensing -- implement properly |
| **Manual memory management** | `heapStr`, `StringtoChar()`, `my_delay()` -- all anti-patterns. Rust's ownership system eliminates these |
| **Global mutable state** | `S_OnOff`, `S_ManAut`, etc. as globals -- replaced by Rust's `StateMachine` struct with owned state |

---

## 7. Design Lessons from Purbright

### 7.1 Architecture Lessons

**Lesson 1: Separate concerns properly.**
Purbright mixed LED control with sensor logic, button handling with state transitions, and network code with business logic. PetFilter's hexagonal architecture prevents this.

**Lesson 2: Don't leave features half-implemented.**
Speaker, AccumulateData, Schedule, and BasicMQTT were all stubs or broken. Ship it or strip it. PetFilter's firmware has `TODO` markers but they're in a skeleton -- no false promises of functionality.

**Lesson 3: Persistent config is essential.**
The JSON-on-SPIFFS pattern in Purbright was one of its best features. PetFilter should use NVS (Non-Volatile Storage) on ESP32-S3 with serde serialization -- faster and more wear-resistant than file-based storage.

**Lesson 4: AP fallback is non-negotiable for IoT.**
Purbright's WiFi AP fallback meant the device was always accessible. PetFilter must preserve this: BLE provisioning as primary, AP fallback as secondary.

### 7.2 Hardware Lessons

**Lesson 5: Use a module, not a raw chip.**
Purbright's PCB used the raw ESP3212 QFN48 -- requiring external flash, crystal, antenna matching, and careful RF layout. PetFilter correctly uses ESP32-S3-WROOM-1 module which integrates all of this. Dramatically simpler PCB, better RF performance, pre-certified for FCC.

**Lesson 6: The power management circuit works.**
MCP73831 + AP2112K is a proven, compact power solution. While PetFilter uses external PSU (not battery), the 3.3V LDO circuit pattern is directly reusable.

**Lesson 7: Footprint libraries have limited shelf life.**
Eagle-imported footprints from 2022 are outdated. KiCad 8's built-in libraries are comprehensive and current. Don't carry forward old footprints -- use KiCad 8 standard library.

### 7.3 Software Lessons

**Lesson 8: Blocking delays kill responsiveness.**
`my_delay()` and `delay()` caused missed button presses and delayed sensor readings. PetFilter's async architecture (embassy or FreeRTOS tasks) eliminates this.

**Lesson 9: Authentication is not optional.**
Purbright's web server had zero auth -- anyone on the network could control the device. PetFilter must implement at minimum a device password for web/BLE access.

**Lesson 10: Real-time updates need WebSocket.**
Purbright required page refresh to see state changes. PetFilter should use WebSocket or Server-Sent Events for live sensor data streaming.

---

## 8. Migration Checklist

### Carry Forward to PetFilter

- [ ] Port AP fallback WiFi provisioning logic (Rust)
- [ ] Port edge-detection button debouncing (Rust)
- [ ] Port NeoPixel color-coding status scheme (Rust)
- [ ] Port MQTT Home Assistant auto-discovery pattern (Rust)
- [ ] Port mDNS hostname advertisement (Rust)
- [ ] Port JSON config persistence with dirty-flag (Rust/NVS)
- [ ] Port periodic timer multi-interval architecture (Rust/embassy)
- [ ] Adapt WiFi manager HTML for PetFilter provisioning
- [ ] Adapt card-based web dashboard layout
- [ ] Reference teal accent color for brand consistency

### Explicitly Do NOT Carry Forward

- [ ] No global mutable state
- [ ] No blocking delays
- [ ] No manual memory management
- [ ] No unauthenticated web endpoints
- [ ] No Eagle-imported PCB designs
- [ ] No Arduino framework dependencies
- [ ] No incomplete feature stubs
- [ ] No hardcoded credentials
- [ ] No raw ESP32 chip (use module)
- [ ] No polling-based button reading (use interrupts)
