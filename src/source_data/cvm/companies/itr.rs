use super::financial::ItrStatementRow;
use crate::source_data::cvm::{CsvOptions, ResourceParser};
use std::io::Read;

/// Stream typed rows from an extracted ITR statement CSV.
pub fn parse_itr_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, ItrStatementRow> {
    ResourceParser::csv_rows(reader, options)
}
