# Unit Economics: PetFilter

## 1. Device Economics

### 1.1 Cost Structure (at 500 units)

| Item | Cost |
|------|------|
| BOM (components) | $126.65 |
| Assembly labor | $8.00 |
| Testing/QC | $3.00 |
| **Landed cost** | **$137.65** |
| Shipping to warehouse | $5.00 |
| Packaging | included in BOM |
| **Total COGS** | **$142.65** |

### 1.2 Revenue per Unit

| Channel | Retail Price | Platform Fee | Net Revenue |
|---------|------------|-------------|-------------|
| DTC (Shopify) | $399 | 2.9% + $0.30 (Stripe) | $387.13 |
| Amazon | $399 | 15% referral | $339.15 |
| Kickstarter | $299 (early bird) | 5% + 3% payment | $275.08 |
| Wholesale (Chewy/Petco) | $399 retail, 50% wholesale | $0 (wholesale) | $199.50 |

### 1.3 Gross Margin per Unit

| Channel | Net Revenue | COGS | Gross Profit | Gross Margin |
|---------|-----------|------|-------------|-------------|
| DTC | $387.13 | $142.65 | $244.48 | 63.1% |
| Amazon | $339.15 | $142.65 | $196.50 | 57.9% |
| Kickstarter | $275.08 | $142.65 | $132.43 | 48.1% |
| Wholesale | $199.50 | $142.65 | $56.85 | 28.5% |

**Blended gross margin** (assuming 40% DTC, 40% Amazon, 10% KS, 10% wholesale): **57.2%**

---

## 2. Filter Subscription Economics

### 2.1 Replacement Cartridge Costs

| Item | COGS | Retail | Subscription Price |
|------|------|--------|-------------------|
| Intake filter (HEPA+carbon) | $4.00 | $18.99 | $9.99/mo bundle |
| Exhaust filter (carbon) | $2.50 | $12.99 | included |
| Water treatment tablet | $0.30 | included | included |
| Shipping | $3.50 | included | included |
| **Total per shipment** | **$10.30** | **$31.98** | **$29.97/quarter** |

### 2.2 Subscription Margin

| Billing | Revenue | COGS | Gross Profit | Margin |
|---------|---------|------|-------------|--------|
| Monthly ($9.99) | $9.99 | $3.43/mo | $6.56 | 65.7% |
| Quarterly ($29.97) | $29.97 | $10.30 | $19.67 | 65.6% |
| Annual ($99.99) | $99.99 | $41.20 | $58.79 | 58.8% |

### 2.3 Subscription Adoption Assumptions
- 60% of device buyers subscribe (based on pet product subscription benchmarks)
- Average subscription length: 24 months
- Monthly churn: 4-6%

---

## 3. Customer Lifetime Value (LTV)

### 3.1 LTV Calculation

| Component | Value | Notes |
|-----------|-------|-------|
| Device revenue (blended) | $337 | Weighted average across channels |
| Device gross profit | $194 | 57.2% margin |
| Subscription revenue (24mo × $9.99) | $240 | At 60% adoption |
| Subscription gross profit | $158 | 65.7% margin |
| **Total LTV (revenue)** | **$481** | Per device buyer |
| **Total LTV (gross profit)** | **$290** | Per device buyer |

### 3.2 LTV:CAC Target
- Target customer acquisition cost (CAC): $50-80
- LTV:CAC ratio: 3.6-5.8x
- Payback period: <3 months (device purchase covers CAC immediately)

---

## 4. Break-Even Analysis

### 4.1 Fixed Costs (Year 1)

| Item | Cost |
|------|------|
| Injection mold tooling (housing) | $15,000 |
| Injection mold tooling (tanks, venturi, filters) | $10,000 |
| PCB design finalization | $2,000 |
| Certifications (FCC, UL) | $8,000-15,000 |
| Patent filing (provisional) | $3,000 |
| Website + branding | $5,000 |
| Marketing (pre-launch) | $10,000 |
| **Total fixed costs** | **$53,000-60,000** |

### 4.2 Break-Even Units

Using blended gross profit of $194/unit:
- Break-even = $60,000 / $194 = **310 units**
- At Kickstarter price ($132 GP): 455 units

### 4.3 Path to Break-Even

| Milestone | Units | Revenue | Cumulative GP | Status |
|-----------|-------|---------|--------------|--------|
| Kickstarter | 200 | $59,800 | $26,486 | -$33,514 |
| Post-KS DTC (3 months) | 150 | $59,850 | $36,672 | -$23,328 |
| Amazon launch (3 months) | 200 | $79,800 | $39,300 | +$15,972 |
| **Total to break-even** | **~350** | **~$125K** | **~$60K** | **Break-even at ~8 months post-launch** |

---

## 5. Sensitivity Analysis

### 5.1 Price Sensitivity

| Retail Price | COGS | Gross Margin (DTC) | Break-even Units |
|-------------|------|-------------------|-----------------|
| $299 | $143 | 49.3% | 408 |
| $349 | $143 | 56.2% | 328 |
| $399 | $143 | 61.3% | 272 |
| $449 | $143 | 65.2% | 235 |

### 5.2 BOM Sensitivity

| BOM at Volume | COGS | Gross Margin ($399 DTC) | Break-even Units |
|--------------|------|------------------------|-----------------|
| $150 (pessimistic) | $158 | 57.7% | 296 |
| $127 (baseline) | $143 | 61.3% | 272 |
| $105 (optimistic) | $121 | 66.0% | 233 |

---

## 6. Key Metrics Summary

| Metric | Value |
|--------|-------|
| Target retail price | $399 |
| COGS (500 units) | $143 |
| Blended gross margin | 57.2% |
| Break-even units | ~310 |
| Filter subscription margin | 65.7% |
| Customer LTV (gross profit) | $290 |
| Target CAC | $50-80 |
| LTV:CAC | 3.6-5.8x |
| Monthly operating cost to user | <$1 electricity + $10 filters |
