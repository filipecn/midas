# CVM data source (Phase 1)

MIDAS exposes the CVM open-data CKAN catalog through `source_data::cvm`. The
catalog remains authoritative: dataset identifiers and resource URLs are
discovered at runtime, and unrecognized dataset/resource metadata is retained
in each model's `extra` map.

## Discover datasets and resources

```rust,ignore
use crate::source_data::cvm::{CkanClient, CkanConfig};

let cvm = CkanClient::new(CkanConfig::default())?;
let names = cvm.list_datasets()?;
let page = cvm.search_datasets("companhias abertas", 0, 20)?;
let itr = cvm.dataset("cia_aberta-doc-itr")?;
for resource in &itr.resources {
    println!("{} {:?} {}", resource.id, resource.resource_format(), resource.url);
}
let resource = cvm.resource(&itr.resources[0].id)?;
```

`search_datasets` supports CKAN offset/limit pagination. `dataset` and
`resource` call `package_show` and `resource_show`; `datastore_search` is
available for resources that CKAN has imported into DataStore.

## FII periodic reports

`source_data::cvm::fii_reports` discovers FII monthly, quarterly, annual, and
financial-statement resources from CKAN. `FiiMonthlyArchive` currently parses
the three annual monthly-report tables (general, assets/liabilities, and
complement). It retains unknown columns and keeps numeric source cells as
their original text, so precision and formatting are not lost during parsing.

`FiiReportDataSource::download_snapshot` returns the exact resource's CKAN
`last_modified` timestamp alongside the download. The monthly ingestion path
uses that archive snapshot time as `available_at`; row-level `Data_Entrega` is
retained as reported source data. Since CVM republishes annual archives,
point-in-time history before MIDAS captures a particular archive revision is
not available from the current download alone.

## Download, extract, and parse

```rust,ignore
use crate::source_data::cvm::{
    CsvEncoding, CsvOptions, ResourceDownloader, ResourceParser, extract_zip,
};
use serde::Deserialize;
use std::path::Path;

let downloader = ResourceDownloader::default();
let downloaded = downloader.download(&resource, "data/cvm/itr.zip", false)?;
let extracted = extract_zip(&downloaded.path, "data/cvm/itr")?;

#[derive(Deserialize)]
struct ExampleRow { #[serde(rename = "CD_CVM")] company_code: String }

let file = std::fs::File::open(Path::new("data/cvm/sample.csv"))?;
let options = CsvOptions { encoding: CsvEncoding::Windows1252, delimiter: b';', has_headers: true };
for row in ResourceParser::csv_rows::<_, ExampleRow>(file, options) {
    let row = row?;
    println!("{}", row.company_code);
}
```

Downloads stream to a `.part` file and are renamed after success. Existing
nonempty files are reused unless `force` is true. ZIP extraction rejects path
traversal and symlink entries, and leaves the original archive in place. CSV
rows and JSON documents deserialize into caller-provided types. CSV encoding
and delimiter are explicit because CVM resource schemas and encodings differ.

## Phase 2: open-company information

`source_data::cvm::companies` provides `CompanyRegistration`,
`ItrStatementRow`, and `DfpStatementRow`, plus `CompanyDataSource` methods that
resolve the current CAD CSV and year-specific ITR/DFP ZIP resources through
CKAN metadata. Resource URLs are not embedded in code.

```rust,ignore
use crate::source_data::cvm::{
    CkanClient, CsvEncoding, CsvOptions, ResourceDownloader, extract_zip,
};
use crate::source_data::cvm::companies::{
    CompanyDataSource, parse_registration_rows, parse_itr_rows, StatementKind,
};
use crate::investing::cvm::itr_rows_for_company;
use std::fs::File;

let ckan = CkanClient::with_defaults()?;
let downloads = ResourceDownloader::default();
let companies = CompanyDataSource::new(&ckan, &downloads);
let options = CsvOptions {
    encoding: CsvEncoding::Windows1252,
    delimiter: b';',
    has_headers: true,
};

// An investing feature can select a company from the streaming CAD rows.
let registration = companies.download_registration("data/cvm/cad.csv", false)?;
let company = parse_registration_rows(File::open(registration.path)?, options)
    .find_map(|row| row.ok().filter(|company| company.cnpj == "00.000.000/0001-91"));

// Retrieve and filter one company's ITR account rows.
let itr = companies.download_itr(2026, "data/cvm/itr_2026.zip", false)?;
let files = extract_zip(itr.path, "data/cvm/itr_2026")?.files;
let bpa = files.iter().find(|path| {
    path.file_name().is_some_and(|name| name.to_string_lossy().contains("BPA_con_2026.csv"))
}).unwrap();
let statement_rows = itr_rows_for_company(
    File::open(bpa)?,
    options,
    &company.as_ref().unwrap().cnpj,
)
    .filter_map(Result::ok)
    .filter(|row| row.kind() == StatementKind::Bpa)
    .collect::<Vec<_>>();
```

ITR and DFP rows retain `DT_REFER`, each row's `DT_FIM_EXERC`, `VERSAO`, the
individual or consolidated `GRUPO_DFP`, account code/description, fixed-account
flag, currency/scale, and an exact base-10 account value. Rows are not
deduplicated, so filing versions and restatements remain available. The
optional `DT_RECEB` field is absent from the actual BPA resource schema;
`DT_REFER` is preserved separately as the CVM reference date and is not
relabeled as a submission date. CKAN resource update metadata and downloaded
archives remain available separately.

The opt-in live test validates the current CAD, ITR, and DFP resources,
including individual and consolidated BPA rows. It downloads and extracts the
official ZIPs and parses their dates, version, scope, statement type, and
numeric values.

## Phase 3: corporate disclosures

`CompanyDisclosureSource` resolves yearly IPE, FRE, and FCA resources from
CKAN. `IpeDocument`, `FreDocument`, and `FcaDocument` preserve company
identifiers, reference and submission dates, categories, versions, document
identifiers, and document links. Linked document downloads use the shared
streaming downloader. IPE metadata classifies Fatos Relevantes and Comunicados
ao Mercado while retaining other categories as `Other`.

The opt-in live validation downloads the official IPE/FRE/FCA archives, parses
metadata, and retrieves linked documents from each filing family.

The market details floating window includes a **Material Fact** tab. It maps
the selected ticker through B3's official negotiable-securities file to the
issuer, then resolves that issuer against the CVM registration file by
normalized legal/trading name or a verified CVM-code override. It then shows
the current year's IPE material-fact subjects and dates, newest first. An
unresolved ticker/issuer is reported instead of being guessed from a fuzzy
company-name match.

The **Investments → News** view lists up to 250 newest Material Fact records
from the current year's IPE archive across companies, with their submission
date, company name, and subject. Type in the company filter to narrow the list;
press `Tab` to move focus between the filter and list navigation.

## Phase 4: investment funds

`source_data::cvm::funds` offers CKAN-backed resource lookup and typed parsers
for fund registration (CAD), daily information, CDA portfolio holdings,
balancete account rows, and FIDC monthly tables. Profile, eventual-event,
extract, and other evolving table feeds can be read through
`FundDatasetRow`, which preserves unmodeled official columns. `FundDataSource`
resolves resources by dataset, year, and month and downloads the selected CSV
or ZIP. Fund/class CNPJs remain distinct from registration CNPJs, and
financial values remain strings to preserve the source's decimal precision.
The investing adapters `fund_daily_rows_for_cnpj` and
`fund_portfolio_rows_for_cnpj` stream rows for a selected class.

```rust,ignore
use crate::source_data::cvm::{CkanClient, CsvEncoding, CsvOptions, ResourceDownloader, extract_zip};
use crate::source_data::cvm::funds::{FundDataSource, FundDataset, parse_fund_daily_rows};
use crate::investing::cvm::fund_daily_rows_for_cnpj;
use std::fs::File;

let ckan = CkanClient::with_defaults()?;
let downloads = ResourceDownloader::default();
let funds = FundDataSource::new(&ckan, &downloads);
let zip = funds.download(FundDataset::Daily, Some(2026), Some(11), "data/cvm/daily.zip", false)?;
let csv_path = extract_zip(zip.path, "data/cvm/daily")?.files.remove(0);
let options = CsvOptions { encoding: CsvEncoding::Windows1252, delimiter: b';', has_headers: true };
let observations = fund_daily_rows_for_cnpj(
    File::open(csv_path)?, options, "00.017.024/0001-53",
).collect::<Result<Vec<_>, _>>()?;
```

The opt-in live fund validation covers actual CAD, daily, CDA, balancete, and
FIDC monthly files. The catalog also lists other fund feeds (including
profile, event, extract, delivery, complement, and factsheet resources); their
schemas continue to be discoverable and parseable generically while typed
coverage is added as investment use cases require it.

## Phase 5: remaining catalog datasets

The underlying CKAN client can discover and search the full live catalog,
retrieve any package/resource metadata, query DataStore records where
available, download resources, safely extract ZIP files, and stream generic
CSV/JSON content. This makes unsupported packages accessible without adding
dataset-specific URLs or blocking future typed models on shared infrastructure.

## Validation scope

Phases 1 through 4 provide generic CKAN access and typed support for open
company financials, disclosures, and the main investment-fund datasets. Phase
5 retains generic access to all other discovered catalog resources; these do
not yet have dedicated typed models.

For optional online validation, set `CVM_LIVE_TESTS=1` when running the CVM
integration test. Ordinary unit tests use local fixtures and do not require
network access.
