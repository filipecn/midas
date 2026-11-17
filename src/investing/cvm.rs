//! Small investing-side adapters for filtering typed CVM statement rows by
//! company. Retrieval and file parsing remain in `source_data::cvm`.

use crate::source_data::cvm::{
    CsvOptions, companies,
    companies::{DfpStatementRow, ItrStatementRow},
};
use std::io::Read;

/// Parse extracted fund daily data for one CVM fund/class CNPJ.
pub fn fund_daily_rows_for_cnpj<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
    cnpj: &str,
) -> impl Iterator<Item = Result<crate::source_data::cvm::funds::FundDailyInformation, csv::Error>>
{
    let target = cnpj.to_owned();
    crate::source_data::cvm::funds::parse_fund_daily_rows(reader, options).filter(move |row| {
        match row {
            Ok(row) => row.cnpj == target,
            Err(_) => true,
        }
    })
}

/// Parse extracted portfolio disclosures for one CVM fund/class CNPJ.
pub fn fund_portfolio_rows_for_cnpj<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
    cnpj: &str,
) -> impl Iterator<Item = Result<crate::source_data::cvm::funds::FundPortfolioHolding, csv::Error>>
{
    let target = cnpj.to_owned();
    crate::source_data::cvm::funds::parse_fund_portfolio_rows(reader, options).filter(move |row| {
        match row {
            Ok(row) => row.cnpj == target,
            Err(_) => true,
        }
    })
}

/// Parse an extracted ITR CSV and stream only the selected company's rows.
/// CSV errors are yielded unchanged for the caller to handle.
pub fn itr_rows_for_company<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
    cnpj: &str,
) -> impl Iterator<Item = Result<ItrStatementRow, csv::Error>> {
    let target = cnpj.to_owned();
    companies::parse_itr_rows(reader, options).filter(move |row| match row {
        Ok(row) => row.cnpj == target,
        Err(_) => true,
    })
}

/// Parse an extracted DFP CSV and stream only the selected company's rows.
pub fn dfp_rows_for_company<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
    cnpj: &str,
) -> impl Iterator<Item = Result<DfpStatementRow, csv::Error>> {
    let target = cnpj.to_owned();
    companies::parse_dfp_rows(reader, options).filter(move |row| match row {
        Ok(row) => row.cnpj == target,
        Err(_) => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_data::cvm::CsvEncoding;

    #[test]
    fn filters_itr_rows_without_discarding_other_companies_as_errors() {
        let csv = "CNPJ_CIA;DT_REFER;VERSAO;DENOM_CIA;CD_CVM;GRUPO_DFP;MOEDA;ESCALA_MOEDA;ORDEM_EXERC;DT_FIM_EXERC;CD_CONTA;DS_CONTA;VL_CONTA;ST_CONTA_FIXA\n00.000.000/0001-91;2026-03-31;1;A;000001;DF Consolidado - BPA;REAL;MIL;ÚLTIMO;2026-03-31;1;Ativo Total;12.00;S\n11.111.111/0001-11;2026-03-31;1;B;000002;DF Consolidado - BPA;REAL;MIL;ÚLTIMO;2026-03-31;1;Ativo Total;13.00;S\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let rows = itr_rows_for_company(csv.as_bytes(), options, "11.111.111/0001-11")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cnpj, "11.111.111/0001-11");
    }
}
