# Odor Chemistry: Cat Litter & Pet Waste Volatile Compounds

## Executive Summary

Cat litter box odor is a complex mixture of volatile compounds dominated by ammonia (NH3) from urea decomposition, sulfur compounds from amino acid metabolism, and various volatile organic compounds (VOCs). Understanding the chemistry of each compound -- especially its water solubility and UV reactivity -- is essential for designing an effective scrubbing system.

---

## 1. Primary Odor Compounds

### 1.1 Ammonia (NH3)

**Source**: Bacterial urease enzyme converts urea (CO(NH2)2) in cat urine to ammonia and CO2. This is the dominant odor-producing reaction.

**Chemical reaction**: CO(NH2)2 + H2O → 2NH3 + CO2

**Properties**:
- Molecular weight: 17.03 g/mol
- Boiling point: -33.34°C (highly volatile)
- Odor threshold: 5-50 ppm (varies by individual)
- Typical concentration near litter box: 10-50 ppm
- Immediately dangerous to life: 300 ppm

**Water solubility**: EXTREMELY HIGH
- Henry's law constant (H): 0.00075 atm·m³/mol at 25°C
- Aqueous solubility: 31% w/w at 25°C (899 g/L)
- pH-dependent: NH3 + H2O ⇌ NH4+ + OH- (pKa = 9.25)
- At neutral pH (~7): >99% of dissolved ammonia exists as NH4+ (ammonium ion)
- This means water actively PULLS ammonia from air into solution

**Scrubbing prediction**: 80-95% removal per pass through venturi. This is the primary target compound and the system's strongest advantage.

### 1.2 Hydrogen Sulfide (H2S)

**Source**: Anaerobic bacterial decomposition of sulfur-containing amino acids (cysteine, methionine) in fecal matter.

**Properties**:
- Molecular weight: 34.08 g/mol
- Odor threshold: 0.5 ppb (extremely low -- detectable at trace levels)
- Characteristic "rotten egg" smell
- Typical concentration near litter box: 0.1-2 ppm
- Toxic at >50 ppm

**Water solubility**: MODERATE
- Henry's law constant: 0.105 atm·m³/mol at 25°C
- pH-dependent: H2S ⇌ HS- + H+ (pKa1 = 7.0)
- At neutral pH: ~50% dissociated, improving effective solubility
- Lower pH slightly reduces capture; higher pH improves it

**Scrubbing prediction**: 60-80% removal per pass. Moderate but meaningful. UVC photolysis provides additional degradation of dissolved H2S.

### 1.3 Methyl Mercaptan (CH3SH / Methanethiol)

**Source**: Bacterial degradation of methionine. Also produced during felinine metabolism (cat-specific).

**Properties**:
- Molecular weight: 48.11 g/mol
- Odor threshold: 0.002 ppm (2 ppb) -- extremely potent
- Characteristic "cabbage/garlic" smell
- Typical concentration near litter box: 0.01-0.5 ppm

**Water solubility**: MODERATE
- Henry's law constant: 0.19 atm·m³/mol at 25°C
- Less soluble than ammonia but still captures meaningfully

**Scrubbing prediction**: 50-70% removal per pass. Combined with carbon filtration, total system removal approaches 85-95%.

### 1.4 Felinine & Derived Sulfur Compounds

**Source**: Felinine (2-amino-7-hydroxy-5,5-dimethyl-4-thiaheptanoic acid) is a unique amino acid found exclusively in cat urine. Bacterial action converts felinine to volatile sulfur compounds.

**Key derivative**: 3-mercapto-3-methylbutan-1-ol (MMB) -- the compound primarily responsible for the distinctive "cat pee" smell

**Research reference**: Miyazaki et al. (2006), Journal of Chemical Ecology. Control of felinine-derived malodor documented in JFMS (2021).

**Properties**:
- Extremely low odor threshold
- Moderate water solubility (sulfur-containing, polar)
- Susceptible to oxidative degradation (UVC + dissolved O2)

**Scrubbing prediction**: 60-80% removal. The polar sulfur group aids dissolution.

### 1.5 Trimethylamine (TMA, (CH3)3N)

**Source**: Bacterial degradation of choline and carnitine in fecal matter. Responsible for "fishy" odor component.

**Properties**:
- Molecular weight: 59.11 g/mol
- Odor threshold: 0.00021 ppm (0.21 ppb) -- one of the most potent odorants
- Strong "fishy" smell

**Water solubility**: EXTREMELY HIGH
- Henry's law constant: 0.0005 atm·m³/mol at 25°C
- Highly soluble amine, readily protonates in water

**Scrubbing prediction**: 85-95% removal per pass. Excellent target for water scrubbing.

### 1.6 Volatile Organic Compounds (VOCs)

**Skatole (3-methylindole)**: Fecal odor compound. Moderate water solubility. Carbon filter primary removal mechanism.
**Indole**: Similar to skatole but less potent. Moderate solubility.
**p-Cresol**: Phenolic compound from amino acid degradation. Moderate solubility.
**Various amines**: Putrescine, cadaverine (from protein decomposition). Highly water-soluble.

---

## 2. Henry's Law Summary Table

Henry's law governs gas-liquid equilibrium: lower H = higher water solubility = better scrubbing performance.

| Compound | H (atm·m³/mol @ 25°C) | Solubility Class | Expected Removal |
|----------|----------------------|-----------------|-----------------|
| Ammonia (NH3) | 0.00075 | Very High | 80-95% |
| Trimethylamine | 0.0005 | Very High | 85-95% |
| Hydrogen sulfide (H2S) | 0.105 | Moderate | 60-80% |
| Dimethyl disulfide | 0.15 | Moderate | 55-75% |
| Methyl mercaptan | 0.19 | Moderate | 50-70% |
| Skatole | ~0.01 (est.) | High | 70-85% |
| Indole | ~0.02 (est.) | High | 65-80% |
| p-Cresol | 0.012 | High | 70-85% |

**Reference**: Sander, R. (2015). "Compilation of Henry's law constants for water as solvent." Atmos. Chem. Phys., 15, 4399-4981. (Most comprehensive Henry's law database)

---

## 3. Odor Concentration Timeline

Ammonia production from cat urine follows a predictable curve:

| Time After Urination | NH3 Concentration (near litter) | Notes |
|---------------------|-------------------------------|-------|
| 0-1 hours | 2-10 ppm | Urea still intact, low ammonia |
| 1-4 hours | 10-25 ppm | Urease activity accelerating |
| 4-12 hours | 25-50 ppm | Peak production period |
| 12-24 hours | 40-60 ppm | Sustained high levels |
| 24-48 hours | 30-50 ppm | Urea supply depleting |
| 48+ hours | 20-40 ppm | Steady state (litter saturation) |

**Implication**: PetFilter's gas sensor should trigger activation at ~10 ppm and sustain operation until levels drop below 5 ppm.

---

## 4. pH Effects on Scrubbing Efficiency

The water in the scrubbing system will gradually change pH as it absorbs compounds:

- **NH3 absorption raises pH**: NH3 + H2O → NH4+ + OH-
- **H2S absorption lowers pH**: H2S → H+ + HS-
- **Net effect**: Slight pH rise (ammonia dominates by mass)

**Optimal pH range**: 6.5-8.0
- At pH 7: Excellent ammonia capture, good H2S capture
- Above pH 9: Ammonia equilibrium shifts unfavorably (less NH4+ formation)
- Below pH 6: H2S equilibrium shifts unfavorably

**Water replacement frequency**: Every 7-14 days maintains effective pH range without additives. Optional citric acid tablets could buffer pH.

---

## 5. UVC Photolysis of Dissolved Compounds

Once odor compounds are dissolved in water, UVC light (265-280nm) initiates photochemical degradation:

### 5.1 Direct Photolysis
- **NH3/NH4+**: Weak direct absorption at 265-280nm. Requires advanced oxidation.
- **H2S/HS-**: Moderate absorption. H2S + hv → H• + HS• (radical formation)
- **Mercaptans**: Moderate absorption. R-SH + hv → R-S• + H•
- **Amines**: Good absorption. Degradation to N2, CO2, H2O

### 5.2 Advanced Oxidation (UVC + Dissolved O2)
UVC irradiation of water generates hydroxyl radicals (•OH) via:
- H2O + hv (185nm) → •OH + H• (requires vacuum UV, less relevant for LEDs)
- O2 dissolved in water + hv → O3 → •OH (secondary pathway)
- Venturi bubble entrainment maintains high dissolved O2

•OH radicals are extremely powerful oxidizers (E° = 2.80V) that non-selectively destroy organic compounds.

### 5.3 Photocatalytic Enhancement (Optional)
TiO2-coated surfaces in the UVC chamber generate additional •OH radicals:
- TiO2 + hv → e- + h+
- h+ + H2O → •OH + H+
- This significantly enhances ammonia degradation

**Design decision**: TiO2 coating adds ~$5-10 to BOM but potentially doubles ammonia destruction rate. Recommended for production version.

---

## 6. Key Findings

1. **Ammonia is the ideal target for water scrubbing**: H = 0.00075, meaning water has enormous capacity to absorb it. No other consumer technology exploits this.

2. **Trimethylamine is equally well-suited**: H = 0.0005. The "fishy" component of cat litter odor dissolves readily.

3. **Sulfur compounds are moderately captured**: 50-80% per pass. Combined with carbon filtration and UVC degradation, total removal reaches 85-95%.

4. **The system addresses ALL major odor families**: Water for ammonia/amines, carbon for VOCs, UVC for degradation of dissolved compounds, HEPA for particulates.

5. **Water chemistry is self-regulating**: pH stays in effective range for 7-14 days before water replacement needed.

6. **UVC + dissolved O2 from venturi creates advanced oxidation conditions**: This degrades compounds that simple dissolution alone would not eliminate.
