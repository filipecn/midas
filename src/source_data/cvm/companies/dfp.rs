use super::financial::DfpStatementRow;
use crate::source_data::cvm::{CsvOptions, ResourceParser};
use std::io::Read;

/// Stream typed rows from an extracted DFP statement CSV.
pub fn parse_dfp_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, DfpStatementRow> {
    ResourceParser::csv_rows(reader, options)
}
