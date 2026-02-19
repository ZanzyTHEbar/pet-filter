# Filtration Subsystem Design Specification

## 1. Overview

PetFilter uses a dual-stage dry filtration system on the air path (intake and exhaust) to complement the water scrubbing. HEPA captures particulates (dander, dust, litter particles), and activated carbon adsorbs non-water-soluble VOCs.

---

## 2. Intake Filter (Pre-Scrub)

### 2.1 Configuration
Air passes through the intake filter BEFORE entering the venturi:
```
Room air → HEPA element → Carbon element → Venturi air inlet
```

### 2.2 HEPA Element
- **Grade**: H13 (True HEPA, 99.97% at 0.3μm)
- **Media**: Glass fiber or synthetic (polypropylene)
- **Size**: 80mm × 80mm × 20mm (compact panel)
- **Airflow**: Rated for 0.1-0.5 L/min (extremely low for HEPA, minimal pressure drop)
- **Life**: 2-3 months (based on residential dust/dander loading)
- **Captures**: Cat dander, litter dust, hair, pollen, mold spores

### 2.3 Carbon Element
- **Type**: Granular activated carbon (coconut shell) in mesh pouch
- **Mass**: 50-100g
- **Mesh size**: 12×40 (standard granular)
- **Bed depth**: 10-15mm
- **Adsorption targets**: Skatole, indole, p-cresol, other VOCs
- **Life**: 2-3 months (ammonia exhausts carbon faster -- but ammonia handled by water)
- **Note**: Because water scrubbing handles ammonia, the carbon filter lasts significantly longer than in a conventional air purifier

---

## 3. Exhaust Filter (Post-Scrub)

### 3.1 Configuration
Air exits the water scrubbing zone through exhaust filter:
```
Air-water contact zone → Mist eliminator → Carbon element → Exhaust port
```

### 3.2 Mist Eliminator
- **Purpose**: Prevent water droplets from reaching the carbon filter
- **Type**: Polypropylene mesh pad or foam
- **Thickness**: 10-20mm
- **Efficiency**: >99% for droplets >10μm

### 3.3 Exhaust Carbon Element
- **Purpose**: Final polishing -- catches any VOCs not absorbed by water
- **Same spec as intake carbon**: 50-100g granular activated carbon
- **Life**: 3-4 months (lighter loading since water handled most compounds)

---

## 4. Filter Housing Design

### 4.1 Intake Filter Assembly
- Slide-in cartridge design (tool-free replacement)
- HEPA + carbon combined in single replaceable unit
- Orientation keyed (can only insert one way)
- Gasket seal on all edges (prevents bypass)
- Access door on housing exterior

### 4.2 Exhaust Filter Assembly
- Similar slide-in cartridge
- Mist eliminator: permanent (washable), not replaced
- Carbon element: replaceable cartridge
- Located between water zone and exhaust port

---

## 5. Filter Replacement Cartridge (Consumable)

### 5.1 Intake Cartridge
- Contains: HEPA element + carbon pouch
- Replacement interval: Every 2-3 months
- Retail price target: $15-20
- COGS: $3-5

### 5.2 Exhaust Cartridge
- Contains: Carbon pouch only
- Replacement interval: Every 3-4 months
- Retail price target: $10-15
- COGS: $2-3

### 5.3 Subscription Model
- **Basic**: Intake + Exhaust shipped every 3 months = $25-35/shipment
- **Annual**: 4 sets/year = $80-120/year
- **Monthly subscription**: $9.99/month (covers both + water treatment tablets)

---

## 6. Performance Specifications

### 6.1 Air Resistance
At 0.3 L/min (max air entrainment rate through venturi):

| Component | Pressure Drop |
|-----------|--------------|
| HEPA element | 5-15 Pa |
| Carbon element (intake) | 2-5 Pa |
| Carbon element (exhaust) | 2-5 Pa |
| Mist eliminator | 1-3 Pa |
| **Total filter resistance** | **10-28 Pa** |

This is negligible -- the venturi generates 2,700+ Pa vacuum. Filters will not restrict airflow.

### 6.2 Capture Efficiency (Combined System)

| Compound Class | Water Scrub | Intake Carbon | Exhaust Carbon | Total |
|---------------|-------------|---------------|----------------|-------|
| Ammonia (NH3) | 85-95% | 5-10% | 2-5% | **92-99%** |
| Mercaptans | 50-70% | 15-25% | 5-10% | **70-95%** |
| H2S | 60-80% | 10-15% | 5-10% | **75-95%** |
| Skatole/Indole | 70-85% | 10-15% | 5-10% | **85-98%** |
| Particulates (dander) | 0% | 99.97% (HEPA) | N/A | **99.97%** |

---

## 7. Cost Analysis

### 7.1 Initial (Included with Device)
| Component | Cost |
|-----------|------|
| Intake HEPA element | $1.50-2.50 |
| Intake carbon (100g) | $1.00-1.50 |
| Exhaust carbon (100g) | $1.00-1.50 |
| Mist eliminator (permanent) | $0.50-1.00 |
| Filter housings (2, molded) | $2.00-4.00 |
| **Total (initial set)** | **$6-10.50** |

### 7.2 Replacement Cartridge Margin
| Item | COGS | Retail | Margin |
|------|------|--------|--------|
| Intake cartridge | $3-5 | $15-20 | 70-75% |
| Exhaust cartridge | $2-3 | $10-15 | 75-80% |
| Combo pack | $5-8 | $22-30 | 68-73% |
