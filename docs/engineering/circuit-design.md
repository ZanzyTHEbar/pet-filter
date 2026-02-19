# PetFilter Circuit Design Specification

**Rev 0.1** | **2026-02-15** | **Definitive Electrical Design**

---

## 1. Design Overview

Single PCB, 100×80mm, 2-layer, hosts all electronics for the PetFilter venturi-based pet odor scrubber.

**Power**: 12V DC external PSU → on-board 5V (MP1584EN) → 3.3V (AMS1117-3.3)
**Control**: ESP32-S3-WROOM-1-N16R8 (WiFi + BLE, OTA, sensor fusion)
**Actuators**: Peristaltic pump (DRV8871 H-bridge), UVC LEDs (PT4115 CC driver)
**Sensors**: NH3 (MQ-137), flow (YF-S201), water level ×2 (XKC-Y25), temperature (10K NTC)

---

## 2. Power Distribution

### 2.1 Input Stage (12V Rail)

```
J1 (Barrel Jack) → F1 (5A Fuse) → D1 (SMBJ15A TVS) → +12V Rail
                                                         ├── C1: 100µF/25V electrolytic
                                                         └── C2: 100nF/50V ceramic
```

| Ref | Component | Value | Footprint | Purpose |
|-----|-----------|-------|-----------|---------|
| J1 | DC Barrel Jack | 5.5×2.1mm | BarrelJack_Horizontal | Power input |
| F1 | Fuse | 5A Mini Blade | Fuseholder_Blade_Mini | Overcurrent protection |
| D1 | TVS Diode | SMBJ15A | D_SMB | Transient voltage suppression |
| C1 | Electrolytic Cap | 100µF/25V | CP_Radial_D8.0mm | Bulk input decoupling |
| C2 | Ceramic Cap | 100nF/50V | C_0603 | HF input decoupling |

### 2.2 12V → 5V Buck Converter (MP1584EN)

**Design targets**: Vin=12V, Vout=5.0V, Iout_max=3A, fsw=500kHz

```
+12V → C3,C4 → U1(IN) → U1(SW) → L1 → +5V → C6,C7
                  U1(BST) ← C5 ← L1 junction
                  U1(FB) ← R1/R2 divider from +5V
                  U1(EN) ← +12V (always on)
                  U1(FREQ) ← NC (float = 500kHz)
                  U1(VCC) → C_VCC (100nF)
                  D2: SS340 from GND to SW node
```

**Feedback Divider** (Vfb = 0.8V):
- R1 (top) = 100kΩ, R2 (bottom) = 19.1kΩ
- Vout = 0.8V × (1 + 100k/19.1k) = 0.8V × 6.236 = **4.99V** ✓

| Ref | Component | Value | Footprint | Notes |
|-----|-----------|-------|-----------|-------|
| U1 | Buck Converter | MP1584EN | SOIC-8 | 4.5-28V in, 3A |
| C3 | Ceramic Cap | 22µF/25V | C_0805 | Input bulk |
| C4 | Ceramic Cap | 100nF/50V | C_0603 | Input HF |
| C5 | Ceramic Cap | 100nF | C_0603 | Bootstrap |
| C6 | Ceramic Cap | 22µF/10V | C_0805 | Output |
| C7 | Ceramic Cap | 22µF/10V | C_0805 | Output |
| C8 | Ceramic Cap | 100nF | C_0603 | VCC decoupling |
| L1 | Inductor | 10µH/3A | L_Bourns_SRN6045 | Main inductor |
| D2 | Schottky Diode | SS340 (3A/40V) | D_SMA | Freewheeling |
| R1 | Resistor | 100kΩ | R_0603 | FB divider top |
| R2 | Resistor | 19.1kΩ | R_0603 | FB divider bottom |

### 2.3 5V → 3.3V LDO (AMS1117-3.3)

**Design targets**: Vin=5V, Vout=3.3V, Iout_max=1A, dropout=1.1V

| Ref | Component | Value | Footprint | Notes |
|-----|-----------|-------|-----------|-------|
| U2 | LDO Regulator | AMS1117-3.3 | SOT-223-3 | Fixed 3.3V out |
| C9 | Ceramic Cap | 10µF/10V | C_0805 | Input |
| C10 | Ceramic Cap | 22µF/10V | C_0805 | Output (stability) |

---

## 3. ESP32-S3 MCU

### 3.1 Pin Allocation (Definitive)

| Function | ESP32 Pin | Type | Notes |
|----------|-----------|------|-------|
| NH3 Analog | GPIO1 (ADC1_CH0) | Analog In | Through voltage divider |
| Temp Analog | GPIO2 (ADC1_CH1) | Analog In | NTC divider |
| Pump IN1 | GPIO4 | PWM Out | LEDC Ch0 to DRV8871 |
| Pump IN2 | GPIO5 | PWM Out | LEDC Ch1 to DRV8871 |
| UVC DIM | GPIO6 | Digital Out | PT4115 dimming control |
| UVC Interlock | GPIO7 | Digital In | Reed switch, 10k pull-up |
| Flow Pulse | GPIO8 | Digital In | YF-S201 pulse, interrupt |
| Water Level A | GPIO9 | Digital In | Tank A sensor |
| Water Level B | GPIO10 | Digital In | Tank B sensor |
| MQ Heater EN | GPIO15 | Digital Out | MOSFET gate for MQ-137 heater |
| USB D- | GPIO19 | USB | USB-C data |
| USB D+ | GPIO20 | USB | USB-C data |
| I2C SDA | GPIO21 | I2C | Future expansion |
| I2C SCL | GPIO38 | I2C | Future expansion |
| UART TX | GPIO43 | UART | Debug serial |
| UART RX | GPIO44 | UART | Debug serial |
| WS2812B Data | GPIO48 | Digital Out | Status LED |

**Design rationale**:
- ADC1 channels (GPIO1-10) used for sensors — ADC1 works during WiFi unlike ADC2
- GPIO0 reserved for boot button (strapping pin)
- GPIO19/20 are the native USB pins
- GPIO45/46 avoided (strapping pins)
- GPIO43/44 are default UART0

### 3.2 Decoupling

| Ref | Component | Value | Net | Notes |
|-----|-----------|-------|-----|-------|
| C11 | Ceramic | 10µF | 3V3 RF pins 2,3 | Per Espressif HW guide |
| C12 | Ceramic | 100nF | 3V3 RF pins 2,3 | HF bypass, close to pins |
| C13 | Ceramic | 100nF | VDD3P3_CPU | Close to CPU power pin |
| C14 | Ceramic | 10µF | 3V3 bulk | Near module |

### 3.3 Reset & Boot

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| R3 | Resistor | 10kΩ | EN pin pull-up to 3.3V |
| C15 | Ceramic | 100nF | EN pin RC delay (power-on reset) |
| R4 | Resistor | 10kΩ | GPIO0 pull-up (normal boot) |
| SW1 | Tactile | - | Boot button: GPIO0 → GND |
| SW2 | Tactile | - | Reset button: EN → GND |

### 3.4 USB-C (Debug/Programming)

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| J2 | USB-C Receptacle | USB2.0 | Debug/programming port |
| R5 | Resistor | 22Ω | USB D- series (GPIO19) |
| R6 | Resistor | 22Ω | USB D+ series (GPIO20) |
| R7 | Resistor | 5.1kΩ | CC1 pull-down (device mode) |
| R8 | Resistor | 5.1kΩ | CC2 pull-down (device mode) |

### 3.5 Status LED

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| U4 | WS2812B | - | Addressable RGB LED |
| C16 | Ceramic | 100nF | WS2812B decoupling |
| R9 | Resistor | 100Ω | Data line series (EMI) |

---

## 4. Pump Motor Driver (DRV8871)

**Design targets**: 12V motor, 500mA nominal, 1.5A trip limit

```
+12V → C17,C18 → U5(VVM)
ESP32 GPIO4 → R10(100Ω) → U5(IN1)
ESP32 GPIO5 → R11(100Ω) → U5(IN2)
U5(OUT1) → J3 pin 1 (pump motor +)
U5(OUT2) → J3 pin 2 (pump motor -)
R12(86.6kΩ) from ILIM to GND
```

**Current limit**: I_TRIP = K_ILIM / R_ILIM = 128.75kΩ·A / 86.6kΩ = **1.49A** ≈ 1.5A

| Ref | Component | Value | Footprint | Notes |
|-----|-----------|-------|-----------|-------|
| U5 | Motor Driver | DRV8871 | HSOP-8 PowerPAD | 3.6A, built-in current sense |
| C17 | Ceramic Cap | 100nF | C_0603 | VVM HF decoupling |
| C18 | Ceramic Cap | 10µF/25V | C_0805 | VVM bulk |
| R10 | Resistor | 100Ω | R_0603 | IN1 ESD protection |
| R11 | Resistor | 100Ω | R_0603 | IN2 ESD protection |
| R12 | Resistor | 86.6kΩ | R_0603 | Current limit set |
| J3 | Connector | Molex Micro-Fit 4-pin | Micro-Fit 3.0 | Pump motor |

**DRV8871 control modes** (firmware):
- Coast (IN1=L, IN2=L): Motor off, low power
- Forward (IN1=PWM, IN2=L): Pump forward
- Reverse (IN1=L, IN2=PWM): Pump reverse (flush)
- Brake (IN1=H, IN2=H): Active braking

---

## 5. UVC LED Driver (PT4115)

**Design targets**: Vin=12V, I_LED=350mA, 5 LEDs in series (Vf≈4.5V each → 22.5V total)

Wait — 5 LEDs × 4.5V = 22.5V > 12V input. The PT4115 is a buck driver: Vout < Vin.
**Revised**: Drive 2 LEDs in series (Vf=9V total) at 350mA per string, with 2-3 parallel strings on separate drivers, OR use a boost driver.

**Practical design**: Use 2 LEDs in series per string (Vf ≈ 9V), PT4115 can drive this from 12V (headroom = 3V). Run 2-3 parallel strings controlled by a single DIM signal.

For POC: **Single string, 2 UVC LEDs, 350mA**.

```
+12V → C19,C20 → U6(VIN)
U6(SW) → L2(68µH) → UVC_LED+ (J4 pin 1)
UVC_LED- (J4 pin 2) → R14(0.3Ω) → GND
D3: SS340 cathode to SW, anode to GND
GPIO6 → R15(1kΩ) → U6(DIM) ← R16(4.7kΩ) → GND
U6(CSN) → junction of R14 and LED-
```

**LED current**: I_LED = 0.1V / R_sense = 0.1V / 0.3Ω = **333mA** ≈ 350mA

**DIM control** (fail-off design):
- R15 (1kΩ series) + R16 (4.7kΩ pull-down) ensures DIM defaults LOW when ESP32 floating
- GPIO HIGH (3.3V): V_DIM = 3.3 × 4.7/(1+4.7) = 2.72V > 1.5V → ON ✓
- GPIO LOW (0V): V_DIM = 0V < 0.3V → OFF ✓
- GPIO floating: V_DIM ≈ 50µA × 4.7kΩ = 0.235V < 0.3V → OFF ✓ (fail-safe)

| Ref | Component | Value | Footprint | Notes |
|-----|-----------|-------|-----------|-------|
| U6 | LED Driver | PT4115 | SOT-89-5 | 30V, 1.2A CC buck |
| C19 | Electrolytic | 100µF/25V | CP_Radial_D8.0mm | Input bulk |
| C20 | Ceramic | 100nF/50V | C_0603 | Input HF |
| L2 | Inductor | 68µH/1A | L_Bourns_SRN6045 | Main inductor |
| D3 | Schottky Diode | SS340 | D_SMA | Freewheeling |
| R14 | Resistor | 0.3Ω 1W | R_2512 | Current sense |
| R15 | Resistor | 1kΩ | R_0603 | DIM series |
| R16 | Resistor | 4.7kΩ | R_0603 | DIM pull-down (fail-off) |
| J4 | Connector | Molex Micro-Fit 2-pin | Micro-Fit 3.0 | UVC LED string |

### 5.1 Interlock Circuit

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| J5 | Connector | JST-PH 2-pin | Reed switch connector |
| R17 | Resistor | 10kΩ | GPIO7 pull-up to 3.3V |

Reed switch closed (chamber sealed) → GPIO7 = LOW → safe
Reed switch open (chamber open) → GPIO7 = HIGH → fault → firmware disables UVC

---

## 6. Sensor Interfaces

### 6.1 MQ-137 NH3 Sensor

**Heater**: 5V, ~150mA — switched by N-MOSFET for power control.
**Signal**: Analog 0-5V — level-shifted via voltage divider.

```
+5V → MQ-137 heater → Q1(drain)
Q1(source) → GND
Q1(gate) → GPIO15, R18(100kΩ) pull-down to GND

MQ-137 AOUT → R19(10kΩ) → ADC junction → R20(15kΩ) → GND
ADC junction → GPIO1 (ADC1_CH0)
```

**Voltage divider**: V_adc = V_sensor × 15k/(10k+15k) = V_sensor × 0.6
Max 5V → 3.0V at ADC (within 3.3V limit) ✓

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| Q1 | N-MOSFET | IRLML2502 | Heater switch (Vgsth ~1V) |
| R18 | Resistor | 100kΩ | Gate pull-down |
| R19 | Resistor | 10kΩ | Voltage divider top |
| R20 | Resistor | 15kΩ | Voltage divider bottom |
| R21 | Resistor | 10kΩ | MQ-137 load resistor |
| J6 | Connector | JST-PH 4-pin | MQ-137 connector (VH, GND, A, B) |

### 6.2 YF-S201 Flow Sensor

**Signal**: 5V square wave — level-shifted for 3.3V GPIO.

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| R22 | Resistor | 10kΩ | Voltage divider top |
| R23 | Resistor | 18kΩ | Voltage divider bottom |
| J7 | Connector | JST-PH 3-pin | Flow sensor (VCC, GND, PULSE) |

V_gpio = 5V × 18k/(10k+18k) = **3.21V** (≤ 3.3V) ✓

### 6.3 XKC-Y25-T12V Water Level Sensors (×2)

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| R24 | Resistor | 10kΩ | Pull-up to 3.3V (sensor A) |
| R25 | Resistor | 10kΩ | Pull-up to 3.3V (sensor B) |
| J8 | Connector | JST-PH 3-pin | Water level A (VCC, GND, OUT) |
| J9 | Connector | JST-PH 3-pin | Water level B (VCC, GND, OUT) |

Powered at 5V, NPN open-collector output pulled to 3.3V externally.

### 6.4 NTC Thermistor

| Ref | Component | Value | Purpose |
|-----|-----------|-------|---------|
| R26 | Resistor | 10kΩ | Fixed half of voltage divider |
| J10 | Connector | JST-PH 2-pin | NTC thermistor connector |

**Divider**: V_adc = 3.3V × NTC / (10kΩ + NTC)
- At 25°C (NTC=10kΩ): 1.65V
- At 80°C (NTC≈1.2kΩ): 0.35V

---

## 7. Reference Designator Summary

| Ref Range | Count | Type |
|-----------|-------|------|
| U1-U6 | 6 | ICs (MP1584EN, AMS1117, ESP32, WS2812B, DRV8871, PT4115) |
| R1-R26 | 26 | Resistors |
| C1-C20 | 20 | Capacitors |
| L1-L2 | 2 | Inductors |
| D1-D3 | 3 | Diodes |
| Q1 | 1 | MOSFET |
| F1 | 1 | Fuse |
| J1-J10 | 10 | Connectors |
| SW1-SW2 | 2 | Switches |
| **Total** | **71** | |

---

## 8. Net List (Inter-block Connections)

### Power Nets
| Net Name | Source | Sinks |
|----------|--------|-------|
| +12V | J1 → F1 → D1 | U1(IN), U5(VVM), U6(VIN), C1, C2 |
| +5V | U1 output | U2(VIN), MQ-137 heater, flow sensor, water level sensors |
| +3V3 | U2 output | ESP32, WS2812B, pull-ups, NTC divider |
| GND | Common | All components |

### Signal Nets
| Net Name | Source | Destination | Notes |
|----------|--------|-------------|-------|
| PUMP_IN1 | ESP32 GPIO4 | DRV8871 IN1 | Via 100Ω series |
| PUMP_IN2 | ESP32 GPIO5 | DRV8871 IN2 | Via 100Ω series |
| UVC_DIM | ESP32 GPIO6 | PT4115 DIM | Via 1k + 4.7k divider |
| INTERLOCK | Reed switch | ESP32 GPIO7 | 10k pull-up |
| NH3_ADC | MQ-137 divider | ESP32 GPIO1 | 0.6× attenuation |
| TEMP_ADC | NTC divider | ESP32 GPIO2 | Direct |
| FLOW_PULSE | YF-S201 divider | ESP32 GPIO8 | 0.64× attenuation |
| WATER_A | XKC-Y25 #1 | ESP32 GPIO9 | 10k pull-up |
| WATER_B | XKC-Y25 #2 | ESP32 GPIO10 | 10k pull-up |
| MQ_HEATER | ESP32 GPIO15 | Q1 gate | 100k pull-down |
| WS2812_DATA | ESP32 GPIO48 | WS2812B DIN | Via 100Ω |
| USB_DN | ESP32 GPIO19 | USB-C D- | Via 22Ω |
| USB_DP | ESP32 GPIO20 | USB-C D+ | Via 22Ω |

---

## 9. Design Rules & Constraints

### PCB
- **Layers**: 2 (top + bottom)
- **Size**: 100mm × 80mm
- **Min trace**: 0.25mm (signal), 1.0mm (power)
- **Min via**: 0.6mm diameter, 0.3mm drill
- **Power traces**: +12V ≥1mm, +5V ≥0.5mm, +3V3 ≥0.25mm
- **Ground plane**: Solid pour on bottom layer

### Placement
- Buck converter (U1) components close together, short SW loop
- AMS1117 input/output caps within 5mm of IC
- ESP32 decoupling caps within 3mm of module pins
- MQ-137 connector near board edge (external sensor)
- USB-C connector at board edge
- Barrel jack at board edge

### Thermal
- DRV8871 exposed pad: large copper pour + thermal vias to bottom layer
- AMS1117 tab: connected to output copper pour
- PT4115 tab pad: thermal relief to ground
