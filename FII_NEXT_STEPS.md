# MIDAS FII Next Steps

This roadmap keeps source evidence and point-in-time correctness ahead of any scoring or strategy layer.

## 1. Immediate data-integrity corrections

### Priority A — Lock CVM ingestion atomicity with failure-injection coverage

**Dependency:** none.

The implementation now puts the CVM source snapshot row, CNPJ/ISIN aliases, fund metadata, monthly filing revisions, and `ingestion_status='complete'` under one outer SQLite savepoint. Add an injectable persistence failure so this boundary remains protected against regressions.

**Acceptance criteria:** inject a failure after filing persistence but before completion; assert all writes roll back. Re-ingest the same snapshot and assert it becomes complete, remains idempotent, and preserves first capture time.

### Priority B — Complete source-record validation

**Dependency:** BDI/security-code coverage decision for FII classification.

Add row-level diagnostics for Arrow nulls, invalid dates, invalid/nonpositive factors, non-BRL currencies, impossible OHLC orderings, duplicate keys, and nonstandard BDI/market/specification combinations. Preserve rejected raw rows or explicit counts/examples so filtered coverage can be assessed.

**Acceptance criteria:** synthetic fixtures cover null/malformed/factor/duplicate cases; no invalid row is silently converted into a valid zero-valued price; report identifies excluded row counts and reasons.

### Priority C — Deterministic end-to-end correction fixture

**Dependency:** a small synthetic ZIP fixture in the documented COTAHIST layout and three-table CVM fixture. Production must continue using `rsb3` as its parser.

Run a local-only ingestion fixture through source snapshot, observation/filing persistence, identity mapping, as-of report, repeat ingestion, and simulated revised artifact.

**Acceptance criteria:** test asserts raw values and source hashes, idempotent repeat, correction coexistence, previous as-of output, later mapping availability, and final JSON stability. No network is used.

## 2. Distribution-event acquisition

### Priority A — Obtain B3 UP2DATA Corporate Action evidence

**Dependency:** B3 sales/access response and applicable contract/retention terms.

Request a sample specifically containing FII Rendimentos and Amortizações, plus schema/data dictionary, historical availability, event IDs, corrections/cancellations, announcement and ex/record/payment dates, and historical-data licensing scope. The public product description is encouraging, but the actual historic product/channel still needs validation.

**Acceptance criteria:** parse a licensed sample; map stable event IDs; reconcile a sample of HGLG11, XPLG11, and VISC11 events against administrator notices; document gaps and legal retention limits.

### Priority B — Evaluate a paid third-party alternative if B3 coverage is unavailable

**Dependency:** provider access and written retention/usage rights.

Test brapi or another documented provider against the same manually verified event sample. Track actual source dates separately from provider ingestion/verification timestamps. Do not substitute `approvedOn`, payment date, or inferred date for publication time without evidence.

**Acceptance criteria:** documented completeness by year/fund/type; corrections and cancellations tested; field semantics validated; source payload/hash retained; discrepancies reported.

### Priority C — Implement event storage after validation

Use stable source event IDs, event type, amount/currency, announcement/availability timestamps, ex/data-com/record/payment dates, reference period, revision/cancellation state, snapshot, original payload, and data-quality flags. Unknown dates remain unknown.

**Acceptance criteria:** as-of event revisions behave correctly; entitlement and cash-flow dates are not conflated; no cash-yield or total-return metric is enabled until source coverage passes the validated cohort.

## 3. Historical identity and universe coverage

**Dependency:** authoritative B3/CVM/fund administrator identity and listing sources.

Expand beyond the three technical examples. Establish stable fund CNPJ and effective-dated ISIN/ticker/security aliases; document delistings, name changes, incorporations, and ambiguous class/fund cases. Verify new alphanumeric CNPJ handling and ISIN validation against official records.

**Acceptance criteria:** effective-dated regression cases include ticker reuse/change, ISIN change, ambiguous CNPJ/ISIN, delisted FII, and mapping evidence captured after the target as-of. Report coverage counts by period and unresolved reason.

Historical market observations are not a historical investable universe. IFIX history must be acquired and constituent membership sourced separately if index-universe studies are planned.

## 4. Exchange calendar and liquidity data

**Dependency:** reliable official/licensed session calendar and stable COTAHIST field semantics.

Add a B3 trading-session calendar to distinguish expected sessions from weekends/holidays. Then compute liquidity summaries from raw daily quantity/trade/financial volume with explicit zero-trade and stale-data rules.

**Acceptance criteria:** calendar matches exchange sessions for test periods; no claim of missing trading data is made without a session expectation; liquidity metrics expose observation window and source snapshot IDs.

## 5. FII indicator development

**Dependency:** source quality, units, identity, and event availability accepted for each metric.

Build indicators as a separate consumer of PIT observation/report APIs. Include raw inputs, formulas, unit semantics, as-of availability, and data quality with every output. Keep CVM-reported yield separate from event-derived cash yield.

**Acceptance criteria:** each metric has unit tests, source references, missing-data behavior, outlier policy, and a snapshot-pinned reproducibility example. Do not introduce a composite score until peer classification is effective-dated and validated.

## 6. Strategy and backtesting readiness

**Dependency:** historical universe, publication histories, complete distributions/corporate actions, calendar, and survivorship controls.

Only then begin strategy research or backtests. Point-in-time signal availability must use CVM/B3 event knowledge time; trades must observe execution timing, liquidity, fees, taxes, and suspensions. Historical prices must remain raw or use a documented event-adjustment series with complete corporate actions.

**Acceptance criteria:** a replay from archived snapshots reproduces signals and portfolio cash flows; delisted funds are retained; no data is visible before its source availability; benchmark constituents and rebalancing rules are historical; sensitivity to data gaps is reported.
