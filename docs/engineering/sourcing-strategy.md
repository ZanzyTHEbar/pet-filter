# Component Sourcing Strategy: Cross-Industry Ecosystem Approach

## Executive Summary

The fastest way to kill a hardware startup is custom-everything. The fastest way to ship one is to realize that **your product is a novel *arrangement* of existing components, not a collection of novel components**. PetFilter shares DNA with at least six existing product ecosystems, each with mature supply chains, proven components, and competitive pricing at low MOQs. This document maps every PetFilter component to its optimal sourcing ecosystem and lays out the methodology for executing procurement.

---

## 1. The Core Insight: Ecosystem Parasitism

Every component in PetFilter already exists -- in a different product. The game isn't inventing parts, it's identifying which industry already mass-produces the exact thing you need, then buying from their supply chain at their volume pricing.

```
PetFilter Component          Donor Ecosystem             Why It's Cheaper There
─────────────────────        ───────────────             ─────────────────────
Peristaltic pump             Medical / Lab / Aquarium    Millions produced annually
Venturi nozzle               Pool/Spa ozone injection    Mazzei makes exactly this
Water tanks (2-3L)           Coffee machines             Identical volume, food-grade PP
Silicone tubing              Medical / Coffee / Lab      Commodity, pennies per meter
Flow sensor                  Coffee / HVAC               Standard hall-effect pulse
Water level sensor           Coffee / Humidifier         Capacitive touch, commodity
UVC LED module               Water purifier industry     Pre-built, certified, $30-45
HEPA filter element          Air purifier OEMs           Cut-to-size from roll stock
Activated carbon             Aquarium / Water filter     Bulk commodity
Power supply (12V 60W)       Laptop / LED lighting       Billions produced annually
ESP32 MCU                    IoT ecosystem               $3 at volume
Barbed fittings              Aquarium / Irrigation       Cents per piece
Housing                      Small appliance (generic)   Standard injection mold shop
```

The only genuinely custom part is the *arrangement* -- how these components connect. And even the arrangement borrows from aquarium protein skimmers.

---

## 2. Donor Ecosystem Deep-Dive

### 2.1 Coffee Machines (Highest Component Overlap)

Coffee machines are the richest donor ecosystem for PetFilter. A typical espresso machine contains:

| Coffee Machine Component | PetFilter Equivalent | Reusability |
|-------------------------|---------------------|-------------|
| Water tank (1-2L, PP) | Water tank (2.5L, PP) | Direct (larger size) |
| Vibratory pump (Ulka/CEME) | Not ideal (too high pressure) | Partial |
| Solenoid valve | Not needed | -- |
| Food-grade silicone tubing | Silicone tubing | Direct |
| Quick-connect fittings | Barbed fittings | Direct |
| Flow meter (hall-effect) | Flow sensor | Direct |
| Water level sensor | Water level sensor | Direct |
| NTC thermistor | Temperature sensor | Direct |
| 12V/24V DC power supply | Power supply | Direct |
| Drip tray (PP) | Tank B / collection | Adaptable |
| Control PCB (MCU-based) | ESP32 main board | Architecture similar |

**Why coffee machines?**
- The coffee machine supply chain in Shenzhen/Ningbo is **enormous** (China produces ~80% of the world's consumer coffee machines)
- Food-grade PP water tanks at 1-3L are a commodity: $1.50-4.00 per unit at 500+
- Tubing, fittings, and sensors are interchangeable
- Alibaba has 9,800+ coffee pump listings alone

**What doesn't transfer:**
- Coffee machines use high-pressure vibratory pumps (15 bar for espresso). PetFilter needs low-pressure peristaltic flow. Different pump category entirely.
- Heating elements are irrelevant
- Grinder mechanics are irrelevant

**Sourcing action:** Contact 3-5 coffee machine OEM component suppliers on Alibaba for water tanks, fittings, tubing, and flow sensors. Request samples at coffee-machine volume pricing.

### 2.2 Aquarium Equipment (Closest Technical Analogue)

This is the most important ecosystem. **Aquarium protein skimmers are the direct technical ancestor of PetFilter.**

A protein skimmer works like this:
1. A needle-wheel pump or venturi draws air into water
2. Fine bubbles create a foam that captures dissolved organics
3. Foam rises into a collection cup

PetFilter works identically -- except we're capturing airborne gases *into* the water rather than pulling dissolved organics *out* of it.

| Aquarium Component | PetFilter Equivalent | Reusability |
|-------------------|---------------------|-------------|
| Venturi injector | Venturi nozzle | Near-direct (resize throat) |
| Needle-wheel pump | Alternative to peristaltic | Evaluate (possibly better!) |
| Reaction column (acrylic tube) | Contact column | Direct |
| Collection cup | Tank B | Adaptable |
| Air silencer | Intake filter housing | Adaptable |
| Bubble diffuser plate | Bubble optimization | Direct |

**Critical insight: Needle-wheel pumps**

Aquarium protein skimmer pumps (e.g., Marine Sources SP3, Sicce SK-400, Sedra KSP series) are purpose-built to entrain air into water via venturi/needle-wheel and produce fine bubbles. They are:
- 15-50W power
- 200-1000 L/hr water flow
- Specifically designed for air-water mixing
- $15-40 per unit
- Available from multiple OEMs in Guangdong

**This might be a better choice than a peristaltic pump + separate venturi.** A needle-wheel pump combines the pump AND the venturi into a single unit. Trade-off: it's submersible (sits in the water tank) vs. external peristaltic.

**Sourcing action:** Order 3-5 different needle-wheel skimmer pumps for POC testing. Compare air entrainment, bubble size, noise, and power consumption against peristaltic + venturi approach.

### 2.3 Water Purification Industry (UVC Modules)

The point-of-use water purification market is exploding, and Shenzhen is the global hub. Pre-built UVC LED modules exist that are:
- 275nm wavelength
- 12-36V DC input
- 2-15 L/min flow rate
- 99.99%+ sterilization rate
- Integrated flow sensors
- IP67 sealed
- $30-45 per unit at 100+

**Key manufacturers:**
- **Shenzhen Hechuang Hitech** (HC-Hitech): 15 LPM modules, 8-year lifespan, stainless steel
- **Shenzhen Leader UV Technology**: 16+ years, ISO 9001, accepts small batch OEM
- **Shenzhen HCEN Technology**: 200+ employees, UV LED packaging specialists
- **WM aquatec** (Germany): Premium, 113×159×105mm, 14W, used in RV/marine

**Why this matters:** Rather than designing a custom UVC chamber with individual LEDs, heatsinks, drivers, and seals (15+ individual parts), we can buy a pre-certified, pre-sealed, pre-tested module for $30-45. This:
- Eliminates UVC safety design risk
- Reduces assembly complexity
- May already have certifications we need (IEC 62471)
- Comes with thermal management built in

**Sourcing action:** Contact HC-Hitech and Leader UV for sample modules. Specify: 275nm, 1-2 L/min flow rate, 12V DC input, inline configuration. Request datasheets and MOQ pricing.

### 2.4 Pool & Spa Ozone Treatment (Venturi Injectors)

The pool/spa industry uses venturi injectors to dissolve ozone gas into water -- mechanically identical to what PetFilter does with room air.

**Mazzei Injector Corporation** is the gold standard:
- Model 684: 3/4" barb, 1/4" suction port, PVDF material
- Retail: $59-67 per unit
- Designed for exactly this: entraining gas into flowing water
- Internal mixing vanes create microbubbles
- No moving parts, no electricity, runs dry without damage
- Flow rates: 2.2-9.4 GPM (8-36 L/min) -- oversized for us, but smaller models exist

**Problem:** Mazzei injectors are designed for much higher flow rates (pool pumps) than PetFilter needs (1 L/min). Their smallest unit handles 2.2 GPM minimum.

**Alternative:** Smaller venturi injectors exist for aquarium ozone and hydroponics:
- 1/2" and 3/8" venturi injectors on Alibaba: $0.50-3.00 per unit
- Designed for 1-10 L/min flow rates
- PVC or PP material
- Perfect size range for PetFilter

**Sourcing action:** Order an assortment of 1/2" and 3/8" venturi injectors from Alibaba ($20-30 for 10 units). Test air entrainment at 1 L/min water flow. Compare against 3D-printed custom design.

### 2.5 Medical / Laboratory (Peristaltic Pumps)

If we stick with peristaltic pumps (rather than needle-wheel), the medical/lab ecosystem is the source:

**Kamoer Fluid Tech (Shanghai):**
- 14,428 m² facility, 15 patents, ISO 9001
- Accepts MOQ of 1 unit on Alibaba
- KCM-ODM stepper peristaltic: $15-25 per unit
- KAS series: Smaller, cheaper ($4-10)
- Food-grade BPT and Pharmed tubing available
- OEM customization for motor/tube/casing

**Longer Pump (Baoding):**
- Major Chinese peristaltic pump OEM
- BT100-2J and similar benchtop models
- OEM pump heads available separately: $8-15

**Adafruit (for prototyping):**
- 12V DC peristaltic: $25 retail
- 100 mL/min, self-priming
- Ideal for POC, not production

**Sourcing action:** Order Kamoer KAS (small) and KCM (medium) samples directly from Alibaba. Test noise, flow rate, and pressure generation.

### 2.6 HVAC / Air Purification (Filters)

HEPA and carbon filter elements are commodities in the air purification industry.

**HEPA filter sourcing:**
- **APC Filtration** (US): Custom OEM, MOQ ~100 units, 40+ years experience
- **Sidco Filter** (US): Custom OEM, MOQ 200+ for volume pricing, rapid prototyping
- **Cleanroom-FFU** (China): Custom sizes, 3D printing for prototypes, low MOQ
- **Roll stock approach**: Buy H13 HEPA media in sheet form ($15-30/m²), die-cut to size. 80×80mm element uses $0.10-0.20 of media.

**Activated carbon sourcing:**
- Aquarium carbon (coconut shell, 12×40 mesh): $5-10/kg bulk
- 200g per device = $1-2 in carbon material
- Pre-filled mesh pouches available from aquarium supply: $0.50-1.00 each at bulk

**Sourcing action:** Buy H13 HEPA sheet media and activated carbon in bulk. Fabricate filter elements in-house for POC and pre-production. Switch to OEM supplier (APC or Sidco) at 500+ units.

---

## 3. Sourcing Methodology: The Five-Layer Strategy

### Layer 1: Identify the Donor Ecosystem

For every component on the BOM, ask: **"What existing mass-market product already contains this exact part?"**

The mental model:

```
                    ┌─────────────────────────┐
                    │     YOUR PRODUCT        │
                    │   (novel arrangement)   │
                    └───────────┬─────────────┘
                                │
              ┌─────────────────┼──────────────────┐
              │                 │                   │
     ┌────────▼────────┐ ┌─────▼──────┐ ┌──────────▼────────┐
     │ Coffee Machine  │ │ Aquarium   │ │ Water Purifier    │
     │ Supply Chain    │ │ Supply     │ │ Supply Chain      │
     │                 │ │ Chain      │ │                   │
     │ Tanks, tubing,  │ │ Venturi,   │ │ UVC modules,      │
     │ fittings, flow  │ │ pumps,     │ │ flow sensors,     │
     │ sensors, PSU    │ │ columns,   │ │ inline chambers   │
     └─────────────────┘ │ carbon     │ └───────────────────┘
                         └────────────┘
```

### Layer 2: Source from the Ecosystem, Not the Component

Don't search Alibaba for "peristaltic pump." Search for "coffee machine OEM supplier" or "aquarium pump manufacturer" and ask what components they sell separately. OEM component suppliers think in *systems*, and they'll suggest parts you didn't know existed.

**Why this works:**
- You get ecosystem-optimized pricing (their margins are built for appliance volumes)
- You discover complementary parts (a pump supplier also sells fittings, tubing, tanks)
- You reduce supplier count (one relationship for 5+ components)
- You inherit their quality systems (food-grade, medical-grade already certified)

### Layer 3: The Prototype-to-Production Ladder

| Stage | Sourcing Approach | Supplier Type | MOQ |
|-------|------------------|---------------|-----|
| POC (1-3 units) | Off-the-shelf retail | Amazon, Adafruit, Aliexpress | 1 |
| Eng. Proto (5-10) | Small batch direct | Alibaba gold suppliers | 5-50 |
| Pre-Prod (50-100) | OEM sample orders | Alibaba verified, direct factories | 50-100 |
| Production (500+) | OEM production orders | Negotiated contracts, 2+ sources | 500+ |
| Scale (2000+) | Custom-spec OEM | Long-term agreements | 1000+ |

**Golden rule:** Never negotiate production pricing until you have a working prototype. You have zero leverage without a proven design.

### Layer 4: The Dual-Source Mandate

Every critical component MUST have two qualified sources. Not because of paranoia -- because single-source dependency kills hardware startups.

| Component | Primary Source | Backup Source | Switch Cost |
|-----------|---------------|---------------|-------------|
| Peristaltic pump | Kamoer (Shanghai) | Longer (Baoding) | Low (standard sizing) |
| UVC LED module | HC-Hitech (Shenzhen) | Leader UV (Shenzhen) | Low (standard interface) |
| ESP32-S3 | LCSC (Shenzhen) | Mouser (US) | Zero (identical part) |
| HEPA media | Chinese roll stock | APC Filtration (US) | Low (die-cut to spec) |
| Carbon | Aquarium bulk | Jacobi Carbons | Zero (commodity) |
| Water tanks | Coffee OEM (Ningbo) | Local thermoformer | Medium (retooling) |
| PSU (12V 60W) | Mean Well | Generic UL-listed | Low (standard spec) |

### Layer 5: The "Buy vs. Build vs. Borrow" Decision Matrix

For each component, evaluate three options:

| Decision | When to Use | Example |
|----------|-------------|---------|
| **Buy off-shelf** | Part exists exactly as needed | ESP32 module, PSU, fittings, tubing |
| **Buy OEM module** | Pre-integrated subsystem saves design time | UVC LED module (replaces 15+ discrete parts) |
| **Build custom** | No existing part fits, OR custom part is core IP | Venturi nozzle (if off-shelf doesn't perform) |
| **Borrow design** | Adapt an existing product's form factor | Coffee machine tank → PetFilter tank (resize) |

**Decision framework:**

```
Does an off-the-shelf part work?
├── YES → Buy it. Don't overthink this.
└── NO → Does an OEM module exist?
    ├── YES → Buy the module. You're not in the module business.
    └── NO → Is this part core IP?
        ├── YES → Build custom. Protect it.
        └── NO → Borrow from closest ecosystem. Adapt minimally.
```

---

## 4. Revised BOM with Ecosystem Sourcing

### 4.1 Option A: Peristaltic Pump Approach (Current Design)

| # | Component | Donor Ecosystem | Source | Unit Cost (500x) |
|---|-----------|----------------|--------|------------------|
| 1 | Peristaltic pump | Medical/Lab | Kamoer (Alibaba direct) | $12-15 |
| 2 | Venturi injector (1/2") | Pool/Hydro | Alibaba (generic) | $0.80-1.50 |
| 3 | Silicone tubing (10mm, 1.5m) | Coffee/Medical | Alibaba bulk | $0.60-1.00 |
| 4 | Water tank × 2 | Coffee machine | Ningbo OEM | $3.00-4.00 ea |
| 5 | UVC LED module (inline) | Water purifier | HC-Hitech / Leader UV | $28-35 |
| 6 | HEPA element (80×80mm) | Air purifier | Roll stock, die-cut | $0.80-1.50 |
| 7 | Activated carbon (200g) | Aquarium | Bulk commodity | $0.80-1.00 |
| 8 | Contact column (PC tube) | Aquarium | Standard acrylic/PC tube | $1.50-2.50 |
| 9 | ESP32-S3 + PCB (assembled) | IoT | JLCPCB / LCSC | $12-18 |
| 10 | Sensors (NH3+flow+level+temp) | Coffee/HVAC | Alibaba assorted | $6-9 |
| 11 | PSU (12V 60W, UL listed) | Laptop/LED | Standard commodity | $4-6 |
| 12 | Housing (injection molded) | Small appliance | Shenzhen mold shop | $8-12 |
| 13 | Fittings, O-rings, hardware | Aquarium/Coffee | Alibaba assorted | $2-4 |
| 14 | Filter housings (molded) | Air purifier | Included in housing mold | $1.50-2.50 |
| 15 | Packaging | Generic | Custom print | $1.50-2.50 |
| | **TOTAL BOM** | | | **$84-119** |

This is **30-40% cheaper** than the original BOM estimate ($127 at 500x) by:
- Using pre-built UVC module instead of discrete LEDs + heatsink + driver + chamber
- Using off-shelf venturi injector instead of custom machined
- Sourcing tanks from coffee machine OEM instead of custom blow mold
- Buying from ecosystem suppliers at their volume pricing

### 4.2 Option B: Needle-Wheel Pump Approach (EXPLORE THIS)

Replace the peristaltic pump + separate venturi with a single aquarium needle-wheel pump:

| Change | Peristaltic + Venturi | Needle-Wheel Pump |
|--------|----------------------|-------------------|
| Parts count | 2 (pump + venturi + tubing) | 1 (integrated) |
| Air entrainment | Moderate (0.1-0.3 ratio) | High (purpose-built) |
| Bubble quality | Depends on venturi geometry | Optimized by design |
| Noise | Pump pulsing + water flow | Continuous hum (potentially quieter) |
| Placement | External (pump outside tank) | Submersible (inside tank) |
| Cost | $13-17 combined | $12-20 |
| Maintenance | Tubing replacement (6-12mo) | Impeller cleaning (6-12mo) |
| Reversibility | Bidirectional pumping | Not reversible |

**This is worth serious POC evaluation.** The aquarium industry has spent decades optimizing needle-wheel pumps for exactly the air-water mixing task PetFilter needs.

---

## 5. Sourcing Execution Plan

### 5.1 Phase 1: Sample Procurement ($150-300, Week 1-2)

**Amazon/Adafruit (immediate, US shipping):**
- [ ] Adafruit peristaltic pump 12V ($25)
- [ ] ESP32-S3 dev board ($15)
- [ ] MQ-137 NH3 sensor ($8)
- [ ] YF-S201 flow sensor ($5)
- [ ] 12V 5A power supply ($12)
- [ ] Silicone tubing 10mm ID, 2m ($8)
- [ ] Clear acrylic tube 50mm OD × 300mm ($10)

**Alibaba (1-2 week shipping):**
- [ ] Venturi injectors 1/2" assortment (10 pcs) ($15-25)
- [ ] Kamoer KAS peristaltic pump sample ($15-25)
- [ ] Needle-wheel skimmer pump (Marine Sources SP3 or similar) ($20-30)
- [ ] Needle-wheel skimmer pump (alternate brand) ($20-30)
- [ ] Capacitive water level sensors (5 pcs) ($8)
- [ ] Barbed fittings assortment (20 pcs) ($5)

**Specialty (1-2 week shipping):**
- [ ] UVC LED module sample (HC-Hitech or Leader UV) ($35-50)
- [ ] HEPA H13 sheet media 300×300mm ($10)
- [ ] Activated carbon, coconut shell, 1kg ($8)
- [ ] Mist eliminator mesh, PP, 150×150mm ($5)

### 5.2 Phase 2: A/B Testing (Week 2-4)

Build TWO proof-of-concept configurations:

**Config A: Peristaltic + External Venturi**
```
Tank → Peristaltic pump → Venturi injector → Contact column → UVC module → Tank
                              ↑ air in
```

**Config B: Needle-Wheel Pump (Submersible)**
```
Tank (pump submerged) → Needle-wheel entrains air → Contact column → UVC module → Tank
                                    ↑ air in
```

Measure for each:
- Air entrainment rate (mL/min)
- Bubble size distribution (photograph through clear column)
- NH3 reduction (MQ-137 sensor, sealed chamber test)
- Noise (phone dB meter at 1m)
- Power consumption (kill-a-watt meter)

### 5.3 Phase 3: Supplier Qualification (Week 4-8)

Based on POC results, contact production suppliers:

1. **Request for Quotation (RFQ)** to 3+ suppliers per critical component
2. **Sample evaluation**: Order 5-10 units from top 2 suppliers
3. **Quality check**: Measure consistency across samples
4. **Negotiate**: Use competing quotes as leverage
5. **Select primary + backup** for each component

### 5.4 Phase 4: Supply Chain Consolidation (Week 8-12)

**Goal:** Minimize supplier count. Ideal target: 5-7 suppliers total.

| Supplier | Components | Why |
|----------|-----------|-----|
| Kamoer (or pump winner) | Pump + tubing + fittings | One relationship for fluid path |
| UVC module OEM | UVC module complete | Turnkey subsystem |
| JLCPCB | PCB + SMT assembly | Electronics in one shop |
| LCSC | All discrete components | One order, one shipment |
| Coffee tank OEM (Ningbo) | Water tanks + housing | Plastic parts consolidated |
| APC Filtration or DIY | HEPA + carbon elements | Filter media |
| Mean Well / PSU supplier | Power supply | Certified, commodity |

---

## 6. Cost Optimization Techniques

### 6.1 Module Substitution (Biggest Single Savings)

Replacing the discrete UVC subsystem with a pre-built module:

| Approach | Parts Count | Assembly Time | Cost (500x) | Risk |
|----------|------------|---------------|-------------|------|
| Discrete LEDs + custom chamber | 12-15 parts | 20 min | $50-77 | High (UVC safety design) |
| Pre-built OEM module | 1 part | 2 min (plug in) | $28-35 | Low (pre-certified) |
| **Savings** | | | **$22-42 per unit** | |

At 500 units, this single decision saves **$11,000-21,000**.

### 6.2 Ecosystem Pricing Arbitrage

The same silicone tubing costs:
- $8/meter on Amazon (retail, "food grade" branded)
- $3/meter on Adafruit (maker-priced)
- $0.30/meter on Alibaba (coffee machine OEM volume)
- $0.15/meter on Alibaba (medical tubing bulk, 1000m roll)

**Same tube.** The difference is which supply chain you're buying from.

### 6.3 Design-for-Existing-Parts

Instead of designing a custom tank and finding a manufacturer, start with the question: "What existing tank is close to what I need?"

Coffee machine water reservoirs:
- 1.5L, 2.0L, 2.5L are standard sizes
- PP, food-grade, with integrated handle and lid
- $2-4 per unit at OEM volume
- Already have filling port, already have drain port

**If you design PetFilter to use a 2.5L standard coffee tank**, you skip blow mold tooling entirely ($4,000-8,000 saved) and ship faster.

### 6.4 The 80/20 Rule for Custom Parts

Only 2-3 parts in PetFilter actually NEED to be custom:
1. **Main housing** (defines the brand/product identity)
2. **Venturi nozzle** (IF off-shelf injectors don't perform)
3. **PCB** (always custom for your specific circuit)

Everything else should be off-the-shelf, adapted, or sourced from an existing ecosystem.

---

## 7. Supplier Contact Templates

### 7.1 Alibaba Initial Contact

```
Subject: OEM Component Inquiry - Water Treatment Device (500-2000 units/year)

Hello,

I am developing a compact water-based air treatment device for the consumer
pet care market. We are seeking OEM component suppliers for production
volumes of 500-2000 units annually, scaling to 5000+.

We are interested in:
- [Specific component with specifications]
- Food-grade / medical-grade quality required
- 12V DC operation
- [Dimensions / flow rate / other specs]

Could you provide:
1. Product datasheet / specification sheet
2. MOQ and pricing at 100, 500, and 1000 units
3. Lead time for sample and production orders
4. OEM customization options (if any)
5. Certifications (CE, UL, FDA as applicable)

We would like to order 3-5 samples for evaluation first.

Thank you,
[Name]
[Company]
```

### 7.2 UVC Module Specific Inquiry

```
Subject: UVC LED Water Treatment Module - OEM Inquiry

Hello,

We are developing a consumer water-air contact device that requires an
inline UVC sterilization module. Specifications:

- Wavelength: 270-280nm
- Flow rate: 1-2 L/min (low flow application)
- Input voltage: 12V DC preferred (24V DC acceptable)
- Form factor: Inline, compact (< 150mm length)
- Connection: Standard barbed or push-fit (10-12mm ID)
- Certification: IEC 62471 photobiological safety preferred

Annual volume projection: 500-2000 units Year 1, scaling to 5000+ Year 2.

Could you provide:
1. Catalog of available modules in this specification range
2. Technical datasheets
3. Sample pricing (1-5 units)
4. Production pricing (500, 1000, 2000 units)
5. Lead times
6. Available certifications

Thank you,
[Name]
```

---

## 8. Risk Mitigation

### 8.1 Quality Risks with Chinese OEM Sourcing

| Risk | Mitigation |
|------|-----------|
| Sample quality ≠ production quality | Require production-line samples, not "golden samples" |
| Material substitution | Specify materials by grade (PP Grade X, not just "plastic") |
| Dimensional drift | Provide tolerance drawings, inspect first batch 100% |
| Certification fraud | Verify UL/CE certificates against issuing lab databases |
| IP theft | Don't share full system design with any single supplier |

### 8.2 Supply Chain Risks

| Risk | Mitigation |
|------|-----------|
| Single supplier dependency | Dual-source every critical component |
| Shipping delays | Maintain 4-6 week safety stock for critical parts |
| Tariff changes | Budget 25% tariff buffer for Chinese components |
| Component EOL | Choose components with active lifecycle status |
| Currency fluctuation | Price in USD, lock rates for large orders |

---

## 9. Key Decision: Peristaltic vs. Needle-Wheel

This is the single most important sourcing decision for PetFilter. The POC must test both:

| Factor | Peristaltic + Venturi | Needle-Wheel |
|--------|----------------------|-------------|
| Proven in PetFilter context | Matches original design | Requires validation |
| Air entrainment quality | Good (adjustable) | Excellent (optimized) |
| Noise profile | Pulsing (can be annoying) | Continuous (may be quieter) |
| Maintenance | Tubing swap (easy, DIY) | Impeller cleaning (moderate) |
| Water path | External (above water) | Submersible (in water) |
| UVC integration | Easy (inline after pump) | Need to route water out of tank |
| Cost | $13-17 | $12-20 |
| Supply chain depth | Deep (medical + lab) | Deep (aquarium) |
| Differentiator value | More "techy" looking | More elegant (hidden) |
| Failure mode | Tubing wear (predictable) | Impeller clog (requires cleaning) |

**Recommendation:** Build both for POC. Decide based on measured performance data, not assumptions.
