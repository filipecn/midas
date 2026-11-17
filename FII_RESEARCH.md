# FII research foundation

## Current coverage

MIDAS has source-specific CVM support in `source_data::cvm::funds` for CAD
registration, daily NAV/quota information, CDA portfolio holdings, balance
accounts, FIDC monthly rows, and generic catalog-backed fund datasets. Typed
coverage is not yet available for the structured monthly, quarterly, and
annual FII reports or fund financial statements. CVM filing rows must retain
their source versions when these are added.

`source_data::b3` provides current listed FII identity, B3 security catalog
mapping, issuer details, and dividend-like cash distribution records. It does
not currently supply a persisted historical price/volume series, IFIX history
or composition, or a fully dated history of corporate actions. Fundamentus
provides current FII snapshots. No BCB or IBGE adapter was found in the
current project.

## Domain and time semantics

`investing::fii` defines source-independent fund identity, time-varying
classification exposure, distributions, distinct raw/adjusted market prices,
and filing observations. Use a stable fund identifier (normally a normalized
CNPJ) and treat ticker as a changeable listing attribute. Source parsers remain
in `source_data`.

`db::fii` stores filing observations append-only by source revision. The
point-in-time query filters on `available_at`, while preserving `reference_date`
and `ingested_at`. Reingesting an identical revision is idempotent; a different
revision is retained. This storage is prepared for ingestion but existing CVM
parsers are not yet wired to it.

## Quantitative research status

The FII module includes a deterministic percentile normalization primitive
with explicit skip/require-all missing-data behavior. It is not yet a complete
indicator registry or strategy evaluator. No FII indicator values are
calculated from incomplete or unavailable source data. Existing QARP remains
stock-specific and unchanged.

## Next steps

1. Add typed, source-preserving parsers for structured FII monthly, quarterly,
   annual, and financial-statement filings; connect them to append-only storage.
2. Add fixture-driven B3 market history, distributions, IFIX composition, and
   macroeconomic source adapters where the official datasets support them.
3. Define FII indicator requirements and provenance, then implement common and
   category-specific historical calculations with missing-data semantics.
4. Add configurable factor aggregation and explainable strategy outputs with
   versioned configurations, without changing stock QARP.
5. Add historical universes, portfolio construction, costs, cash/distribution
   handling, total returns, and backtesting after point-in-time source coverage
   is established.

Portfolio positions and household investment transactions remain separate
from research strategy scores. Portfolio construction and backtesting are not
implemented by this foundation.
