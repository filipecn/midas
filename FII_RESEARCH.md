# FII research foundation

## Current coverage

MIDAS has CVM catalog discovery, downloading, ZIP extraction, and CSV parsing.
The FII report adapter discovers monthly, quarterly, annual, and financial
statement resources through CKAN. Typed parsing and ingestion currently cover
the three-table annual monthly report archive only; the other report families
are discoverable/downloadable but do not yet have typed row mappers.

The monthly archive has separate general, asset/liability, and complement CSVs.
The importer preserves source columns and versions, normalizes the fund CNPJ,
and stores rows in the append-preserving filing store. It resolves the three
tables on fund CNPJ, reference date, and version. Metrics expose the source
NAV-per-share and reported monthly distribution-yield fields with diagnostics;
P/NAV is calculated only when an admissible raw market observation is supplied.

`source_data::b3` provides current listed FII identity, a security catalog,
issuer details, and distribution-like detail fields. It does not provide a
persisted historical price/volume series, IFIX history or composition, or a
complete event record with publication time. The general market-data provider
can return price bars, but MIDAS does not yet connect it to FII storage or
define provider-specific adjustment semantics. Fundamentus provides current
FII snapshots. No BCB or IBGE adapter was found.

## Domain and time semantics

`investing::fii` defines source-independent fund identity, time-varying
classification exposure, distributions, separate raw/adjusted market prices,
and filing observations. CVM monthly mapping uses normalized CNPJ as the stable
fund key. The monthly report does not establish a B3 ticker or normalized FII
category, so those are left unset rather than guessed.

`db::fii` stores filings append-only by source revision and filters `as_of`
queries on `available_at`, preserving `reference_date` and `ingested_at`. The
importer uses the CKAN `last_modified` timestamp of the exact annual ZIP as
availability, while retaining row-level `Data_Entrega` in the source payload.
This is conservative for the captured archive snapshot. Today's annual ZIP
cannot reconstruct data that was available before the first snapshot MIDAS
captured, and all rows inside one ZIP share that snapshot availability time.

The database also stores point-in-time market observations and distributions.
Distribution writes require a source publication timestamp; B3's current
detail response does not provide enough timing information to use for
historical queries. Market persistence is ready, but no market provider is
connected to it yet.

## Quantitative research status

The importer currently exposes monthly NAV per share, the source-reported
monthly distribution yield value, and P/NAV when price data is supplied. The
source percentage is kept as reported; the pipeline does not infer its scale.
Missing/invalid fields produce diagnostics. FII percentile normalization also
supports explicit skip/require-all missing-data behavior. There is no complete
indicator registry or FII strategy evaluator. Existing QARP remains
stock-specific and unchanged.

## Next steps

1. Add typed parsers and mappings for quarterly, annual, and financial
   statement datasets, with fixtures for schema revisions.
2. Connect a market-bar provider to the FII market store and verify source,
   adjusted-price, and availability semantics.
3. Add dated B3 distribution/corporate-action ingestion. Calculate total
   returns only when event dates and price adjustment treatment are known.
4. Add classification mapping from validated source labels and create a
   representative Tijolo/Papel/FOF sample, preserving raw labels and unknowns.
5. Extend indicator provenance and calculations, then add factor aggregation.
   Historical universes, portfolio construction, costs, reinvestment, and
   backtesting should follow after the data coverage is established.

Portfolio positions and household investment transactions remain separate
from research strategy scores. Strategy evaluation, portfolio construction,
and backtesting are not implemented by this foundation.
