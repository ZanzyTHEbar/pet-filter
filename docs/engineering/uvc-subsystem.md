# UVC Subsystem Design Specification

## 1. Overview

The UVC subsystem irradiates water flowing from the air-water contact zone to photochemically degrade dissolved odor compounds. UVC light at 265-280nm breaks molecular bonds in ammonia, sulfur compounds, and organic molecules through direct photolysis and advanced oxidation processes.

---

## 2. UVC LED Selection

### 2.1 Wavelength
- **Target**: 265-280nm (deep UVC)
- **Optimal**: 265nm (peak germicidal effectiveness, best NH3 absorption)
- **Practical**: 275-280nm (better LED efficiency, lower cost, adequate performance)
- **NOT suitable**: 365nm (UVA), 315nm (UVB) -- wrong wavelength for photolysis

### 2.2 LED Module Options

| Module | Wavelength | Power | Flow Rate | Dimensions | Price | Notes |
|--------|-----------|-------|-----------|------------|-------|-------|
| WM aquatec UVC LED | 275nm | 14W (max) | 2-8 L/min | 113×159×105mm | ~$45 | Compact, integrated |
| Seoul Viosys CUD8AF4A | 275nm | 100mW optical | - | SMD 3535 | ~$8 | Bare LED, needs driver |
| Crystal IS Klaran WD | 265nm | 30mW optical | - | SMD 3535 | ~$15 | Premium LED |
| Generic 3W UVC module | 275nm | 3W elec | - | 25mm dia | ~$12 | Needs heatsink |

### 2.3 Recommended Approach

**POC**: 3-5 individual UVC LEDs (275nm, 1W each) on custom heatsink
- Total electrical: 15-25W
- Total optical: ~0.5-1.5W (3-10% wall plug efficiency)
- Adequate for validation

**Production**: WM aquatec integrated module OR custom LED array
- Total electrical: 14-30W
- Total optical: 1-3W
- 5000+ hour lifetime

---

## 3. Exposure Chamber Design

### 3.1 Geometry
- **Type**: Flow-through chamber with UVC LEDs mounted on one side
- **Water depth**: 2-5 cm (UVC penetration in clear water)
- **Chamber volume**: 30-50 mL
- **Flow rate**: 1 L/min (16.7 mL/s)
- **Residence time**: 50mL / 16.7 mL/s = 3.0 seconds per pass

### 3.2 UV Dose Calculation

For 5 × 1W UVC LEDs at 5% wall plug efficiency:
- Optical power: 5 × 0.05W = 0.25W = 250 mW
- Irradiated area: ~10 cm² (LED array footprint)
- Fluence rate: 250 / 10 = 25 mW/cm²
- Per-pass dose: 25 × 3.0 = 75 mJ/cm²

For WM aquatec 14W module at ~5% efficiency:
- Optical power: ~700 mW
- Fluence rate: ~35 mW/cm²
- Per-pass dose: 35 × 3.0 = 105 mJ/cm²

### 3.3 Dose Requirements

| Application | Required Dose | Our Dose | Status |
|-------------|--------------|----------|--------|
| E. coli 4-log kill | 40 mJ/cm² | 75-105 | PASS |
| Mercaptan photolysis (partial) | 100 mJ/cm² | 75-105 | MARGINAL |
| H2S photolysis (partial) | 100-200 mJ/cm² | 75-105 | MARGINAL |
| NH3 direct photolysis | 200-500 mJ/cm² | 75-105 | INSUFFICIENT |

**Key insight**: Direct UVC photolysis of ammonia is insufficient at these doses. The primary ammonia removal mechanism is WATER SCRUBBING (dissolution). UVC serves as a secondary treatment that:
1. Disinfects the water (prevents bacterial growth)
2. Partially degrades sulfur compounds and mercaptans
3. Generates reactive oxygen species (ROS) for indirect oxidation
4. Combined with dissolved O2 from venturi, creates mild AOP conditions

---

## 4. Advanced Oxidation Enhancement

### 4.1 TiO2 Photocatalysis (Optional for V2)
- Coat chamber interior with anatase TiO2 nanoparticles
- UVC activates TiO2 → generates •OH radicals
- •OH is a powerful non-selective oxidizer (E° = 2.80V)
- Dramatically improves degradation of all dissolved compounds
- Added cost: ~$5-10 per unit

### 4.2 Dissolved Oxygen Advantage
The venturi naturally supersaturates water with dissolved oxygen:
- Ambient water: ~8 mg/L dissolved O2
- After venturi: ~12-15 mg/L (supersaturated)
- UVC + O2 → O3 (trace) → •OH (trace)
- This provides mild AOP without any chemical additives

---

## 5. Safety Design

### 5.1 UVC Containment
- Chamber material: Opaque PP or ABS (zero UV transmission)
- ALL joints sealed with silicone gaskets
- No transparent windows (UVC must not escape)

### 5.2 Interlock System
- Magnetic reed switch on chamber lid/cover
- UVC LEDs ONLY energize when interlock is closed
- Hardware interlock (not just software) -- relay in series with LED power
- If lid opens: LEDs de-energize within 10ms

### 5.3 Electrical Safety
- UVC LED driver: Constant current, isolated
- Over-temperature protection: NTC thermistor on LED heatsink
- Thermal cutoff: 80°C on heatsink (LEDs derate above 60°C)
- Water ingress protection: Conformal coating on driver PCB

### 5.4 End-of-Life
- UVC LEDs (no mercury) -- standard electronics recycling
- No hazardous materials (unlike mercury UV lamps)
- RoHS compliant

---

## 6. Thermal Management

### 6.1 Heat Generation
- UVC LEDs: 14-25W electrical → ~1-2W optical = 12-23W as heat
- LED junction temperature max: 85°C (typical)
- Ambient temperature range: 10-40°C

### 6.2 Cooling Strategy
- **Primary**: Water flowing past LED heatsink (water-cooled)
- Water temperature rise: ΔT = P/(ṁ × Cp) = 23W / (16.7g/s × 4.18 J/g·K) = 0.33°C
- This is negligible -- water provides excellent cooling
- **Secondary**: Aluminum heatsink bonded to LED array
- **Backup**: Thermal shutdown at 80°C junction temperature

---

## 7. Electrical Interface

### 7.1 Power
- Input: 12V DC from main power rail
- LED driver: Buck constant-current driver
- Current per LED: 350-700mA (depends on LED selection)
- Total: 1-2.5A at 12V

### 7.2 Control
- Enable signal from ESP32 GPIO (3.3V logic)
- PWM dimming possible (for power optimization)
- Status feedback: LED current sense to ADC
- Interlock input: Digital GPIO (reed switch)

### 7.3 Connector
- Molex Micro-Fit 3.0 or similar (rated for 5A)
- 4-pin: VCC, GND, ENABLE, INTERLOCK

---

## 8. Water Quality Considerations

### 8.1 UV Transmission
- Clean water transmits UVC well (>90% per cm at 254nm)
- Dissolved ammonia slightly reduces UVC transmission
- Particulates scatter UVC -- pre-filtration needed
- Colored dissolved organics absorb UVC -- reduces effectiveness over time

### 8.2 Water Replacement Trigger
- When dissolved compound concentration reduces UVC transmission below 70%
- Practical indicator: water color change (yellowing indicates saturation)
- Recommendation: Replace water every 7-14 days
- Can be automated: UV transmission sensor (photodiode behind water path)

---

## 9. Bill of Materials (UVC Subsystem)

| Item | Qty | Unit Cost | Total |
|------|-----|-----------|-------|
| UVC LED (275nm, 1W) | 5 | $8-12 | $40-60 |
| Aluminum heatsink | 1 | $3-5 | $3-5 |
| Buck CC LED driver | 1 | $3-5 | $3-5 |
| Chamber housing (molded PP) | 1 | $2-4 | $2-4 |
| Silicone gaskets | 2 | $0.50 | $1 |
| Reed switch (interlock) | 1 | $0.50 | $0.50 |
| Connector + wiring | 1 | $1-2 | $1-2 |
| **TOTAL** | | | **$50-77** |

Note: Using integrated module (WM aquatec) reduces assembly but costs ~$45 alone. Custom LED array allows more design freedom and potentially lower cost at volume.
