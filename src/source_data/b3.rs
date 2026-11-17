//! Official B3 security listings and ticker-to-issuer relationships.
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    sync::OnceLock,
};

const TITLES_URL: &str = "https://bvmf.bmfbovespa.com.br/suplemento/ExecutaAcaoDownload.asp?arquivo=Titulos_Negociaveis.zip&server=L";
const FII_LIST_URL: &str = "https://sistemaswebb3-listados.b3.com.br/fundsProxy/fundsCall/GetListFundDownload/eyJ0eXBlRnVuZCI6NywicGFnZU51bWJlciI6MSwicGFnZVNpemUiOjIwfQ==";

// Verified issuer-code to CVM-code bridges for cases where public name fields
// differ. Extend this table only with a primary-source checked relationship.
const CVM_CODE_OVERRIDES: &[(&str, &str)] = &[("AZUL", "24112")];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B3Issuer {
    pub code: String,
    pub name: String,
    pub trading_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct B3Security {
    pub ticker: String,
    pub issuer_code: String,
}

#[derive(Debug, Clone, Default)]
pub struct B3SecurityCatalog {
    pub issuers: HashMap<String, B3Issuer>,
    pub securities: HashMap<String, B3Security>,
}

/// A real estate investment fund listed on B3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFii {
    pub ticker: String,
    pub trading_code: Option<String>,
    pub name: String,
    pub legal_name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedFiiDetails {
    #[serde(default)]
    pub quoted_per: String,
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub fund: String,
    #[serde(default)]
    pub ifd: String,
    #[serde(default)]
    pub trading_lot: String,
    #[serde(default)]
    pub quantity: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub code_cvm: Option<String>,
    #[serde(default)]
    pub segment: String,
    #[serde(default)]
    pub cash_dividends: Vec<ListedFiiDividend>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedFiiDividend {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub payment_date: String,
    #[serde(default)]
    pub rate: String,
    #[serde(default)]
    pub related_to: String,
    #[serde(default)]
    pub isin_code: String,
}

pub fn listed_fii_details(ticker: &str) -> Result<ListedFiiDetails, String> {
    use base64::Engine;

    let payload = serde_json::json!({ "identifierFund": ticker }).to_string();
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let url = format!(
        "https://sistemaswebb3-listados.b3.com.br/fundsProxy/fundsCall/GetListedSupplementFunds/{encoded}"
    );
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    response.json().map_err(|error| error.to_string())
}

/// Parse the CSV exported by B3's "Exportar lista completa de fundos" action.
pub fn parse_listed_fiis(reader: impl Read) -> Result<Vec<ListedFii>, String> {
    let mut bytes = Vec::new();
    let mut reader = reader;
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(&bytes).0.into_owned(),
    };
    let delimiter = if text
        .lines()
        .next()
        .is_some_and(|line| line.matches(';').count() > line.matches(',').count())
    {
        b';'
    } else {
        b','
    };
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = csv.headers().map_err(|error| error.to_string())?.clone();
    if headers.is_empty() {
        return Err("B3 returned an empty FII CSV response".into());
    }
    let ticker_column = headers
        .iter()
        .position(|header| {
            let header = normalize_header(header);
            header.contains("codigo") || header.contains("ticker") || header.contains("negociacao")
        })
        .ok_or_else(|| format!("B3 FII CSV has no ticker column: {headers:?}"))?;
    let name_column = headers
        .iter()
        .position(|header| normalize_header(header) == "fundo")
        .or_else(|| {
            headers.iter().position(|header| {
                let header = normalize_header(header);
                header.contains("nome") || header.contains("razao social")
            })
        })
        .ok_or_else(|| format!("B3 FII CSV has no fund-name column: {headers:?}"))?;
    let legal_name_column = headers
        .iter()
        .position(|header| normalize_header(header) == "razao social");
    let mut funds = Vec::new();
    for row in csv.records() {
        let row = row.map_err(|error| error.to_string())?;
        let ticker = row
            .get(ticker_column)
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        let name = row.get(name_column).unwrap_or_default().trim().to_owned();
        let legal_name = legal_name_column
            .and_then(|column| row.get(column))
            .unwrap_or(&name)
            .trim()
            .to_owned();
        if !ticker.is_empty() && !name.is_empty() {
            funds.push(ListedFii {
                ticker,
                trading_code: None,
                name,
                legal_name,
            });
        }
    }
    if funds.is_empty() {
        return Err("B3 FII CSV contained no fund records".into());
    }
    funds.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    funds.dedup_by(|a, b| a.ticker == b.ticker);
    Ok(funds)
}

fn normalize_header(value: &str) -> String {
    normalize_header_ascii(value)
}

fn normalize_header_ascii(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\u{00e1}' | '\u{00e0}' | '\u{00e2}' | '\u{00e3}' | '\u{00e4}' => Some('a'),
            '\u{00e9}' | '\u{00e8}' | '\u{00ea}' | '\u{00eb}' => Some('e'),
            '\u{00ed}' | '\u{00ec}' | '\u{00ee}' | '\u{00ef}' => Some('i'),
            '\u{00f3}' | '\u{00f2}' | '\u{00f4}' | '\u{00f5}' | '\u{00f6}' => Some('o'),
            '\u{00fa}' | '\u{00f9}' | '\u{00fb}' | '\u{00fc}' => Some('u'),
            '\u{00e7}' => Some('c'),
            other if other.is_ascii_alphanumeric() => Some(other.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// Fetch B3's complete list of listed FIIs.
pub fn listed_fiis() -> Result<Vec<ListedFii>, String> {
    static FIIS: OnceLock<Vec<ListedFii>> = OnceLock::new();
    if let Some(funds) = FIIS.get() {
        return Ok(funds.clone());
    }

    let mut funds = load_listed_fiis()?;
    add_trading_codes(&mut funds);
    let _ = FIIS.set(funds.clone());
    Ok(funds)
}

fn load_listed_fiis() -> Result<Vec<ListedFii>, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("midas-b3-fiis-{}-{stamp}", std::process::id()));
    let csv_result = (|| {
        let path = directory.join("listed-fiis.csv");
        crate::source_data::cvm::ResourceDownloader::default()
            .download_url(FII_LIST_URL, &path, false)
            .map_err(|error| error.to_string())?;
        parse_listed_fiis(std::fs::File::open(path).map_err(|error| error.to_string())?)
    })();
    let _ = std::fs::remove_dir_all(directory);
    csv_result.or_else(|csv_error| {
        parse_listed_fiis(std::io::Cursor::new(include_bytes!(
            "../../fundosListados.csv"
        )))
        .or_else(|_| listed_fiis_from_security_catalog())
        .map_err(|fallback_error| {
            format!("B3 FII export failed ({csv_error}); fallback failed ({fallback_error})")
        })
    })
}

fn listed_fiis_from_security_catalog() -> Result<Vec<ListedFii>, String> {
    let catalog = security_catalog()?;
    let mut funds = catalog
        .securities
        .keys()
        .filter_map(|ticker| {
            let issuer = catalog.issuer_for_ticker(ticker)?;
            let name = issuer.name.trim();
            let normalized = normalize_name(name);
            (ticker.ends_with("11")
                && normalized.contains("FUNDO")
                && normalized.contains("IMOBILIARIO"))
            .then(|| ListedFii {
                ticker: ticker.clone(),
                trading_code: Some(ticker.clone()),
                name: name.to_owned(),
                legal_name: name.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    funds.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    funds.dedup_by(|a, b| a.ticker == b.ticker);
    if funds.is_empty() {
        return Err("B3 securities catalog contained no identifiable listed FIIs".into());
    }
    Ok(funds)
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\u{00e1}' | '\u{00e0}' | '\u{00e2}' | '\u{00e3}' | '\u{00e4}' | '\u{00c1}'
            | '\u{00c0}' | '\u{00c2}' | '\u{00c3}' | '\u{00c4}' => Some('A'),
            '\u{00e9}' | '\u{00e8}' | '\u{00ea}' | '\u{00eb}' | '\u{00c9}' | '\u{00c8}'
            | '\u{00ca}' | '\u{00cb}' => Some('E'),
            '\u{00ed}' | '\u{00ec}' | '\u{00ee}' | '\u{00ef}' | '\u{00cd}' | '\u{00cc}'
            | '\u{00ce}' | '\u{00cf}' => Some('I'),
            '\u{00f3}' | '\u{00f2}' | '\u{00f4}' | '\u{00f5}' | '\u{00f6}' | '\u{00d3}'
            | '\u{00d2}' | '\u{00d4}' | '\u{00d5}' | '\u{00d6}' => Some('O'),
            '\u{00fa}' | '\u{00f9}' | '\u{00fb}' | '\u{00fc}' | '\u{00da}' | '\u{00d9}'
            | '\u{00db}' | '\u{00dc}' => Some('U'),
            '\u{00e7}' | '\u{00c7}' => Some('C'),
            other if other.is_ascii_alphanumeric() => Some(other.to_ascii_uppercase()),
            _ => None,
        })
        .collect()
}

fn add_trading_codes(funds: &mut [ListedFii]) {
    let Ok(catalog) = security_catalog() else {
        return;
    };
    for fund in funds {
        let legal_name = normalize_name(&fund.legal_name);
        let preferred = format!("{}11", fund.ticker);
        let mut candidates = catalog
            .securities
            .values()
            .filter_map(|security| {
                if !security.ticker.ends_with("11") {
                    return None;
                }
                let issuer = catalog.issuers.get(&security.issuer_code)?;
                (normalize_name(&issuer.name) == legal_name).then_some(security.ticker.as_str())
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|ticker| (*ticker != preferred.as_str(), ticker.len()));
        fund.trading_code = candidates
            .first()
            .map(|ticker| (*ticker).to_owned())
            .or_else(|| {
                catalog
                    .securities
                    .contains_key(&preferred)
                    .then_some(preferred)
            });
    }
}

impl B3SecurityCatalog {
    pub fn issuer_for_ticker(&self, ticker: &str) -> Option<&B3Issuer> {
        let normalized = ticker.trim().to_ascii_uppercase();
        let security = self.securities.get(&normalized)?;
        self.issuers.get(&security.issuer_code)
    }

    pub fn cvm_code_override(&self, issuer_code: &str) -> Option<&'static str> {
        CVM_CODE_OVERRIDES
            .iter()
            .find(|(b3_code, _)| *b3_code == issuer_code)
            .map(|(_, cvm_code)| *cvm_code)
    }
}

/// Parse the fixed-width `TITULOS_NEGOCIAVEIS.txt` file inside B3's ZIP.
pub fn parse_titulos_negociaveis(reader: impl Read) -> std::io::Result<B3SecurityCatalog> {
    let mut catalog = B3SecurityCatalog::default();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let line = line.strip_suffix(b"\n").unwrap_or(&line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.len() < 6 {
            continue;
        }
        match &line[..2] {
            b"01" if line.len() >= 126 => {
                let code = String::from_utf8_lossy(&line[2..6]).trim().to_owned();
                let name = decode_b3_text(&line[6..66]);
                let trading_name = decode_b3_text(&line[66..126]);
                if !code.is_empty() && !name.is_empty() {
                    catalog.issuers.insert(
                        code.clone(),
                        B3Issuer {
                            code,
                            name,
                            trading_name,
                        },
                    );
                }
            }
            b"02" if line.len() >= 18 => {
                let ticker = String::from_utf8_lossy(&line[2..14])
                    .trim()
                    .to_ascii_uppercase();
                let issuer_code = String::from_utf8_lossy(&line[14..18]).trim().to_owned();
                if !ticker.is_empty() && !issuer_code.is_empty() {
                    catalog
                        .securities
                        .entry(ticker.clone())
                        .or_insert(B3Security {
                            ticker,
                            issuer_code,
                        });
                }
            }
            _ => {}
        }
    }
    Ok(catalog)
}

fn decode_b3_text(bytes: &[u8]) -> String {
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    text.trim().to_owned()
}

/// Load and process B3's public securities file once per process.
pub fn security_catalog() -> Result<B3SecurityCatalog, String> {
    static CATALOG: OnceLock<Result<B3SecurityCatalog, String>> = OnceLock::new();
    CATALOG.get_or_init(load_security_catalog).clone()
}

fn load_security_catalog() -> Result<B3SecurityCatalog, String> {
    use crate::source_data::cvm::{ResourceDownloader, extract_zip};

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("midas-b3-listings-{}-{stamp}", std::process::id()));
    let result = (|| {
        let zip_path = directory.join("titulos-negociaveis.zip");
        ResourceDownloader::default()
            .download_url(TITLES_URL, &zip_path, false)
            .map_err(|error| error.to_string())?;
        let extracted = extract_zip(&zip_path, directory.join("extracted"))
            .map_err(|error| error.to_string())?;
        let file = extracted
            .files
            .into_iter()
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .eq_ignore_ascii_case("TITULOS_NEGOCIAVEIS.txt")
                })
            })
            .ok_or_else(|| {
                "B3 securities ZIP did not contain TITULOS_NEGOCIAVEIS.txt".to_string()
            })?;
        let file = std::fs::File::open(file).map_err(|error| error.to_string())?;
        parse_titulos_negociaveis(file).map_err(|error| error.to_string())
    })();
    let _ = std::fs::remove_dir_all(directory);
    result
}

/// Normalize an issuer display/legal name for exact alias matching.
pub fn normalize_issuer_name(value: &str) -> String {
    value.chars().fold(String::new(), |mut output, character| {
        let upper = character.to_uppercase().next().unwrap_or(character);
        let base = match upper {
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' => 'A',
            'Ç' => 'C',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'Ñ' => 'N',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            other => other,
        };
        if base.is_ascii_alphanumeric() {
            output.push(base);
        }
        output
    })
}
