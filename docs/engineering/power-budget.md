# Power Budget Analysis

## 1. System Power Architecture

```
AC Mains (120V/240V)
  └── External PSU (60W, 12V/5A)
        └── 12V DC Rail
              ├── Peristaltic pump motor (12V direct)
              ├── Buck converter → 5V rail
              │     ├── UVC LED driver input
              │     └── Sensor power (5V sensors)
              └── Buck converter → 3.3V rail
                    ├── ESP32-S3
                    ├── I2C/SPI peripherals
                    └── Logic-level signals
```

---

## 2. Component Power Breakdown

### 2.1 Peristaltic Pump

| Parameter | Minimum | Typical | Maximum |
|-----------|---------|---------|---------|
| Voltage | 12V | 12V | 12V |
| Current | 200mA | 400mA | 800mA |
| Power | 2.4W | 4.8W | 9.6W |
| Speed | Low (idle) | Medium (1 L/min) | High (2 L/min) |

Operating at 1 L/min target: ~5W typical

### 2.2 UVC LED Subsystem

| Parameter | POC Config | Production Config |
|-----------|-----------|------------------|
| LED count | 3 × 1W | 5 × 1W |
| Electrical power | 9W | 15W |
| Driver overhead (15%) | 1.4W | 2.3W |
| Total subsystem | 10.4W | 17.3W |

Note: Using integrated module (14W rated) would be 14W + 2W driver = 16W.

### 2.3 ESP32-S3 MCU

| Mode | Current (3.3V) | Power |
|------|---------------|-------|
| Active (WiFi off) | 80mA | 0.26W |
| Active (WiFi TX) | 350mA | 1.16W |
| Active (BLE) | 130mA | 0.43W |
| Light sleep | 2mA | 0.007W |
| Deep sleep | 10μA | 0.00003W |

Typical operating (periodic WiFi): ~0.5W average

### 2.4 Sensors

| Sensor | Voltage | Current | Power | Duty Cycle | Avg Power |
|--------|---------|---------|-------|------------|-----------|
| MQ-137 (NH3) | 5V | 150mA | 0.75W | Continuous (heater) | 0.75W |
| Flow sensor (YF-S201) | 5V | 15mA | 0.075W | Continuous | 0.075W |
| Water level (capacitive) | 3.3V | 5mA | 0.017W | Continuous | 0.017W |
| NTC thermistor | 3.3V | 1mA | 0.003W | Continuous | 0.003W |
| **Sensor total** | | | | | **0.85W** |

Note: MQ-137 heater is the dominant sensor power draw. SGP30 alternative uses only 48mA at 1.8V = 0.086W (major power savings).

### 2.5 Power Conversion Losses

| Converter | Input | Output | Efficiency | Loss |
|-----------|-------|--------|------------|------|
| 12V → 5V buck | 12V | 5V | 90% | ~2W |
| 12V → 3.3V buck | 12V | 3.3V | 88% | ~0.2W |
| **Total conversion loss** | | | | **~2.2W** |

---

## 3. Total Power Summary

### 3.1 Operating Modes

| Mode | Pump | UVC | MCU | Sensors | Conv. Loss | Total |
|------|------|-----|-----|---------|------------|-------|
| **Standby** | 0W | 0W | 0.007W | 0.85W | 0.3W | **1.2W** |
| **Sensing** | 0W | 0W | 0.5W | 0.85W | 0.5W | **1.9W** |
| **Active** | 5W | 17W | 0.5W | 0.85W | 2.2W | **25.5W** |
| **Active + WiFi** | 5W | 17W | 1.2W | 0.85W | 2.2W | **26.3W** |
| **Max (all on, max pump)** | 9.6W | 17W | 1.2W | 0.85W | 2.5W | **31.2W** |

### 3.2 Average Daily Power

Assuming typical usage pattern:
- Standby: 16 hours/day (no litter activity detected)
- Sensing: 4 hours/day (monitoring elevated levels)
- Active: 4 hours/day (scrubbing after litter use)

Average power = (16×1.2 + 4×1.9 + 4×25.5) / 24 = **5.4W average**

### 3.3 Monthly Energy Cost

| Scenario | Avg Power | kWh/month | Cost @ $0.12/kWh |
|----------|-----------|-----------|-------------------|
| Conservative (above pattern) | 5.4W | 3.9 kWh | $0.47 |
| Heavy use (8hr active/day) | 11.5W | 8.3 kWh | $1.00 |
| Continuous operation (24/7) | 25.5W | 18.4 kWh | $2.20 |

**Marketing message**: "Costs less than $1/month to operate in typical use."

---

## 4. Power Supply Specification

### 4.1 Requirements
- Output: 12V DC, 5A (60W)
- Input: 100-240V AC, 50/60Hz (universal)
- Efficiency: >85% (Level VI energy efficiency)
- Safety: UL/CE listed
- Connector: 5.5mm × 2.1mm barrel jack
- Cable length: 6 feet minimum

### 4.2 Recommended Parts
- Mean Well GST60A12 (12V 5A, UL/CE, ~$15)
- Generic 12V 5A adapter (UL listed, ~$5-8)

### 4.3 Why External PSU
- Keeps dangerous AC voltage out of the product housing
- Reduces certification complexity (PSU already UL listed)
- Smaller product housing (no internal AC-DC converter)
- User can replace if PSU fails

---

## 5. Power Budget Margin

| Parameter | Value |
|-----------|-------|
| Max system draw | 31.2W |
| PSU capacity | 60W |
| **Headroom** | **28.8W (48%)** |

48% margin provides room for:
- Component tolerance (+20%)
- Startup inrush current
- Future feature additions (display, extra sensors, fan)
- Temperature derating
