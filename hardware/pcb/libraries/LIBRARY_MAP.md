# KiCad Library Mapping: PetFilter BOM

## Approach

KiCad 8 ships with extensive standard libraries. We only create custom parts for components
not in the standard set. Everything else uses official KiCad library references.

The project uses two library sources:
1. **KiCad standard libraries** (installed with KiCad 8)
2. **petfilter** (project-specific, in `hardware/pcb/libraries/`)
   - Symbol library: `hardware/pcb/libraries/petfilter.kicad_sym`
   - Footprint library: `hardware/pcb/libraries/petfilter.pretty/`

## BOM Triage

### Already in KiCad Standard Libraries

| Component | KiCad Symbol Library | KiCad Footprint Library | Notes |
|-----------|---------------------|------------------------|-------|
| ESP32-S3-WROOM-1 | RF_Module:ESP32-S3-WROOM-1 | RF_Module:ESP32-S3-WROOM-1 | Standard module |
| AMS1117-3.3 | Regulator_Linear:AMS1117-3.3 | Package_TO_SOT_SMD:SOT-223-3_TabPin2 | Common LDO |
| DRV8871 | Motor:DRV8871 | Package_SO:SOIC-8_3.9x4.9mm_P1.27mm | TI motor driver |
| WS2812B | LED:WS2812B | LED_SMD:LED_WS2812B_PLCC4_5.0x5.0mm_P3.2mm | Addressable RGB |
| SMBJ15A | Diode:SMBJ15A | Diode_SMD:D_SMB | TVS protection |
| USB-C connector | Connector:USB_C_Receptacle | Connector_USB:USB_C_Receptacle_GCT_USB4085 | Debug port |
| DC barrel jack | Connector:Barrel_Jack | Connector_BarrelJack:BarrelJack_Horizontal | Power input |
| JST-PH 2-pin | Connector:Conn_01x02_Pin | Connector_JST:JST_PH_B2B-PH-K_1x02_P2.00mm_Vertical | Sensors |
| JST-PH 3-pin | Connector:Conn_01x03_Pin | Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical | Sensors |
| JST-PH 4-pin | Connector:Conn_01x04_Pin | Connector_JST:JST_PH_B4B-PH-K_1x04_P2.00mm_Vertical | I2C |
| Molex Micro-Fit | Connector:Conn_01x04_Pin | Connector_Molex:Molex_Micro-Fit_3.0_43045-0412_1x04_P3.00mm_Vertical | Pump/UVC |
| NTC 10K | Device:Thermistor_NTC | Resistor_SMD:R_0805_2012Metric or Disc_THT | Temp sense |
| Tactile switch | Switch:SW_Push | Button_Switch_SMD:SW_SPST_TL3305 | Reset/Boot |
| Fuse holder | Fuse:Fuse | Fuse:Fuseholder_Blade_Mini_Keystone_3557 | 5A blade |
| Resistors | Device:R | Resistor_SMD:R_0603_1608Metric | Standard 0603 |
| Capacitors | Device:C | Capacitor_SMD:C_0603_1608Metric | Standard 0603 |
| Electrolytic caps | Device:CP | Capacitor_THT:CP_Radial_D8.0mm_P3.50mm | Bulk decoupling |
| Inductors | Device:L | Inductor_SMD:L_Bourns_SRN6045 | For buck converters |

### Need Custom Library (petfilter.kicad_sym / petfilter.pretty)

| Component | Package | Pin Count | Why Custom? |
|-----------|---------|-----------|-------------|
| PT4115 | SOT-89-5 | 5 | Chinese LED driver, not in KiCad std lib |
| MP1584EN | SOIC-8 | 8 | MPS buck converter, may not be in std lib |
| MQ-137 | 6-pin THT module | 6 | Specialty NH3 gas sensor, definitely not in std |
| XKC-Y25-T12V | 3-wire connector | 3 | Non-contact water level sensor, connector only |
| YF-S201 | 3-wire connector | 3 | Hall-effect flow sensor, connector only |
| Reed switch (generic) | 2-pin THT | 2 | Simple but specific footprint |
| Venturi air port | Custom mechanical | - | Mounting holes / connector pads |

### Generation Priority

1. **PT4115** (SOT-89-5) -- UVC LED driver, critical path
2. **MP1584EN** (SOIC-8) -- Buck converter, power system
3. **MQ-137** (6-pin THT) -- Gas sensor, unique footprint
4. **XKC-Y25-T12V** (connector) -- Water level sensor interface
5. **YF-S201** (connector) -- Flow sensor interface

## Library File Structure

```
hardware/pcb/libraries/
├── LIBRARY_MAP.md          # This file
├── petfilter.kicad_sym     # All custom schematic symbols
└── petfilter.pretty/       # Custom footprints
    ├── SOT-89-5.kicad_mod
    ├── MQ-137.kicad_mod
    └── ... (more as needed)
```

## sym-lib-table Entry (add to project)

```
(sym_lib_table
  (lib (name "petfilter")(type "KiCad")(uri "${KIPRJMOD}/libraries/petfilter.kicad_sym")(options "")(descr "PetFilter custom components"))
)
```

## fp-lib-table Entry (add to project)

```
(fp_lib_table
  (lib (name "petfilter")(type "KiCad")(uri "${KIPRJMOD}/libraries/petfilter.pretty")(options "")(descr "PetFilter custom footprints"))
)
```
