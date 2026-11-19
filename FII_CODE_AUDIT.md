# MIDAS FII Code Audit

## Executive summary

The pipeline has a credible append-only foundation for B3 COTAHIST snapshots and CVM monthly filings. The pinned `rsb3` parser, raw quotation retention, SHA-addressed artifacts, source-specific as-of gating, and no-distribution/no-total-return policy are sound design choices. The implementation report was broadly accurate about the live 2025 examples, but overstated validation and missed several important correctness gaps.

This audit fixed the confirmed point-in-time and integrity issues listed below. The pipeline is suitable for research on captured historical prices and CVM NAV after these fixes, with the stated limitations. It is not a complete historical event, universe, or backtesting system.

## Architecture and files inspected

Inspected `src/investing/fii_cotahist.rs`, `src/investing/fii_pipeline.rs`, `src/db/fii.rs`, `src/source_data/cvm/fii_reports.rs`, `src/investing/fii.rs`, `src/main.rs`, `Cargo.toml`, `Cargo.lock`, and `FII_PIPELINE_IMPLEMENTATION.md`. Traced archive download, parsing, persistence, identity assignment, filing as-of selection, monthly metric calculation, and report generation. Checked the actual SQLite DDL and test fixtures.

Key code locations: COTAHIST DDL and uniqueness, `src/investing/fii_cotahist.rs:158-207`; identity validation/persistence, `:225-302`; archive acquisition and B3 transaction, `:304-501`; CVM snapshot and atomic savepoint, `:502-619`; file hash verification, legacy availability repair, and cached reuse, `:620-721`; report as-of identity/date filters, `:822-1084`; P/NAV alignment, `:1085-1140`; CVM source retrieval timestamp, `src/source_data/cvm/fii_reports.rs:277-298`; CVM unique joins and importer savepoint, `src/investing/fii_pipeline.rs:296-365`; NAV calculation and reconciliation, `:595-700`; SQLite filing/market/distribution DDL, `src/db/fii.rs:20-73`; CLI options and execution, `src/main.rs:127-210` and `:522-616`.

The CLI is handled before the stock acquisition/scoring path. The stock QARP workflow was not changed in this audit. The FII schema is created through `CREATE TABLE IF NOT EXISTS` in the FII initializer; there is no versioned migration framework for these new tables.

## Implementation claims checked

| Claim | Audit result |
|---|---|
| `rsb3 = 0.1.0` | Verified in `Cargo.toml` and `Cargo.lock`; fetch feature enabled, default features disabled. |
| ZIP content hash and immutable paths | Verified; hash is over original ZIP bytes. This audit added hash verification for existing content-addressed files and cached snapshots. |
| Fresh cache prevents stale extracted TXT reuse | Verified in acquisition path. Normal mode reuses the latest stored ZIP but reparses in a new staging cache; refresh calls `fetch` with an empty staging cache. |
| BDI 12 selects FII rows | Implemented and exercised by synthetic parser tests and the earlier 2025 retrieval. This is a useful B3 classification rule, but three known FIIs do not establish exhaustive coverage or prove no other BDI/security cases are relevant. |
| HGLG11, XPLG11, VISC11 mappings and 250 records each in 2025 | Consistent with the retained validation notes/database report. This is cohort-specific empirical evidence, not proof of full-universe identity coverage. |
| CVM availability uses first MIDAS capture | True for the CLI importer. A second public importer path incorrectly used CKAN `last_modified`; fixed to use post-download capture time and retain CKAN metadata separately. |
| P/NAV staleness limit is five calendar days | Implemented in the report alignment query. The policy remains hard-coded and does not use an exchange calendar. A future reporting month is now explicitly excluded. |
| No total return or calculated distribution yield | Verified. Distribution output is explicitly unavailable. |
| CLI exposes acquisition and reports | Verified in `src/main.rs`: CVM year, COTAHIST year/day, refresh, identity/cohort files, as-of date, and snapshot directory options. |

## Findings and corrections

### P0 — Historical query admitted identity information learned later

**Evidence:** market observations store `fund_id` at ingestion. The report previously trusted that field without requiring the ISIN alias evidence to have been observed by the requested `as_of`. A later CVM archive could therefore resolve an old observation retroactively in an earlier as-of report.

**Fix:** reports now require an ISIN mapping whose `first_observed_at` is no later than `as_of` and whose effective dates cover the trading date. Unresolved counts use the same as-of rule. The market observation itself must also have a source snapshot captured by `as_of`, and its trading date must not exceed the requested as-of date. A regression test exercises a mapping learned after the market snapshot and an observation dated after the query date.

### P0 — CVM publication time was conflated with archive modification time in a legacy importer

**Evidence:** `FiiReportDataSource::download_snapshot` required and parsed CKAN `last_modified` into `available_at`; `fii_pipeline::ingest_monthly_year` passed that timestamp to filing persistence. That timestamp describes the resource, not when MIDAS received the artifact or when each row became public.

**Fix:** `DownloadedReportSnapshot` now separates `retrieved_at` from optional `source_modified_at`. `retrieved_at` is assigned after download completion and used as the conservative filing availability bound. The CLI path uses the same rule. On re-capture, known v1 records are advanced to the verified completed-download timestamp, including filing availability and source-derived identity first-observed times; the report records this conservative repair. Old databases retain their old bound until that CVM year/archive is ingested again.

### P1 — Content-addressed file names were trusted without verifying existing bytes

**Evidence:** if a file already existed at `<sha>.ZIP`, acquisition reused it without checking that its contents still matched the path or snapshot metadata. Disk damage or accidental replacement could silently undermine reproducibility.

**Fix:** existing B3 and CVM immutable files are SHA-256 verified before reuse. The latest cached B3 artifact is checked against the hash stored in SQLite before being copied into staging. A mismatch now fails ingestion instead of treating the changed bytes as the old snapshot.

### P1 — Duplicate companion-table rows could be joined arbitrarily

**Evidence:** archive rows were collected into `HashMap`s keyed by CNPJ/reference/version. Duplicate assets or complement rows overwrote one another, allowing an arbitrary last row to populate a report.

**Fix:** companion rows are now joined only when their key is unique. A regression test confirms conflicting duplicate complement rows yield no NAV instead of selecting either value.

### P1 — Report generation hid CVM query failures

**Evidence:** the COTAHIST report converted any `monthly_metrics_as_of` error into an empty vector with `unwrap_or_default()`, making a database/serialization failure look like missing CVM coverage.

**Fix:** the report now returns an explicit error. It also reports the CVM source revision ID and whether NAV/share came from the direct CVM field or the equity/shares fallback. When both are present and differ by more than 0.1%, a reconciliation diagnostic is emitted.

### P1 — Alphanumeric CNPJ identifiers were rejected

**Evidence:** the new COTAHIST-side CNPJ normalizer retained digits only, while the CVM pipeline already accepts ASCII alphanumeric registration identifiers. This would reject new-format CNPJs.

**Fix:** identity normalization now retains and uppercases ASCII letters and digits after punctuation removal.

### P1 — CVM ingestion had a crash window between filing persistence and snapshot completion

**Evidence:** filing rows were committed by an importer savepoint before the snapshot row was marked complete. A process interruption in that interval could leave reports visible from a `parsing` snapshot. Identity and metadata writes also occurred outside a common source transaction.

**Fix:** CVM snapshot registration, identity aliases/registry, fund metadata, filing rows, and completion status now run under one outer savepoint. Nested identity and filing savepoints remain atomic within it. Any error rolls the whole source ingestion back.

## Remaining findings

- **P1:** identity aliases and source snapshots have no foreign-key constraints. The current write path groups B3 snapshot and observations in a transaction, but referential integrity is not enforced for manual edits or future writers. There is no schema migration framework to add constraints safely to existing databases.
- **P1:** `fii_source_snapshots` and related tables are initialized with `CREATE TABLE IF NOT EXISTS`; changes to an existing table will not be migrated automatically. Current fixes are query/code changes and report fields, not DDL changes.
- **P1:** the FII filter currently keys on BDI 12 alone. Keep coverage labeled as observed BDI-12 FIIs until broader B3 code/specification coverage is verified. Unknown/other codes are not currently counted as candidate FIIs.
- **P1:** identity aliases are effective-dated in storage, but automatic CVM CNPJ/ISIN aliases currently have no known validity dates. Mapping conflicts make an ISIN unresolved when it points to multiple fund IDs, which is conservative, but ticker reuse and historical identifier changes need more fixtures.
- **P2:** the 0.1% NAV reconciliation threshold is a diagnostic convention, not a CVM data rule. Both raw input fields remain available in the filing payload.
- **P2:** `FiiMonthlyMetrics` keeps CVM yield source-reported and unannualized, but the source field's full scale semantics are not validated here.
- **P2:** fixture tests use a synthetic 245-byte record parsed by `rsb3`; they are not checked-in licensed real archives. The previous live archive evidence is documented, but live tests remain separate from deterministic fixtures.
- **P2:** COTAHIST parser output values are read through Arrow arrays without a comprehensive row-level validation report for every missing/null or implausible OHLC/volume combination. Factor `<= 0` is rejected during normalization.

## Test coverage assessment

Existing coverage includes the actual `rsb3` record batch schema, BDI/ISIN/price extraction, quotation-factor arithmetic, CVM optional dates, duplicate general keys, filing revision preservation, and month-end rules. This audit adds tests for as-of identity/trade-date gating, SHA mismatch rejection, identity savepoint rollback, legacy CVM availability advancement, alphanumeric CNPJ normalization, duplicate companion rejection, and NAV reconciliation. `cargo test` passes all 104 tests. There is still no deterministic end-to-end fixture that runs archive acquisition, snapshot persistence, correction ingestion, and report generation together; refresh/network behavior also lacks a mockable downloader integration test.

The earlier 2025 live-data figures are supported by the retained database and implementation notes, not reproduced by the unit test suite. See `FII_DATA_INTEGRITY_REVIEW.md` for readiness and source limitations.
