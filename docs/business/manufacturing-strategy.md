# Manufacturing Strategy: Solo Founder Approach

## 1. Philosophy

As a solo founder, manufacturing strategy must prioritize:
1. **Capital efficiency**: Minimize upfront investment at each stage
2. **Risk reduction**: Validate before committing to tooling
3. **Speed to market**: 12-18 months from concept to first customer shipment
4. **Quality**: Must be certifiable (UL, FCC) from production V1

---

## 2. Four-Stage Manufacturing Plan

### Stage 1: Proof of Concept (Months 0-3, Budget: $1,500)

**Goal**: Validate core physics -- venturi air entrainment and ammonia absorption

**Build approach**:
- 3D printed venturi nozzle (PETG, FDM printer)
- Off-the-shelf peristaltic pump (Adafruit or Kamoer)
- Clear acrylic tube for contact column (visual bubble observation)
- Plastic food containers as water tanks
- Breadboard electronics (ESP32 dev board + MQ-137 sensor)
- No housing -- open bench prototype

**Quantity**: 1-3 units

**Success criteria**:
- >80% ammonia reduction in sealed test chamber
- Visible fine bubble formation (<3mm)
- System runs for 4+ hours without issue
- Power draw <70W measured

**Key purchases**:
| Item | Cost |
|------|------|
| 3D printing filament (PETG) | $30 |
| Peristaltic pump (12V) | $25-50 |
| ESP32-S3 dev board | $15 |
| MQ-137 NH3 sensor | $8 |
| Flow sensor (YF-S201) | $5 |
| UVC LED module (single) | $45 |
| Acrylic tube, fittings, tubing | $40 |
| Plastic containers (tanks) | $10 |
| Miscellaneous (wires, PSU, etc.) | $50 |
| **Total** | **$230-260** |

### Stage 2: Engineering Prototype (Months 3-6, Budget: $6,000)

**Goal**: Integrated prototype with custom PCB, refined venturi, and preliminary housing

**Build approach**:
- Custom PCB designed in KiCad, fabricated by JLCPCB (5-10 boards with SMT assembly)
- Iterated 3D printed venturi (optimized throat geometry)
- 3D printed or CNC machined housing (functional, not aesthetic)
- Integrated UVC chamber with safety interlock
- Firmware MVP: state machine, sensor reading, pump control

**Quantity**: 5-10 units

**Success criteria**:
- Repeatable >80% ammonia reduction across 5+ units
- Noise <50 dB(A) at 1m
- 24+ hour continuous operation without failure
- No water leaks
- UVC fully contained (measured zero UV leakage)

**Key purchases**:
| Item | Cost |
|------|------|
| PCB fabrication + assembly (10 boards) | $200-400 |
| Components (10 sets of electronics) | $350-500 |
| Pumps (10 units) | $250-500 |
| UVC LEDs (50 units) | $500-800 |
| 3D printing / CNC housing (5-10) | $500-1,000 |
| Test equipment (multimeter, scope if needed) | $200-500 |
| Certification pre-scan (FCC pre-compliance) | $500-1,000 |
| **Total** | **$2,500-4,700** |

### Stage 3: Pre-Production (Months 6-12, Budget: $25,000)

**Goal**: Small batch production-representative units for beta testing and Kickstarter campaign

**Build approach**:
- Soft tooling injection molds (silicone RTV or aluminum) for housing
- Contract PCB assembly (JLCPCB or Seeed Studio Fusion)
- Manual final assembly (self or local contract assembler)
- Full certification testing (FCC, UL)

**Quantity**: 50-100 units

**Key purchases**:
| Item | Cost |
|------|------|
| Soft tooling (housing, 2 molds) | $3,000-6,000 |
| Soft tooling (tanks, venturi, filter housings) | $2,000-4,000 |
| PCB assembly (100 boards) | $1,500-2,500 |
| Components (100 sets) | $5,000-8,000 |
| FCC Part 15 testing | $3,000-5,000 |
| UL/CSA safety testing | $5,000-10,000 |
| Patent filing (provisional → full) | $2,000-5,000 |
| Beta test logistics | $1,000-2,000 |
| **Total** | **$22,500-42,500** |

### Stage 4: Production (Months 12-18, Budget: $75,000)

**Goal**: First 500-1000 unit production run for Kickstarter fulfillment + initial sales

**Build approach**:
- Hard tooling injection molds (steel, multi-cavity)
- Contract manufacturer for full assembly + test
- Amazon FBA or ShipBob for fulfillment
- DTC website (Shopify)

**Key purchases**:
| Item | Cost |
|------|------|
| Hard tooling (all molds) | $15,000-25,000 |
| Components (500 units) | $25,000-35,000 |
| Contract assembly (500 units) | $5,000-10,000 |
| Initial inventory (packaging, filters) | $5,000-8,000 |
| Fulfillment setup (FBA/ShipBob) | $2,000-3,000 |
| Marketing + Kickstarter fees | $5,000-10,000 |
| Working capital reserve | $10,000-15,000 |
| **Total** | **$67,000-106,000** |

---

## 3. Contract Manufacturer Selection

### 3.1 PCB Assembly

**Prototype (Stages 1-2)**: JLCPCB
- Why: Cheapest, fastest (5-7 day turnaround including assembly)
- MOQ: 5 boards
- SMT assembly: $0.0017/joint + setup fee
- Limitation: Manual QC, limited testing options

**Production (Stages 3-4)**: MacroFab or Seeed Studio Fusion
- Why: US-based (MacroFab) or established Chinese (Seeed), better QC
- MOQ: 50-100 boards
- Services: DFM review, AOI inspection, functional testing
- Price: $15-25 per board (assembled, at 500 qty)

### 3.2 Injection Molding

**Soft tooling (Stage 3)**: Xometry or Protolabs
- Aluminum molds, 100-500 shot life
- Lead time: 2-4 weeks
- Cost: $1,500-3,000 per mold
- Use for: Beta units, Kickstarter early production

**Hard tooling (Stage 4)**: Protolabs (US) or Shenzhen mold shop (overseas)
- Steel molds, 100,000+ shot life
- Lead time: 4-8 weeks
- Cost: $3,000-8,000 per mold (domestic), $1,500-4,000 (China)
- Use for: Full production

### 3.3 Final Assembly

**Stages 1-3**: Self-assembly
- Build assembly fixtures (jigs, test rigs)
- Document assembly process (work instructions with photos)
- Target: 30-60 minutes per unit

**Stage 4**: Local contract assembler or CM
- Provide work instructions, test procedure, quality checklist
- Target: 15-20 minutes per unit (production optimized)
- Cost: $8-12 per unit assembly labor

---

## 4. Regulatory Certification Roadmap

### 4.1 Required Certifications

| Certification | Scope | Timeline | Cost | Priority |
|--------------|-------|----------|------|----------|
| FCC Part 15 | Unintentional radiator (ESP32 WiFi) | 2-4 weeks | $3,000-5,000 | Required for US sale |
| UL/CSA | Electrical safety (60950 or 62368) | 4-8 weeks | $5,000-10,000 | Required for retail |
| CE marking | EU compliance (EMC + LVD) | 4-6 weeks | $3,000-6,000 | Required for EU |
| RoHS | Hazardous substances compliance | Self-declaration | $500-1,000 (testing) | Required |
| IEC 62471 | Photobiological safety (UVC) | Part of UL testing | Included | Required |

### 4.2 Certification Timeline

```
Month 6:  FCC pre-compliance scan (identify issues early)
Month 8:  Submit for FCC Part 15 testing
Month 9:  Submit for UL safety testing
Month 10: FCC approval expected
Month 11: UL listing expected
Month 12: CE testing (if targeting EU)
```

### 4.3 Design-for-Certification Tips
- Use a pre-certified ESP32 module (already has FCC modular approval)
- This significantly reduces FCC testing scope and cost
- External PSU with existing UL listing reduces product safety scope
- Fully enclosed UVC simplifies photobiological safety

---

## 5. Supply Chain Strategy

### 5.1 Critical Components
- **UVC LEDs**: Limited suppliers (Seoul Viosys, Crystal IS, Nichia). Maintain 3-month safety stock.
- **ESP32-S3**: Broad availability (Espressif, LCSC). Low risk.
- **Peristaltic pump**: Single supplier (Kamoer). Evaluate backup (Boxer, Williamson).

### 5.2 Dual-Sourcing Plan
| Component | Primary | Backup |
|-----------|---------|--------|
| UVC LEDs | Seoul Viosys | Generic 275nm (verified) |
| Pump | Kamoer KCM | Boxer 9QQ |
| ESP32 | LCSC | Mouser / Digikey |
| HEPA media | Chinese supplier | US alternative (Lydall) |
| Carbon | Chinese bulk | Jacobi Carbons |

---

## 6. Quality Assurance

### 6.1 Incoming Quality
- UVC LED optical power verification (sampling)
- Pump flow rate test (100% tested)
- PCB visual inspection + electrical test (100%)

### 6.2 Final Assembly QC
- Water leak test (pressurize system, check seals)
- Electrical safety (hipot test, ground continuity)
- Functional test (pump runs, UVC activates, sensors read)
- Noise measurement (spot check, 10% sampling)

### 6.3 Burn-In
- 4-hour burn-in at elevated temperature (35°C)
- Continuous pump + UVC operation
- Log sensor readings for anomaly detection
