# Venturi Subsystem Design Specification

## 1. Overview

The venturi nozzle is the core innovation of PetFilter. It uses the Bernoulli principle to entrain contaminated air into a high-velocity water stream, creating fine bubbles with high surface area for gas-liquid mass transfer.

---

## 2. Operating Principle

### 2.1 Bernoulli's Equation
P1 + ½ρv1² + ρgh1 = P2 + ½ρv2² + ρgh2

At the venturi throat:
- Cross-sectional area decreases
- Water velocity increases
- Static pressure decreases below atmospheric
- Atmospheric air is drawn in through a suction port at the throat

### 2.2 Air Entrainment
The pressure differential at the throat creates suction on the air inlet port. Air enters the water stream and is sheared into bubbles by the turbulent flow. Bubble size depends on:
- Water velocity at throat
- Air inlet port geometry
- Surface tension of water
- Turbulence intensity

---

## 3. Design Parameters

### 3.1 Venturi Geometry

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Inlet diameter (D1) | 10 mm | Matches standard 10mm ID silicone tubing |
| Throat diameter (D2) | 3.0 mm | Optimized for 1 L/min flow (validated by calculation) |
| Outlet diameter (D3) | 10 mm | Matches outlet tubing |
| Convergence angle | 21° (half-angle) | Standard for minimal separation loss |
| Divergence angle | 7° (half-angle) | Slow expansion recovers pressure, reduces cavitation |
| Throat length | 6 mm (2 × D2) | Standard throat length ratio |
| Air inlet port diameter | 2.0 mm | Sized for target air entrainment |
| Air inlet port location | At throat, perpendicular | Maximum vacuum point |
| Overall length | ~80 mm | Compact enough for housing |

### 3.2 Flow Conditions

| Parameter | Value | Calculation |
|-----------|-------|-------------|
| Water flow rate | 1.0 L/min (16.7 mL/s) | Peristaltic pump setting |
| Throat velocity | 2.36 m/s | Q/A = 16.7e-6 / 7.07e-6 |
| Throat area | 7.07 mm² | π(1.5)² |
| Inlet velocity | 0.21 m/s | Q/A = 16.7e-6 / 78.5e-6 |
| Pressure drop at throat | ~2.7 kPa | ½ρ(v2² - v1²) |
| Air entrainment ratio | 0.10-0.25 | Typical for this geometry |
| Air flow rate | 100-250 mL/min | Water flow × entrainment ratio |

### 3.3 Bubble Characteristics

| Parameter | Target | Notes |
|-----------|--------|-------|
| Bubble diameter | 0.5-3.0 mm | Smaller = higher surface area |
| Specific surface area | 2000-6000 m²/m³ | For 1-2mm bubbles |
| Rise velocity | 0.1-0.3 m/s | Stokes law for small bubbles |
| Contact time | 0.3-1.0 s | In 10-15cm contact column |

---

## 4. Material Selection

### 4.1 Prototype (POC)
- **Material**: PETG or ABS (3D printed, FDM)
- **Surface finish**: Smoothed with acetone vapor (ABS) or sanded (PETG)
- **Tolerance**: ±0.2mm (adequate for proof of concept)
- **Cost**: ~$2-5 per unit (material + print time)

### 4.2 Production
- **Material**: Polypropylene (PP) or PETG (injection molded)
- **Alternative**: 316 Stainless Steel (CNC machined) for premium durability
- **Tolerance**: ±0.05mm (injection molding standard)
- **Surface finish**: Smooth interior bore critical for flow consistency
- **Cost**: $3-8 per unit at 500+ (injection molded)

### 4.3 Seals and Connections
- **O-rings**: Silicone, food-grade, at inlet/outlet connections
- **Tubing interface**: Barbed fittings for 10mm ID silicone tubing
- **Air inlet**: Barbed fitting for 3mm ID silicone tubing (to filter)

---

## 5. Performance Optimization

### 5.1 Increasing Air Entrainment
- Decrease throat diameter (increases velocity, increases vacuum)
- Increase water flow rate (more energy available)
- Add secondary air injection ports (multiple small holes)
- Trade-off: more vacuum = more pump power required

### 5.2 Decreasing Bubble Size
- Add a diffuser plate downstream of throat (breaks bubbles)
- Use sintered metal or porous ceramic at air injection point
- Increase shear at mixing zone with geometric features
- Target: majority of bubbles <2mm

### 5.3 Cavitation Prevention
- Keep throat pressure above vapor pressure of water (2.3 kPa at 20°C)
- Slow divergence angle (7° half-angle) reduces cavitation risk
- Monitor for audible cavitation during testing (distinct high-pitched noise)
- If cavitation occurs: increase throat diameter or decrease flow rate

---

## 6. Integration Points

### 6.1 Water Path
- Inlet: Connected to peristaltic pump outlet via 10mm silicone tubing
- Outlet: Discharges into air-water contact zone (open tank or column)
- Flow direction: Horizontal or downward (gravity assists bubble contact)

### 6.2 Air Path
- Air inlet port: Connected to HEPA+carbon pre-filter via 3mm tubing
- The venturi's vacuum provides all suction -- no separate fan needed
- Pre-filter restricts air flow slightly, ensuring laminar entry

### 6.3 Contact Zone
- Venturi discharges into a clear acrylic/polycarbonate column (prototype)
- Column height: 10-15 cm
- Column diameter: 40-60 mm
- Water level maintained by overflow to Tank B
- Bubbles rise through standing water, maximizing contact time

---

## 7. Testing Protocol

### Test 1: Flow Characterization
- Measure water flow rate vs. pump speed (RPM)
- Measure air entrainment rate at each flow setting
- Map the operating envelope

### Test 2: Bubble Visualization
- Clear tube contact zone, backlit
- High-speed camera or smartphone slow-motion (240fps)
- Measure bubble diameter distribution

### Test 3: Pressure Mapping
- Pressure transducers at inlet, throat, and outlet
- Validate Bernoulli predictions
- Identify cavitation threshold

### Test 4: Ammonia Absorption
- Sealed test chamber with known NH3 concentration
- Run scrubber for 30 minutes
- Log MQ-137 sensor readings at 1-second intervals
- Calculate removal efficiency per pass and cumulative

---

## 8. 3D Print Files (POC)

The venturi will be designed in FreeCAD and exported as STL for 3D printing:
- `hardware/cad/venturi_v1.FCStd` (FreeCAD source)
- `hardware/cad/venturi_v1.stl` (print file)
- `hardware/cad/venturi_v1.step` (interchange format)

Print settings (PETG):
- Layer height: 0.12mm (fine detail at throat)
- Infill: 100% (must be watertight)
- Walls: 4+ perimeters
- Post-process: Sand interior bore smooth
