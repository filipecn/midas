# MIDAS FII Data Integrity Review

## Scope and conclusion

This review traces the current implementation from source acquisition through normalized SQLite observations and JSON reporting. MIDAS now has a defensible captured-data workflow for COTAHIST prices and CVM monthly reports. Its as-of output means “what MIDAS had captured and could identify by this time”; it does not reconstruct B3/CVM public knowledge timestamps before MIDAS began collecting artifacts.

The pipeline is **partially validated** for historical price and NAV research on mapped FIIs. It is not ready for distribution yield, total-return, survivorship-free universe studies, or trusted historical backtests.

## COTAHIST source and snapshot integrity

- `rsb3` is pinned at exactly 0.1.0 with only `fetch` enabled. MIDAS reads the crate's Arrow `RecordBatch` and checks field names/types through downcasts. A crate/schema change should fail rather than silently reinterpret fields.
- The original ZIP bytes are SHA-256 hashed and stored under a hash-derived path. SQLite stores source, period/resource, filename, retrieval time, optional source-modified time, size, path, parser/schema version, status, count, and diagnostics. Existing files are now hash-checked before reuse.
- A new staging cache avoids the crate's extracted-TXT short circuit. Normal ingestion reparses the latest completed local ZIP; refresh asks `rsb3` to fetch into an empty cache. Old ZIPs are not overwritten. Failed parsing occurs before the SQLite observation transaction. B3 snapshot registration and observation inserts/status completion share one transaction.
- Observation uniqueness includes snapshot, ticker, ISIN, trading date, and market. Corrected archives can therefore coexist. Ordinary report series pick the last captured version for overlapping dates; the snapshot IDs remain available for revision comparison.
- The parser adapter stores raw integer OHLC and volume values and normalized BRL OHLC. It treats `allocation_lot_size` (the crate's name for `FATCOT`) as quotation factor and computes `raw_cents / 100 / factor`. Raw volume remains unchanged. These are unadjusted quotations; no corporate action adjustment is claimed.
- BDI 12 is the implemented FII discriminator. The known HGLG11, XPLG11, and VISC11 examples were observed and mapped in the 2025 annual archive, each with 250 dates (2025-01-02 through 2025-12-30). This validates those examples, not all possible BDI/security classifications. Unresolved BDI-12 rows are retained.

The official [B3 COTAHIST page](https://www.b3.com.br/en_us/market-data-and-indices/data-services/market-data/historical-data/) describes historical quotation data; the [B3 layout](https://b3.com.br/data/files/C8/F3/08/B4/297BE410F816C9E492D828A8/SeriesHistoricas_Layout.pdf) defines source fields and implied decimals. Redistribution and automated-use terms for MIDAS's intended deployment remain to be confirmed.

## CVM source, filing revisions, and availability

- The monthly archive parser reads its three tables independently, preserves extra source columns, accepts blank optional dates as `None`, and reports table plus source row on parse errors.
- Companion tables join on normalized CNPJ/reference date/version only when both general and companion keys are unique. Duplicate rows remain unjoined and diagnostics report missing/ambiguous information.
- Filing revision IDs include the source archive hash, so changed archives remain separately queryable even when CVM's `Versao` is unchanged. Identical payload/revision ingestion is idempotent and does not rewrite the original availability timestamp.
- `Data_Entrega` remains a date in the stored payload. CKAN `last_modified` is retained as archive metadata and is not used as row availability. Both CVM importer paths now use MIDAS time after the download completes as the conservative availability boundary. Re-ingesting an archive tagged with the legacy v1 adapter advances its old pre-download timestamps (snapshot, filings, metadata, and source-derived identity evidence) to the completed recapture time; this correction is deliberately conservative. Existing databases need that archive re-ingestion before their prior bound is repaired.
- `fi-doc-entrega` is documented by CVM as delivery metadata, with monthly updates for recent submissions and historical files since 2021. The structured monthly table does not expose a verified join key/semantic contract in this repository. A delivery/receipt timestamp must not be treated as public availability until its join to each report version and its public-availability meaning are verified. The official [dataset page](https://dados.cvm.gov.br/dataset/fi-doc-entrega) describes the available scope.

As-of CVM metrics require `available_at <= as_of`, and report generation now propagates metric query failures rather than silently presenting an empty filing series. Reference periods after the as-of date are excluded. The source revision identifier is exposed in each report record.

## Identity mapping

The stable ID is normalized CNPJ; ISIN maps a COTAHIST security to a fund, while ticker is stored as an observed effective-dated alias. A mapping conflict for an ISIN is left unresolved instead of selecting a CNPJ. Unresolved market observations remain stored and counted.

The report now requires that the ISIN mapping evidence was observed by `as_of` and valid on the observation date, in addition to requiring the market snapshot itself to have been captured by then. CNPJ normalization accepts ASCII alphanumeric identifiers. Remaining gaps: automated CVM aliases do not yet have reliable validity intervals, ISIN syntax/check digits are not validated, ticker changes/reuse lack fixtures, and the historical fund/security universe is incomplete. The 2025 sample validates the named funds only.

## Database and transaction integrity

- B3 observation uniqueness is revision-aware; B3 rows and the successful snapshot status are written in one SQLite transaction. CVM snapshot, identity, metadata, filing revisions, and completion status now share an outer savepoint; corrected archives are append-preserving.
- Source snapshots, identities, metadata, market observations, and filings are stored in separate tables. The new tables do not declare foreign keys, and there is no migration version table. Additive `CREATE TABLE IF NOT EXISTS` does not migrate any future DDL change.
- CVM generic filing queries do not join snapshot status; integrity instead relies on all writes being in the outer savepoint. The implementation now rolls back the snapshot and related rows together when any operation fails.
- B3 source file paths are verified by content; filesystem permissions/immutable storage are not enforced by the application. Treat snapshots as write-once operational artifacts and retain backups.

## Metric correctness and provenance

- CVM NAV/share prefers positive `Valor_Patrimonial_Cotas`, otherwise computes positive `Patrimonio_Liquido / Cotas_Emitidas`. The report now labels the method and emits a reconciliation diagnostic when both methods differ by over 0.1%. This does not independently validate the fallback's units.
- P/NAV aligns a CVM reference month to the last B3 close on or before calendar month-end, within five calendar days, with both source snapshots available by the requested as-of. It does not produce a value when the reporting month-end is later than `as_of`.
- No exchange calendar exists; weekends/holidays are not identified as missing sessions. A five-calendar-day bound is a reviewable heuristic, not a business-day calendar.
- CVM monthly yield is reported as source-reported and is not annualized or treated as cash paid. Raw filing payloads/revision IDs and the aligned market snapshot are preserved. A full structured formula/config version field for P/NAV is still a useful next improvement; the current report carries the policy text.
- The retained `as_of=2026-11-18` report returns 2025-12 P/NAV of about 0.946 for HGLG11, 0.936 for VISC11, and 1.003 for XPLG11. These use 2025-12-30 closes and their latest captured 2025 monthly filings; they are examples of reproducibility, not performance claims.
- No trailing cash yield, distribution growth, income history, total return, or reinvestment-adjusted performance is produced.

## Historical universe and distributions

Current/daily B3 listing data is not a historical constituent or listing-status history. COTAHIST records show securities that traded in the requested archives, not every fund eligible on each historical date. Delisted funds can remain represented if their old observations and identity evidence are retained, but delisting/renaming completeness is not established. IFIX index history does not itself establish historical constituent membership. No survivorship-bias-free analysis is supported.

B3's current UP2DATA Corporate Actions description lists `Rendimento` and `Amortização`, plus schedule, lifecycle, and confirmed-event/update/cancellation files. It also links sample files. However, the public [UP2DATA FAQ](https://b3.com.br/en_us/market-data-and-indices/data-services/up2data/faq/service-and-files.htm) says historical products not listed on the website are offered through authorized distributors. No licensed historical sample, FII-specific event schema/coverage, contract, or retained data was validated in this audit. UP2DATA is a strong official candidate for an access inquiry, not yet a ready historical adapter. The [B3 product description](https://www.b3.com.br/pt_br/market-data-e-indices/servicos-de-dados/up2data/dados-disponiveis/) supports event categories and operational files, not historical completeness by itself.

brapi currently documents FII historical-price and dividend endpoints, including rendimentos/amortizações, while saying FII endpoints are Pro plan. Its [FII documentation](https://brapi.dev/docs/fiis) does not itself establish event publication-time semantics, correction/cancellation completeness, source-record provenance, or local-retention rights. Treat it as a candidate for a licensed empirical evaluation, not as authoritative PIT event data. The [pricing page](https://brapi.dev/pricing) lists tiers but does not resolve those semantic questions.

## Readiness matrix

Only the requested status vocabulary is used.

| Dimension | Assessment |
|---|---|
| COTAHIST acquisition | Partially validated |
| COTAHIST parsing | Partially validated |
| FII classification | Partially validated |
| Historical market prices | Partially validated |
| Source snapshot integrity | Partially validated |
| Archive revision handling | Partially validated |
| CVM monthly-report ingestion | Partially validated |
| CVM point-in-time availability | Partially validated |
| FII identity resolution | Partially validated |
| NAV/share calculation | Partially validated |
| P/NAV calculation | Partially validated |
| Historical distributions | Not implemented |
| Total-return readiness | Not implemented |
| Historical universe | Not validated |
| Backtesting readiness | Not validated |

## Answers for research planning

1. **Historical price and NAV analysis:** yes, for research-grade exploratory work on mapped cohort funds and source snapshots captured by the query date. Not yet a fully validated market-data product or broad-universe dataset.
2. **Before indicators:** verify/report the identity mapping confidence and effective-date workflow; add malformed/null record diagnostics and a fixture end-to-end correction test.
3. **Distributions:** request a licensed B3 UP2DATA Corporate Action sample/schema and historical coverage statement. Compare its event IDs/dates/amounts/corrections against administrator notices. If unavailable, evaluate a paid brapi plan against official samples and obtain retention/licensing terms.
4. **Total return:** dated and classified distribution/corporate-action events, announcement/availability evidence, ex/record/payment dates, corrections/cancellations, and share/price adjustment rules.
5. **Separate indicator framework:** the data boundary is ready for an isolated prototype that consumes explicit snapshots and quality fields. Do not introduce scoring until metric unit semantics and broad identity/coverage are established.
6. **Trusted backtests:** historical universe/listing and delisting records, PIT identity/classification, capture/publication histories, complete corporate actions, trading calendar, stale/missing-price policy, and deterministic end-to-end revision tests.
