# Noise Analysis & Acoustic Design

## 1. Target Specification

| Parameter | Requirement | Rationale |
|-----------|------------|-----------|
| Noise at 1m | <45 dB(A) | Quieter than refrigerator (40-50 dB) |
| Noise at 3m | <35 dB(A) | Imperceptible in living room background |
| No tonal components | >5 dB margin vs broadband | Tonal noise is perceived as more annoying |

---

## 2. Noise Sources

### 2.1 Peristaltic Pump (Dominant Source)

**Mechanism**: Rollers compressing tubing create periodic pressure pulses and mechanical vibration.

| Parameter | Unmitigated | With Isolation |
|-----------|-------------|---------------|
| Level at 1m | 45-55 dB(A) | 35-45 dB(A) |
| Frequency | 50-200 Hz (fundamental) | Same, reduced amplitude |
| Character | Rhythmic pulsing | Dampened hum |

**Mitigation strategies**:
1. **Vibration isolation**: Mount pump on silicone rubber grommets (10-15 dB reduction)
2. **Mass loading**: Heavy base plate decouples pump from housing
3. **Acoustic enclosure**: Foam-lined compartment around pump
4. **Lower RPM**: Larger tubing ID at lower speed = same flow, less noise
5. **Stepper motor**: Smoother than DC brushed (Kamoer KCM-ODM uses stepper)

### 2.2 Water Flow Noise

| Source | Level at 1m | Notes |
|--------|-------------|-------|
| Venturi discharge into water | 25-35 dB(A) | Splashing at entry point |
| Bubble column | 20-30 dB(A) | Gentle bubbling sound |
| Tubing flow | 15-25 dB(A) | Negligible if laminar |

**Mitigation**: Submerge venturi outlet below water surface (eliminates splash). Longer diffuser reduces turbulence at exit.

### 2.3 Air Flow

| Source | Level at 1m | Notes |
|--------|-------------|-------|
| Air through HEPA filter | 15-20 dB(A) | Very low flow rate (0.1-0.3 L/min) |
| Air through carbon filter | 10-15 dB(A) | Even lower restriction |
| Venturi air inlet | 20-25 dB(A) | Slight whistle possible |

**Mitigation**: Air inlet whistle eliminated by proper port geometry (chamfered entry, no sharp edges).

### 2.4 Electronics
- ESP32: Silent
- UVC LEDs: Silent
- Buck converters: Possible coil whine at <20 dB(A), inaudible

---

## 3. Combined Noise Estimate

Using logarithmic addition: L_total = 10 × log10(Σ 10^(Li/10))

| Configuration | Pump | Water | Air | Total |
|--------------|------|-------|-----|-------|
| Unmitigated | 50 dB | 30 dB | 22 dB | **50.0 dB(A)** |
| With isolation | 40 dB | 25 dB | 20 dB | **40.1 dB(A)** |
| Fully optimized | 35 dB | 22 dB | 18 dB | **35.3 dB(A)** |

**Assessment**: With proper vibration isolation and acoustic treatment, the 45 dB(A) target is achievable. The fully optimized version could approach 35 dB(A).

---

## 4. Acoustic Design Measures

### 4.1 Pump Compartment
- Separate enclosed compartment within housing
- Lined with 10mm acoustic foam (melamine or polyurethane)
- Pump mounted on 4× silicone vibration isolators (Shore A 30-40)
- Compartment walls: 3mm ABS minimum (mass barrier)

### 4.2 Housing Design
- No direct line-of-sight from pump to exterior openings
- Air intake/exhaust ports: baffled (serpentine path reduces transmitted noise)
- Rubber feet on housing base (decouples from furniture/floor)
- Internal partitions separate pump compartment from water/air paths

### 4.3 Water Path
- Venturi outlet submerged 2-3cm below water surface
- No free-falling water (eliminates splashing)
- Smooth bore tubing (reduces turbulence noise)
- Gradual bends, no sharp 90° elbows

---

## 5. Noise Measurement Protocol

### Equipment
- Sound level meter (Class 2 minimum): e.g., NIOSH SLM app (free, validated) or UNI-T UT352
- Calibrated at 94 dB(A) reference

### Test Conditions
- Background noise: <30 dB(A) (quiet room, HVAC off)
- Distance: 1.0m from device center, at ear height
- Orientation: 4 measurements (front, back, left, right), averaged

### Test Protocol
1. Measure background noise (device off): must be <30 dB(A)
2. Measure standby mode: expect 0 dB above background
3. Measure active mode: target <45 dB(A) at 1m
4. Measure frequency spectrum: check for tonal peaks
5. Subjective assessment: 3+ listeners rate on 1-5 annoyance scale
