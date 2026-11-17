use crate::source_data::cvm::{CsvOptions, ResourceParser};
use chrono::NaiveDate;
use serde::Deserialize;
use std::io::Read;

/// Company registration row from the current `cia_aberta-cad` CSV.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CompanyRegistration {
    #[serde(rename = "CNPJ_CIA", default)]
    pub cnpj: String,
    #[serde(rename = "CD_CVM", default)]
    pub cvm_code: String,
    #[serde(rename = "DENOM_SOCIAL", default)]
    pub legal_name: String,
    #[serde(rename = "DENOM_COMERC", default)]
    pub trading_name: Option<String>,
    #[serde(rename = "DT_REG", default)]
    pub registration_date: Option<NaiveDate>,
    #[serde(rename = "DT_CONST", default)]
    pub incorporation_date: Option<NaiveDate>,
    #[serde(rename = "DT_CANCEL", default)]
    pub cancellation_date: Option<NaiveDate>,
    #[serde(rename = "MOTIVO_CANCEL", default)]
    pub cancellation_reason: Option<String>,
    #[serde(rename = "SIT", default)]
    pub registration_status: Option<String>,
    #[serde(rename = "DT_INI_SIT", default)]
    pub status_start_date: Option<NaiveDate>,
    #[serde(rename = "SETOR_ATIV", default)]
    pub activity_sector: Option<String>,
    #[serde(rename = "CATEG_REG", default)]
    pub registration_category: Option<String>,
    #[serde(rename = "SIT_EMISSOR", default)]
    pub issuer_status: Option<String>,
    #[serde(rename = "CONTROLE_ACIONARIO", default)]
    pub ownership_control: Option<String>,
    #[serde(rename = "MUN", default)]
    pub municipality: Option<String>,
    #[serde(rename = "UF", default)]
    pub state: Option<String>,
}

/// Stream typed rows from the CAD registration CSV.
pub fn parse_registration_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, CompanyRegistration> {
    ResourceParser::csv_rows(reader, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_data::cvm::CsvEncoding;

    #[test]
    fn parses_registration_fields_from_a_cad_shaped_csv_fixture() {
        let csv = "CNPJ_CIA;CD_CVM;DENOM_SOCIAL;DENOM_COMERC;DT_REG;SIT\n00.000.000/0001-91;001023;BCO BRASIL S.A.;Banco do Brasil;2000-07-18;ATIVO\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let company = parse_registration_rows(csv.as_bytes(), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(company.cnpj, "00.000.000/0001-91");
        assert_eq!(company.cvm_code, "001023");
        assert_eq!(company.registration_status.as_deref(), Some("ATIVO"));
        assert_eq!(company.registration_date.unwrap().to_string(), "2000-07-18");
    }
}
