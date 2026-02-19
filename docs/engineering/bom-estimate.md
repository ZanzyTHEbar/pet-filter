# Bill of Materials (BOM) Estimate

## 1. Electronics

| # | Component | Part Number / Class | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|-------------------|-----|----------------|------------------|--------|
| 1 | ESP32-S3-WROOM-1 (16MB flash, 8MB PSRAM) | ESP32-S3-WROOM-1-N16R8 | 1 | $4.50 | $3.20 | LCSC / Mouser |
| 2 | 12V→5V Buck Converter (3A) | MP1584EN module | 1 | $1.50 | $0.80 | LCSC |
| 3 | 12V→3.3V Buck Converter (1A) | AMS1117-3.3 LDO | 1 | $0.30 | $0.15 | LCSC |
| 4 | Pump Motor Driver (H-bridge) | DRV8871 or L298N mini | 1 | $2.00 | $1.20 | Mouser / LCSC |
| 5 | UVC LED Driver (CC buck) | AL8860 or PT4115 | 1 | $1.50 | $0.80 | LCSC |
| 6 | NH3 Gas Sensor | MQ-137 | 1 | $6.00 | $4.50 | Aliexpress / LCSC |
| 7 | Flow Sensor | YF-S201 (hall-effect) | 1 | $4.00 | $2.50 | Aliexpress |
| 8 | Water Level Sensor (capacitive) | XKC-Y25-T12V | 2 | $2.00 | $1.50 | Aliexpress |
| 9 | NTC Thermistor (10K) | Standard 10K NTC | 1 | $0.20 | $0.10 | LCSC |
| 10 | Reed Switch (interlock) | MKA-14103 | 1 | $0.50 | $0.30 | LCSC |
| 11 | RGB LED (status) | WS2812B or discrete | 1 | $0.30 | $0.15 | LCSC |
| 12 | USB-C connector (debug) | GCT USB4105 | 1 | $0.80 | $0.50 | LCSC |
| 13 | DC barrel jack (5.5×2.1mm) | Standard panel mount | 1 | $0.50 | $0.30 | LCSC |
| 14 | Fuse holder + 5A fuse | Standard blade fuse | 1 | $0.50 | $0.30 | LCSC |
| 15 | TVS diode (12V rail) | SMBJ15A | 1 | $0.30 | $0.15 | LCSC |
| 16 | Capacitors, resistors, passives | Assorted | ~30 | $2.00 | $1.00 | LCSC |
| 17 | PCB (2-layer, 100×80mm) | Custom | 1 | $5.00 | $2.00 | JLCPCB |
| 18 | JST connectors (sensor) | JST-PH 2.0mm | 6 | $1.80 | $0.90 | LCSC |
| 19 | Molex connector (pump) | Molex Micro-Fit 3.0 | 1 | $0.80 | $0.50 | LCSC |
| **Electronics Subtotal** | | | | **$32.50** | **$20.85** | |

## 2. Mechanical / Fluid

| # | Component | Specification | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|--------------|-----|----------------|------------------|--------|
| 20 | Peristaltic Pump | Kamoer KCM 12V stepper, 0-500mL/min | 1 | $25.00 | $15.00 | Kamoer direct |
| 21 | Silicone Tubing (10mm ID) | Food-grade, 1m length | 1 | $3.00 | $1.50 | Aliexpress |
| 22 | Silicone Tubing (3mm ID) | Food-grade, 0.5m length | 1 | $1.50 | $0.80 | Aliexpress |
| 23 | Venturi Nozzle | Custom (3D print POC / injection mold prod) | 1 | $5.00 | $3.00 | Custom |
| 24 | Water Tank A | 2.5L, PP, blow molded | 1 | $8.00 | $4.00 | Custom mold |
| 25 | Water Tank B | 2.5L, PP, blow molded | 1 | $8.00 | $4.00 | Custom mold |
| 26 | Contact Column (clear) | Polycarbonate tube, 50mm × 150mm | 1 | $4.00 | $2.00 | McMaster / custom |
| 27 | Barbed Fittings (10mm) | PP or nylon | 4 | $2.00 | $1.00 | Aliexpress |
| 28 | Barbed Fittings (3mm) | PP or nylon | 2 | $1.00 | $0.50 | Aliexpress |
| 29 | O-rings (silicone) | Assorted sizes | 6 | $1.50 | $0.60 | Aliexpress |
| **Mechanical Subtotal** | | | | **$59.00** | **$32.40** | |

## 3. UVC

| # | Component | Specification | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|--------------|-----|----------------|------------------|--------|
| 30 | UVC LED (275nm, 1W) | Seoul Viosys or generic | 5 | $50.00 | $35.00 | Digikey / direct |
| 31 | Aluminum Heatsink | 50×50×10mm, machined | 1 | $4.00 | $2.00 | Custom / McMaster |
| 32 | Thermal paste | Arctic Silver or similar | 1 | $0.50 | $0.20 | Various |
| 33 | UVC Chamber Housing | Opaque PP, gasketed | 1 | $3.00 | $2.00 | Custom mold |
| 34 | Silicone Gaskets | Custom cut | 2 | $1.00 | $0.50 | Custom |
| 35 | Magnet (interlock) | 10mm neodymium disc | 1 | $0.50 | $0.30 | Aliexpress |
| **UVC Subtotal** | | | | **$59.00** | **$40.00** | |

## 4. Filtration

| # | Component | Specification | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|--------------|-----|----------------|------------------|--------|
| 36 | HEPA H13 Element | 80×80×20mm panel | 1 | $3.00 | $1.50 | Aliexpress / custom |
| 37 | Activated Carbon (granular) | Coconut shell, 200g total | 1 | $2.00 | $1.00 | Bulk supplier |
| 38 | Carbon Filter Pouches | Mesh fabric, heat-sealed | 2 | $1.00 | $0.50 | Custom |
| 39 | Mist Eliminator Pad | PP mesh, 50×50×15mm | 1 | $1.00 | $0.50 | Custom |
| 40 | Filter Housing (intake) | ABS, snap-fit | 1 | $2.00 | $1.00 | Custom mold |
| 41 | Filter Housing (exhaust) | ABS, snap-fit | 1 | $2.00 | $1.00 | Custom mold |
| **Filtration Subtotal** | | | | **$11.00** | **$5.50** | |

## 5. Housing & Assembly

| # | Component | Specification | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|--------------|-----|----------------|------------------|--------|
| 42 | Main Housing (top shell) | ABS, matte white, injection molded | 1 | $15.00 | $8.00 | Custom mold |
| 43 | Main Housing (bottom shell) | ABS, matte white, injection molded | 1 | $12.00 | $7.00 | Custom mold |
| 44 | Internal Partitions | ABS, integral or separate | 2 | $4.00 | $2.00 | Custom mold |
| 45 | Rubber Feet | Silicone, adhesive-backed | 4 | $1.00 | $0.40 | Aliexpress |
| 46 | Screws (assembly) | M3 stainless, assorted | 12 | $1.00 | $0.50 | Aliexpress |
| 47 | Acoustic Foam (pump compt.) | Melamine, 10mm | 1 | $1.50 | $0.60 | Aliexpress |
| 48 | Silicone Pump Grommets | Shore A 30-40, vibration isolators | 4 | $1.00 | $0.50 | Custom / Aliexpress |
| 49 | Product Label | Printed, adhesive | 1 | $0.50 | $0.20 | Custom |
| **Housing Subtotal** | | | | **$36.00** | **$19.20** | |

## 6. Power Supply & Packaging

| # | Component | Specification | Qty | Unit Cost (1x) | Unit Cost (500x) | Source |
|---|-----------|--------------|-----|----------------|------------------|--------|
| 50 | External PSU | 12V 5A, 60W, UL listed | 1 | $12.00 | $5.00 | Mean Well / generic |
| 51 | AC Power Cord | IEC C7/C8, region-specific | 1 | $2.00 | $1.00 | Bulk |
| 52 | Retail Box | Printed cardboard | 1 | $3.00 | $1.50 | Custom print |
| 53 | Molded Pulp Insert | Device protection | 1 | $2.00 | $1.00 | Custom |
| 54 | Quick-Start Guide | Printed card | 1 | $0.50 | $0.20 | Custom print |
| **PSU & Packaging Subtotal** | | | | **$19.50** | **$8.70** | |

---

## 7. Total BOM Summary

| Category | 1x Prototype | 500x Production |
|----------|-------------|-----------------|
| Electronics | $32.50 | $20.85 |
| Mechanical / Fluid | $59.00 | $32.40 |
| UVC | $59.00 | $40.00 |
| Filtration | $11.00 | $5.50 |
| Housing & Assembly | $36.00 | $19.20 |
| PSU & Packaging | $19.50 | $8.70 |
| **Component Total** | **$217.00** | **$126.65** |
| Assembly labor (est.) | $20.00 | $8.00 |
| Testing/QC | $5.00 | $3.00 |
| **Landed Cost** | **$242.00** | **$137.65** |

---

## 8. Cost Reduction Roadmap

| Volume | Est. BOM | Notes |
|--------|----------|-------|
| 1-10 (POC) | $200-250 | Off-the-shelf, 3D printed housing |
| 50-100 (pre-prod) | $160-190 | Soft tooling, semi-manual assembly |
| 500+ (production) | $125-140 | Full tooling, contract assembly |
| 2000+ (scale) | $100-120 | Volume discounts, design optimization |
| 5000+ (mature) | $85-105 | Second-source components, VE |
