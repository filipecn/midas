# MIDAS — Historical FII Source Feasibility

Assessment date: 2026-11-18. This is a source investigation and readiness report, not a pipeline implementation. It combines source documentation, repository inspection, and limited live samples. Sample results establish reachability and field shape, not complete history or contractual production rights.

## Findings at a glance

| Need | Best evidenced path | Readiness |
|---|---|---|
| FII monthly reports and fund identity | CVM structured FII monthly archives | Usable for reported monthly facts and CNPJ/ISIN mapping. The row's `Data_Entrega` is date-only; it is not an exact public-availability timestamp. |
| Filing delivery timestamps | CVM `fi-doc-entrega` metadata | Schema provides `Data_Hora_Entrega` to milliseconds, document ID/type, CNPJ and competence dates. A reliable join to the FII monthly rows was not established in the sampled records. |
| Raw historical daily prices | B3 COTAHIST | Official historical quotes, unadjusted. Public materials describe downloadable files, not a documented open API or a production automation license. Obtain/confirm the applicable license and delivery method before automating or redistributing. |
| Distribution event history | B3 UP2DATA Corporate Action | Official commercial data product documents Rendimento and Amortização event types, lifecycle and cancellations. It is the strongest official path found, subject to license, schema/sample access, and historical coverage confirmation. |
| Low-friction third-party sample | brapi FII API | HGLG11 price and dividend calls worked unauthenticated; XPLG11 and VISC11 returned HTTP 401. The docs say expanded FII access is Pro. Not a viable no-cost 3–5-fund source based on this sample. |

## What was inspected

The repository already has a CVM FII monthly report parser in `src/source_data/cvm/fii_reports.rs`, a B3 current FII supplement client in `src/source_data/b3.rs`, and FII storage/pipeline code in `src/investing/fii.rs`, `src/investing/fii_pipeline.rs`, and `src/db/fii.rs`. This audit did not modify those code paths.

The monthly CVM report rows carry fields including fund CNPJ, reference month, version, `Data_Entrega`, ISIN, and segment. They do not expose `ID_Documento` from the separate delivery dataset. The report's current PIT handling uses CKAN resource `last_modified` as an availability signal; for an annual ZIP this is a resource-level timestamp and cannot be assigned to each historical row as its original publication time.

## CVM report and delivery metadata

The official [CVM FII monthly dataset](https://dados.cvm.gov.br/dataset/fii-doc-inf_mensal) contains structured monthly reports. For the inspected 2026 archive, records were found for:

| Ticker candidate | CNPJ | ISIN | CVM reported segment | Sample coverage |
|---|---|---|---|---|
| HGLG11 | 11.728.688/0001-47 | BRHGLGCTF004 | Multicategoria | Jan–Oct 2026 |
| XPLG11 | 26.502.794/0001-85 | BRXPLGCTF002 | Logística | Jan–Oct 2026 |
| VISC11 | 17.554.274/0001-25 | BRVISCCTF005 | Shoppings | Jan–Oct 2026 |

The sampled archive also includes version changes: HGLG has v2 for April and August; XPLG has v2 for March; VISC has v2 for January and February. This demonstrates why a single final archive row cannot represent what was known at each historical cutoff.

The separate official [CVM document delivery dataset](https://dados.cvm.gov.br/dataset/fi-doc-entrega) describes a recent rolling set of monthly files (with the latest months updated on a schedule) and historic annual ZIPs from 2021. Its dictionary includes `CNPJ_Fundo_Classe`, `ID_Documento`, `Tipo_Documento`, `Data_Inicio_Competencia`, `Data_Fim_Competencia`, and `Data_Hora_Entrega` (23-character datetime, millisecond precision). This is promising delivery metadata, but the field's semantic meaning still needs to be confirmed as the time the filing became publicly available, rather than merely a system receipt time.

I sampled the November 2026 delivery file against the three CNPJs above and found no matching rows. That is not proof that no join exists: the sampled file may not include these report types or filing system, and the FII monthly rows do not carry the delivery dataset's document ID. A join would likely need CNPJ + document type + competence dates, and could be ambiguous across multiple versions. Therefore the exact delivery-time join remains **unverified**. `Data_Entrega` is only a date and must not be promoted to a timestamp.

**PIT consequence:** use a verified per-document publication timestamp only after matching and validating its semantics. Until then, the defensible availability time is MIDAS's own immutable retrieval/capture timestamp. CKAN archive `last_modified` may describe the archive resource's update, not when a particular row/version first became available. Backfilled rows cannot support a strict historical “MIDAS knew this then” claim before their first captured snapshot.

## Price history: B3

B3's [historical quote documentation](https://www.b3.com.br/en_us/market-data-and-indices/data-services/market-data/historical-data/equities/historical-quote-data/) describes daily historical quotes since 1986 and states that the prices are not adjusted for corporate actions or inflation. The [layout specification](https://www.b3.com.br/data/files/65/50/AD/26/29C8B51095EE46B5790D8AA8/HistoricalQuotations_B3.pdf) defines a fixed-width record with ticker/negotiation code, ISIN, BDI segment code (including real estate funds), prices, trades and volume/value fields.

This is suitable evidence for an official **raw-price** history, not total-return or distribution-adjusted prices. Preserve the source rows and document corporate-action treatment separately. The material reviewed describes a manual downloadable historical-data product; no documented open automated API was found in this audit. B3's data terms restrict commercial use and redistribution without authorization, and its public-data hub warns against automated collection. Those terms do not establish that a local research download is licensed for unattended production ingestion. Confirm delivery and usage rights with B3 before automating, retaining for a commercial product, or redistributing the data.

No automated COTAHIST download or parsed price sample was performed, so sample-level coverage and FII row parsing remain to be proven. The B3 current supplement client in the repository is a different source path and its displayed fields do not supply a historical PIT event stream.

## Distribution events: official B3 product and existing endpoint

B3's commercial [UP2DATA Corporate Action product](https://www.b3.com.br/en_us/market-data-and-indices/data-services/up2data/data-products/corporate-action/) describes Schedule and Lifecycle/confirmed-event data, updates and cancellations. The published supported event types include **Rendimento** and **Amortização**, which are directly relevant to FIIs. B3 says its event data is derived from issuer communications and organized through the corporate-action lifecycle. This makes it the best official distribution-event source identified in the audit, but the documentation reviewed does not settle the FII universe's historical depth, exact event identifiers, fields for announcement/approval/ex/payment dates, correction semantics, or licensing for MIDAS. Those must be validated from a licensed sample/schema before choosing the point-in-time contract.

The repository's [B3 supplement client](src/source_data/b3.rs) models a cash dividend with label, payment date, rate, relation and ISIN. It has no source event ID, announcement/availability timestamp, ex-date, version/cancellation status, or correction lineage. A direct HGLG request in this environment yielded an empty response, so no claim of complete historical coverage is made for that endpoint.

## Third-party probe: brapi

The [brapi FII documentation](https://brapi.dev/docs/fiis) documents dedicated endpoints for historical prices and dividends, and says only MXRF11 and HGLG11 are available without a token; broader FII endpoint access requires Pro. This resolves the earlier access question: **for the 3–5-fund cohort, brapi requires a paid Pro subscription or a token with the applicable access.** A no-token HGLG-only pilot does not demonstrate cohort feasibility.

Unauthenticated live calls on 2026-11-18 returned:

| Candidate | Historical price endpoint | Dividend endpoint |
|---|---|---|
| HGLG11 | 258 daily rows, 2025-11-03 through 2026-11-17 | 13 rows, 2025-11-14 through 2026-11-16 |
| XPLG11 | HTTP 401 | HTTP 401 |
| VISC11 | HTTP 401 | HTTP 401 |

One HGLG dividend row was `R$1.17`, relation `OUTUBRO`, payment date 2026-11-16, ISIN `BRHGLGCTF004`, `approvedOn: null`, and `exDate: null`; the response marked it `verified: true`. The response's `requestedAt` was 2026-11-18T12:14:55Z. The endpoint therefore returned a useful current sample but not an announcement timestamp for that event. `requestedAt` is the provider response/capture time and cannot establish when the event became knowable. The meanings of `verified` and provider correction history were not established by the inspected docs/sample. Price history was only about one year in this probe, not evidence of full fund history.

brapi can be considered a licensed, convenience source after price/terms and data semantics are checked, with a point-in-time capture model of its own. It should not be treated as an official record or assumed to provide publication-time history. Cross-validation against administrator notices was not completed in this audit.

## Recommended data contract and next evidence gate

1. Keep separate fields for event/reference date, source publication/announcement time, source capture time, and local ingestion time. Never substitute a payment date or report competence date for availability.
2. For CVM rows, retain report version and original payload/snapshot. Use `Data_Entrega` as a date-level filing attribute; use exact `Data_Hora_Entrega` only after a successful document-level join and semantic validation. Otherwise gate historical availability at the first MIDAS snapshot.
3. For prices, preserve raw OHLCV, ISIN and source snapshot metadata. Do not label the series adjusted or total-return unless the source explicitly defines that treatment. Resolve B3 license and automated delivery rights before a production COTAHIST loader.
4. For distributions, request a B3 UP2DATA Corporate Action sample/schema and terms covering FII Rendimento/Amortização, event IDs, initial schedule, revisions, cancellations, date fields, and back history. That is the strongest official source route.
5. If evaluating brapi as a fallback, obtain Pro access for all selected tickers and validate event history against administrator/B3 announcements. Preserve every response as an immutable snapshot; do not backdate from event dates.
6. Freeze a 3–5-fund cohort only after joining ticker → ISIN → CNPJ with source evidence and measuring price, report, and event coverage for every member.

## References

- [CVM FII monthly reports](https://dados.cvm.gov.br/dataset/fii-doc-inf_mensal)
- [CVM fund document delivery metadata](https://dados.cvm.gov.br/dataset/fi-doc-entrega)
- [B3 historical quotation data](https://www.b3.com.br/en_us/market-data-and-indices/data-services/market-data/historical-data/equities/historical-quote-data/)
- [B3 historical quotation layout](https://www.b3.com.br/data/files/65/50/AD/26/29C8B51095EE46B5790D8AA8/HistoricalQuotations_B3.pdf)
- [B3 UP2DATA Corporate Action](https://www.b3.com.br/en_us/market-data-and-indices/data-services/up2data/data-products/corporate-action/)
- [brapi FII API](https://brapi.dev/docs/fiis)
- [brapi historical FII prices](https://brapi.dev/docs/fiis/historico-precos)
- [brapi FII dividends](https://brapi.dev/docs/fiis/dividendos)
