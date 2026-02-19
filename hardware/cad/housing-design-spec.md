# Housing Design Specification

## 1. Overall Dimensions

| Parameter | Value | Notes |
|-----------|-------|-------|
| Width | 350mm (13.8") | Fits beside standard litter box |
| Depth | 250mm (9.8") | Compact front-to-back |
| Height | 400mm (15.7") | Includes water fill area on top |
| Weight (empty) | ~2.5 kg | Housing + electronics |
| Weight (full) | ~7.5 kg | With 5L water |
| Footprint area | 875 cm² | Smaller than a paper towel roll |

## 2. External Features

### 2.1 Top Surface
- Water fill port: 60mm diameter flip-top lid, silicone gasket
- Status LED ring: 30mm diameter, visible from above and sides
- LED colors: Teal (idle), Blue (active), Amber (needs attention), Red (error)
- Surface: Matte finish, slightly recessed to prevent spills from reaching electronics

### 2.2 Front Panel
- Intake filter access door: 100mm × 80mm, spring-loaded latch
- Single press to release, tool-free filter swap
- Filter orientation key (can only insert one way)
- Minimal text/logo badge

### 2.3 Back Panel
- Exhaust filter access: 80mm × 60mm, similar spring latch
- DC barrel jack (5.5 × 2.1mm): Recessed to prevent accidental disconnect
- USB-C port: For firmware debug/update (behind rubber flap)
- Power switch: Rocker switch, illuminated when on

### 2.4 Bottom
- 4× rubber feet (15mm diameter, 5mm height, Shore A 50)
- Water drain plug (for maintenance/storage): 20mm threaded cap
- Product label recess (regulatory markings, serial number)
- Anti-tip weight distribution: heavier toward back

### 2.5 Sides
- Left/right: Baffled ventilation slots (for any residual heat dissipation)
- Slots angled downward to prevent water ingress from above
- No sharp edges anywhere (pet safety)

## 3. Internal Layout

### 3.1 Compartment Map (Top View, from above)

```
┌──────────────────────────────────┐
│         WATER FILL PORT          │  ← Top access
├────────────────┬─────────────────┤
│                │                 │
│    Tank A      │     Tank B      │  ← Upper section
│   (Supply)     │   (Collection)  │
│    2.5 L       │     2.5 L       │
│                │                 │
├────────┬───────┴──────┬──────────┤
│ Pump   │   Venturi    │   UVC    │  ← Middle section
│ Compt. │ + Contact    │ Chamber  │
│ (foam  │   Column     │ (sealed) │
│  lined)│  (clear PC)  │          │
├────────┴──────────────┴──────────┤
│  Intake    │           │ Exhaust │  ← Lower section
│  Filter    │    PCB    │ Filter  │
│  Access    │           │ Access  │
└────────────┴───────────┴─────────┘
      ↑ FRONT              BACK ↑
```

### 3.2 Vertical Section (Side View)

```
    ┌─────────────────────┐
    │   Water Fill Port   │ ← 400mm
    │  ┌─────────────┐    │
    │  │  Tank A/B    │   │ ← 250-350mm
    │  │  (water)     │   │
    │  └──────┬───────┘   │
    │  ┌──────┴───────┐   │
    │  │ Venturi +     │  │ ← 150-250mm
    │  │ Contact Zone  │  │
    │  │ + UVC Chamber │  │
    │  └──────┬───────┘   │
    │  ┌──────┴───────┐   │
    │  │  Pump + PCB   │  │ ← 0-150mm
    │  │  + Filters    │  │
    │  └───────────────┘  │
    └─────────────────────┘
```

## 4. Material Specification

### 4.1 Main Housing
- **Material**: ABS (Acrylonitrile Butadiene Styrene)
- **Grade**: General purpose, flame retardant (UL94 V-0)
- **Color**: Matte white (RAL 9003 or custom match)
- **Wall thickness**: 2.5mm minimum (3.0mm at stress points)
- **Surface finish**: SPI-C1 matte texture (hides fingerprints)

### 4.2 Water Tanks
- **Material**: Polypropylene (PP) or HDPE
- **Grade**: Food-contact safe (FDA 21 CFR 177.1520)
- **Color**: Translucent (for water level visibility) or opaque white
- **Wall thickness**: 1.5mm (blow molded)

### 4.3 Contact Column
- **Material**: Polycarbonate (PC) -- clear
- **Purpose**: Visual bubble observation (user engagement + QC)
- **Wall thickness**: 2.0mm
- **Diameter**: 50mm OD × 46mm ID
- **Height**: 150mm

### 4.4 UVC Chamber
- **Material**: Opaque PP (zero UV transmission at 275nm)
- **Wall thickness**: 3.0mm (extra for UV blocking)
- **Gaskets**: Silicone (Shore A 40), compression seal
- **Interlock**: Magnetic reed switch, magnet in lid

### 4.5 Filter Housings
- **Material**: ABS, matching main housing
- **Type**: Slide-in cartridge with snap-fit retention
- **Gasket**: Foam tape (closed-cell polyethylene) on edges

## 5. Assembly

### 5.1 Housing Assembly
- Top shell + bottom shell: 6× M3×12mm stainless screws
- Internal partitions: Snap-fit + 2× M3 screws each
- Tanks: Seated in molded cradles, secured by top shell pressure

### 5.2 Electronics
- PCB: 4× M3 standoffs, screwed to bottom shell
- Pump: 4× silicone grommets on threaded studs
- UVC chamber: 2× M3 screws to partition wall

### 5.3 Fluid Connections
- All tubing: Push-fit barbed connectors (10mm and 3mm)
- Tubing retention: Stainless hose clamps or zip ties
- Tank connections: Molded-in barbed ports (integral to tank)

## 6. Design for Manufacturing (DFM)

### 6.1 Injection Molding Considerations
- Draft angle: ≥1.5° on all vertical surfaces
- No undercuts without side actions
- Uniform wall thickness where possible (±20% variation max)
- Gate location: Hidden on bottom or internal surface
- Ejector pin marks: On internal or bottom surfaces only
- Weld lines: Away from visible surfaces and stress points

### 6.2 Mold Tooling Plan
| Part | Mold Type | Cavities | Est. Tool Cost |
|------|-----------|----------|---------------|
| Top shell | Steel P20 | 1 | $5,000-8,000 |
| Bottom shell | Steel P20 | 1 | $4,000-7,000 |
| Tank A | HDPE blow mold | 1 | $2,000-4,000 |
| Tank B | HDPE blow mold | 1 | $2,000-4,000 |
| UVC chamber | Aluminum (soft) | 1 | $1,000-2,000 |
| Filter housing (×2) | Aluminum (soft) | 2 | $1,500-3,000 |
| Venturi nozzle | Aluminum (soft) | 2 | $800-1,500 |
| **Total tooling** | | | **$16,300-29,500** |

## 7. IP Rating Target

- **IPX1** minimum for the complete product (protection against dripping water)
- **IP67** for UVC chamber (fully sealed against water ingress)
- **IP54** for electronics compartment (dust and splash protection)

## 8. Thermal Considerations

- UVC LEDs generate ~20W heat → water-cooled, no concern
- Pump motor: ~5W heat → enclosed compartment with passive ventilation
- PCB: <2W heat → no active cooling needed
- Worst case ambient: 40°C → all components rated to 60°C+ minimum
