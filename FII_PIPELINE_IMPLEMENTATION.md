# MIDAS FII Historical Pipeline — Implementation Notes

## Status

The first reproducible CVM monthly-report and B3 COTAHIST acquisition path is implemented and exposed through the existing clap CLI. This is a price-and-filing foundation, not a complete total-return data set: no validated historical distribution adapter is available, so cash yield and total return remain unavailable. The post-implementation audit and integrity findings are in `FII_CODE_AUDIT.md`, `FII_DATA_INTEGRITY_REVIEW.md`, and `FII_NEXT_STEPS.md`.

## Architecture changes

- `src/investing/fii_cotahist.rs` acquires COTAHIST, preserves immutable artifacts, parses with `rsb3`, resolves ISINs to CVM CNPJs, persists raw/normalized OHLC and emits an as-of report.
- `src/db/fii.rs` remains the filing/PIT persistence layer. New FII snapshot, identity, fund metadata, and full COTAHIST OHLC tables are created by the FII initializer. The older single-price market table was not expanded because it cannot hold the COTAHIST raw fields and snapshot revision contract.
- `src/investing/fii_pipeline.rs` now supports snapshot-hash revisions and preserves duplicate CVM general rows. Companion tables are not joined when the CVM CNPJ/reference/version key is ambiguous.
- `src/source_data/cvm/fii_reports.rs` handles blank optional date fields in real CVM CSV files and reports which table/row failed parsing.
- `src/main.rs` adds acquisition, refresh, identity/cohort configuration, and report options. Stock/QARP scoring code was not changed.

## rsb3 integration and cache behavior

Dependency: `rsb3 = 0.1.0`, exactly pinned, with only the `fetch` feature enabled (`default-features = false`). `arrow = 59` is used to read the actual `RecordBatch` schema; `sha2 = 0.11` hashes source artifacts.

The inspected crate has `Period::Year(i32)` and `Period::Day(NaiveDate)`, `Cache { dir }`, `cotahist::fetch`, `cotahist::parse`, and `cotahist::to_parquet`. Version 0.1.0's cache returns immediately when an extracted TXT exists and reuses an existing ZIP without checking the remote file. Its manifest stores the ZIP SHA-256 and byte size, but does not revalidate a cached archive.

MIDAS avoids stale TXT reuse by using a fresh private cache directory for each ingestion. Normal ingestion copies the latest stored immutable ZIP into that fresh cache and re-parses it. `--fii-refresh` leaves the cache empty so rsb3 fetches the current remote ZIP. Every archive is stored at `data/fii/snapshots/b3/<period>/<sha256>.ZIP`; an earlier version is never overwritten. Annual and daily periods are supported. CVM monthly ZIPs are stored under `data/fii/snapshots/cvm/<year>/<sha256>.zip`.

The adapter checks schema names and Arrow types from the actual crate. The crate's Arrow field called `allocation_lot_size` is the raw COTAHIST `FATCOT`; MIDAS stores it as `quotation_factor`. B3's [official COTAHIST layout](https://b3.com.br/data/files/C8/F3/08/B4/297BE410F816C9E492D828A8/SeriesHistoricas_Layout.pdf) defines OHLC values as `V99` (two implied decimal places) and FATCOT as 1 for unit quotation or 1000 for a thousand-share lot. MIDAS preserves raw integer cents and normalizes BRL as `raw_cents / 100 / quotation_factor`. A zero/invalid factor is rejected. `rsb3` does not return an original ticker field separately; MIDAS retains its parsed `symbol`, ISIN, BDI, market, security name/specification, currency, bids/asks, trades, quantity, and volume.

## Identity and point-in-time semantics

The stable key is `cnpj:<14 normalized characters>`. CVM monthly report CNPJ/ISIN pairs populate the evidence-backed identity registry. BDI 12 filters FII market records; ISIN is the join key and ticker is stored as a time-dated observed alias. Unresolved market rows remain in storage and are counted in the report. A JSON identity file is also available for configured mappings; its evidence and configured confidence are retained rather than rewritten when a matching COTAHIST row is later observed. Report queries require the identity evidence to have been observed by `as_of` and to be valid for the trading date.

CVM `Data_Entrega` is preserved as a date and is not interpreted as a publication timestamp. CVM delivery-dataset joins and receipt-time semantics remain unverified. Filing `available_at` is the first MIDAS capture time; `ingested_at` is kept separately. A changed CVM ZIP hash creates separate filing revision IDs, while same-hash ingestion is idempotent. B3 COTAHIST observations are available from their snapshot retrieval time. CKAN `last_modified` is retained as source metadata only.

P/NAV uses the CVM reference date as a reporting month, selects the last B3 close on or before calendar month-end, requires the price to be no more than five calendar days stale, and requires both source snapshot and reporting month-end to be admissible by the requested `as_of`. The report records the aligned date and market snapshot, CVM source revision, and NAV method. No exchange calendar is present, so weekends/holidays are not classified as missing sessions. CVM-reported monthly yield is kept separate from calculated distribution yield. No total return is calculated.

## Run the workflow

```powershell
cargo run -- --db midas.db `
  --fii-cvm-year 2025 `
  --fii-cotahist-year 2025 `
  --fii-refresh `
  --fii-identities fii-identities.json `
  --fii-cohort fii-cohort.json `
  --fii-report-as-of 2026-11-18
```

Repeat `--fii-cvm-year`, `--fii-cotahist-year`, or use `--fii-cotahist-day YYYY-MM-DD` for incremental daily archives. `--fii-refresh` re-fetches rather than reusing the most recently stored COTAHIST ZIP. The cohort file is a JSON array of ticker strings. Identity input is a JSON array with `cnpj`, `isin`, optional `ticker`, `fund_name`, `cvm_segment`, optional `valid_from`/`valid_to`, and an `evidence` string. Running CVM ingestion first creates CNPJ/ISIN mappings automatically.

## Live validation performed

The official B3 2025 annual ZIP (SHA-256 `6cc0ad851c8f605ca30418acac48be81f1767f6ed05b4f563772d06ae448047d`, 89,056,788 bytes) parsed to 3,174,698 market records, including 74,449 BDI 12 FII records. HGLG11, XPLG11, and VISC11 each matched the expected ISIN and CVM CNPJ and had 250 trading observations from 2025-01-02 to 2025-12-30. All three had 12 CVM report periods in the 2025 archive. Their reported CVM segments were Multicategoria, Logística, and Shoppings respectively; they should not be treated as one peer group.

The report was generated for `as_of=2026-11-18`. It contains raw close, normalized close, daily trades/quantity/financial volume, CVM NAV/yield/version/availability, aligned P/NAV, mapping evidence, source snapshot IDs, and unresolved-identity counts. A downloaded raw B3 ZIP is retained under the configured snapshot directory. The CVM run persisted 40,978 typed filing-table observations across the three monthly-report tables.

## Current limitations

- B3 UP2DATA Corporate Action access, schema, and license remain unverified. No historical FII distribution events are persisted; trailing cash yield and total return are explicitly unavailable.
- `fi-doc-entrega` timestamp joins to the structured monthly reports remain unverified. No receipt timestamp is presented as public availability.
- An initial CVM archive probe exposed blank optional dates and duplicate Fund/Class keys. Blank dates are now parsed as `None`; ambiguous rows are preserved and not force-joined. The report emits an ambiguity diagnostic.
- The B3 historical-data product's production automation and redistribution rights were not determined here. Confirm terms for the intended use.
- There is no exchange trading calendar, so session-level gaps are not asserted.
- The three example mappings were validated as technical examples. A broader 3–5-fund cohort is configurable and has not been approved as a homogeneous peer universe.

## Audit follow-up

The code audit added as-of identity/trading-date gates, SHA verification on snapshot reuse, atomic CVM source writes, legacy CVM timestamp repair on recapture, unique-only companion joins, NAV-source reconciliation, and explicit NAV/revision provenance. Reingest a legacy CVM archive to advance its earlier pre-download availability bound conservatively.

## Tests and checks

Fixture tests cover the actual `rsb3` schema, BDI/ISIN classification, cent and quotation-factor conversion, CVM blank dates, ambiguous general/companion rows, revision preservation, as-of identity gating, legacy availability repair, snapshot hash verification, and month-end alignment. The latest audit run passed `cargo check`, 104 tests, and `cargo clippy --all-targets`; Clippy still reports repository warnings.
