# Regulatory Compliance Roadmap

## 1. Applicable Standards

### 1.1 Electrical Safety
- **UL 62368-1** (Audio/Video, Information and Communication Technology Equipment)
- **CSA C22.2 No. 62368-1** (Canadian equivalent)
- **IEC 62368-1** (International)

### 1.2 Electromagnetic Compatibility (EMC)
- **FCC Part 15, Subpart B** (US, unintentional radiator)
- **FCC Part 15, Subpart C** (US, intentional radiator -- WiFi/BLE)
- **EN 55032** (EU emissions)
- **EN 55035** (EU immunity)

### 1.3 Photobiological Safety
- **IEC 62471** (Photobiological safety of lamps and lamp systems)
- UVC must be classified as "Exempt" or "Risk Group 1" with enclosure

### 1.4 Environmental
- **RoHS** (Restriction of Hazardous Substances) -- EU and many US retailers require
- **REACH** (EU chemical regulation)
- **WEEE** (Waste Electrical and Electronic Equipment) -- EU recycling directive

### 1.5 Wireless
- **FCC Part 15.247** (WiFi 2.4GHz)
- **FCC Part 15.249** (BLE)
- **RED** (EU Radio Equipment Directive)
- Note: Using pre-certified ESP32 module covers most wireless requirements

---

## 2. Certification Strategy

### 2.1 Use Pre-Certified Modules
The ESP32-S3-WROOM-1 module has existing FCC and CE modular approvals. By using it as a pre-certified module (without modifications to the antenna), the product only needs unintentional radiator testing (Part 15B), not full intentional radiator testing. This saves $5,000-10,000 in testing costs.

### 2.2 External Power Supply
Using an externally UL-listed power supply (Mean Well or equivalent) removes AC-DC conversion from the product scope. The product is classified as "DC-powered equipment" which has simpler safety requirements.

### 2.3 UVC Enclosure Design
The UVC chamber must be fully enclosed with no UV leakage path. Design measures:
- Opaque housing material (ABS or PP, zero UV transmission)
- Gasketed seams on all joints
- Hardware interlock (reed switch) prevents operation when opened
- IEC 62471 testing should show "Exempt" classification with enclosure sealed

---

## 3. Testing Labs

| Lab | Location | Services | Estimated Cost |
|-----|----------|----------|---------------|
| TUV Rheinland | Various US | FCC, UL, CE, IEC 62471 | $8,000-15,000 |
| Intertek (ETL) | Various US | UL alternative (ETL mark), FCC | $6,000-12,000 |
| MET Labs | Baltimore, MD | FCC, UL, focused on small products | $5,000-10,000 |
| Eurofins | Multiple | FCC pre-compliance, full test | $4,000-8,000 |

**Recommendation**: Start with FCC pre-compliance at a local EMC lab ($500-1,000) to identify issues before committing to full testing.

---

## 4. Timeline

| Month | Activity | Status |
|-------|----------|--------|
| 6 | FCC pre-compliance scan | Design validation |
| 8 | Design freeze for certification | Final design |
| 8-9 | Submit samples for FCC Part 15 | Testing |
| 9-10 | Submit samples for UL 62368-1 | Testing |
| 10 | FCC approval received | Approved |
| 11 | UL listing received | Approved |
| 12 | CE testing (if EU launch) | Testing |
| 12 | Production release | Ready to ship |

---

## 5. Product Labeling Requirements

### 5.1 Required Markings
- FCC ID (or Declaration of Conformity statement)
- UL/ETL listing mark
- CE mark (EU)
- RoHS compliance mark
- WEEE symbol (EU)
- Product name, model number
- Manufacturer name and address
- Input voltage/current rating
- Serial number
- Country of origin

### 5.2 UVC Warning Label
Required per IEC 62471 and ANSI/IESNA RP-27:
- "CAUTION: UV-C RADIATION. DO NOT LOOK DIRECTLY AT LIGHT SOURCE."
- Even though UV is fully enclosed, label is required on the UVC chamber interior (visible during maintenance)
