use serde::{Deserialize, Deserializer};
use std::fmt;
use std::str::FromStr;

/// Base-10 decimal represented exactly as an integer coefficient and scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CvmDecimal {
    pub coefficient: i128,
    pub scale: u32,
}

impl FromStr for CvmDecimal {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let mut value = raw.trim().replace(' ', "");
        if value.is_empty() {
            return Err("empty decimal".into());
        }
        let decimal_separator = match (value.rfind('.'), value.rfind(',')) {
            (Some(dot), Some(comma)) => Some(if dot > comma { '.' } else { ',' }),
            (Some(_), None) => Some('.'),
            (None, Some(_)) => Some(','),
            (None, None) => None,
        };
        if let Some(separator) = decimal_separator {
            let index = value.rfind(separator).expect("separator found");
            let fraction = value[index + 1..].to_owned();
            let integer: String = value[..index]
                .chars()
                .filter(|character| *character != '.' && *character != ',')
                .collect();
            value = format!("{integer}.{fraction}");
        }
        let negative = value.starts_with('-');
        let unsigned = value.trim_start_matches(['-', '+']);
        let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let digits = format!("{integer}{fraction}");
        let mut coefficient = digits.parse::<i128>().map_err(|error| error.to_string())?;
        if negative {
            coefficient = -coefficient;
        }
        Ok(Self {
            coefficient,
            scale: fraction.len() as u32,
        })
    }
}

impl<'de> Deserialize<'de> for CvmDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for CvmDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.coefficient < 0;
        let digits = self.coefficient.unsigned_abs().to_string();
        let scale = self.scale as usize;
        if scale == 0 {
            return if negative {
                write!(f, "-{digits}")
            } else {
                write!(f, "{digits}")
            };
        }
        let padded = format!("{:0>width$}", digits, width = scale + 1);
        let split = padded.len() - scale;
        write!(
            f,
            "{}{}.{}",
            if negative { "-" } else { "" },
            &padded[..split],
            &padded[split..]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementScope {
    Individual,
    Consolidated,
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Bpa,
    Bpp,
    Dre,
    DfcDirect,
    DfcIndirect,
    Dmpl,
    Dra,
    Dva,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExerciseOrder {
    Current,
    Previous,
    Unknown,
}

impl ExerciseOrder {
    pub fn from_cvm(raw: &str) -> Self {
        let normalized = raw.trim().to_uppercase();
        if normalized.contains("LTIMO") {
            if normalized.contains("PEN") {
                Self::Previous
            } else {
                Self::Current
            }
        } else {
            Self::Unknown
        }
    }
}

pub fn statement_scope(raw: &str) -> StatementScope {
    let value = raw.to_ascii_lowercase();
    if value.contains("consolidado") {
        StatementScope::Consolidated
    } else if value.contains("individual") {
        StatementScope::Individual
    } else {
        StatementScope::Unspecified
    }
}

pub fn statement_kind(raw: &str) -> StatementKind {
    let value = raw.to_uppercase();
    if value.contains("BPA") || (value.contains("BALAN") && value.contains("PATRIMONIAL ATIVO")) {
        StatementKind::Bpa
    } else if value.contains("BPP")
        || (value.contains("BALAN") && value.contains("PATRIMONIAL PASSIVO"))
    {
        StatementKind::Bpp
    } else if value.contains("DFC-MD")
        || value.contains("DFC MD")
        || (value.contains("FLUXO") && value.contains("DIRETO") && !value.contains("INDIRETO"))
    {
        StatementKind::DfcDirect
    } else if value.contains("DFC-MI")
        || value.contains("DFC MI")
        || (value.contains("FLUXO") && value.contains("INDIRETO"))
    {
        StatementKind::DfcIndirect
    } else if value.contains("DMPL") || (value.contains("MUTA") && value.contains("PATRIM")) {
        StatementKind::Dmpl
    } else if value.contains("DRA") || value.contains("RESULTADO ABRANGENTE") {
        StatementKind::Dra
    } else if value.contains("DVA") || value.contains("VALOR ADICIONADO") {
        StatementKind::Dva
    } else if value.contains("DRE")
        || (value.contains("RESULTADO") && !value.contains("ABRANGENTE"))
    {
        StatementKind::Dre
    } else {
        StatementKind::Unknown
    }
}

macro_rules! statement_row {
    ($name:ident) => {
        /// One original row from the corresponding CVM financial statement CSV.
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
        pub struct $name {
            #[serde(rename = "CNPJ_CIA", default)]
            pub cnpj: String,
            #[serde(rename = "CD_CVM", default)]
            pub cvm_code: String,
            #[serde(rename = "DENOM_CIA", default)]
            pub company_name: String,
            #[serde(rename = "DT_REFER", default)]
            pub reference_date: Option<chrono::NaiveDate>,
            #[serde(rename = "VERSAO", default)]
            pub filing_version: Option<u32>,
            #[serde(rename = "GRUPO_DFP", default)]
            pub statement_group: String,
            #[serde(rename = "MOEDA", default)]
            pub currency: Option<String>,
            #[serde(rename = "ESCALA_MOEDA", default)]
            pub currency_scale: Option<String>,
            #[serde(rename = "ORDEM_EXERC", default)]
            pub exercise_order_raw: Option<String>,
            #[serde(rename = "DT_FIM_EXERC", default)]
            pub financial_period_end: Option<chrono::NaiveDate>,
            #[serde(rename = "CD_CONTA", default)]
            pub account_code: String,
            #[serde(rename = "DS_CONTA", default)]
            pub account_description: String,
            #[serde(rename = "VL_CONTA", default)]
            pub account_value: Option<CvmDecimal>,
            #[serde(rename = "ST_CONTA_FIXA", default)]
            pub fixed_account: Option<String>,
            // The statement CSVs do not publish submission timestamps. This
            // optional field is populated only by future resources that do.
            #[serde(rename = "DT_RECEB", default)]
            pub submission_date: Option<chrono::NaiveDate>,
        }

        impl $name {
            pub fn scope(&self) -> StatementScope {
                statement_scope(&self.statement_group)
            }

            pub fn kind(&self) -> StatementKind {
                statement_kind(&self.statement_group)
            }

            pub fn exercise_order(&self) -> ExerciseOrder {
                self.exercise_order_raw
                    .as_deref()
                    .map(ExerciseOrder::from_cvm)
                    .unwrap_or(ExerciseOrder::Unknown)
            }
        }
    };
}

statement_row!(ItrStatementRow);
statement_row!(DfpStatementRow);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_parser_preserves_cvm_precision_and_accepts_brazilian_notation() {
        assert_eq!(
            "2455143253.0000000000"
                .parse::<CvmDecimal>()
                .unwrap()
                .to_string(),
            "2455143253.0000000000"
        );
        assert_eq!(
            "1.234,56".parse::<CvmDecimal>().unwrap().to_string(),
            "1234.56"
        );
        assert_eq!("-0,05".parse::<CvmDecimal>().unwrap().to_string(), "-0.05");
    }

    #[test]
    fn classifies_cvm_statement_group_and_exercise_order() {
        assert_eq!(
            statement_scope("DF Consolidado - Balanço Patrimonial Ativo"),
            StatementScope::Consolidated
        );
        assert_eq!(
            statement_scope("DF Individual - Demonstração de Resultado"),
            StatementScope::Individual
        );
        assert_eq!(
            statement_kind("DF Consolidado - Balanço Patrimonial Ativo"),
            StatementKind::Bpa
        );
        assert_eq!(
            ExerciseOrder::from_cvm("PENÚLTIMO"),
            ExerciseOrder::Previous
        );
        assert_eq!(
            statement_kind("DF Consolidado - Demonstração do Resultado Abrangente"),
            StatementKind::Dra
        );
        assert_eq!(
            statement_kind("DF Individual - Demonstração do Resultado"),
            StatementKind::Dre
        );
        assert_eq!(statement_kind("DFC-MD"), StatementKind::DfcDirect);
        assert_eq!(statement_kind("DFC-MI"), StatementKind::DfcIndirect);
        assert_eq!(statement_kind("DMPL"), StatementKind::Dmpl);
        assert_eq!(statement_kind("DVA"), StatementKind::Dva);
        assert_eq!(
            statement_kind("DF Consolidado - Balanço Patrimonial Passivo"),
            StatementKind::Bpp
        );
    }

    #[test]
    fn parses_an_actual_schema_shaped_itr_row_without_collapsing_versions() {
        use super::super::itr::parse_itr_rows;
        use crate::source_data::cvm::{CsvEncoding, CsvOptions};

        let csv = "CNPJ_CIA;DT_REFER;VERSAO;DENOM_CIA;CD_CVM;GRUPO_DFP;MOEDA;ESCALA_MOEDA;ORDEM_EXERC;DT_FIM_EXERC;CD_CONTA;DS_CONTA;VL_CONTA;ST_CONTA_FIXA\n00.000.000/0001-91;2026-03-31;2;BCO BRASIL S.A.;001023;DF Consolidado - BPA;REAL;MIL;PENÚLTIMO;2025-12-31;1;Ativo Total;2455143253.0000000000;S\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let row = parse_itr_rows(csv.as_bytes(), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.reference_date.unwrap().to_string(), "2026-03-31");
        assert_eq!(row.financial_period_end.unwrap().to_string(), "2025-12-31");
        assert_eq!(row.filing_version, Some(2));
        assert_eq!(row.scope(), StatementScope::Consolidated);
        assert_eq!(row.kind(), StatementKind::Bpa);
        assert_eq!(row.exercise_order(), ExerciseOrder::Previous);
        assert_eq!(
            row.account_value.unwrap().to_string(),
            "2455143253.0000000000"
        );
    }
}
