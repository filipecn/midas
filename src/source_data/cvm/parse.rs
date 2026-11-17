use super::CvmError;
use csv::DeserializeRecordsIntoIter;
use encoding_rs::{Encoding, WINDOWS_1252};
use serde::de::DeserializeOwned;
use std::io::{self, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvEncoding {
    Utf8,
    Windows1252,
    Latin1,
}

#[derive(Debug, Clone, Copy)]
pub struct CsvOptions {
    pub encoding: CsvEncoding,
    pub delimiter: u8,
    pub has_headers: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            encoding: CsvEncoding::Utf8,
            delimiter: b',',
            has_headers: true,
        }
    }
}

pub struct ResourceParser;

impl ResourceParser {
    /// Parse a JSON resource directly from a reader. serde_json reads the
    /// document incrementally; callers can deserialize into typed models.
    pub fn json<T: DeserializeOwned>(reader: impl Read) -> Result<T, CvmError> {
        Ok(serde_json::from_reader(reader)?)
    }

    /// Return a streaming CSV row iterator. Each row is deserialized using its
    /// own dataset-specific type; malformed rows are returned as errors rather
    /// than silently skipped.
    pub fn csv_rows<R, T>(
        reader: R,
        options: CsvOptions,
    ) -> DeserializeRecordsIntoIter<Box<dyn Read>, T>
    where
        R: Read + 'static,
        T: DeserializeOwned,
    {
        let reader: Box<dyn Read> = match options.encoding {
            CsvEncoding::Utf8 => Box::new(reader),
            CsvEncoding::Windows1252 => Box::new(EncodingReader::new(reader, WINDOWS_1252)),
            // encoding_rs maps the ISO-8859-1 label to Windows-1252, matching
            // how legacy CVM CSV files are commonly encoded in practice.
            CsvEncoding::Latin1 => Box::new(EncodingReader::new(reader, WINDOWS_1252)),
        };
        csv::ReaderBuilder::new()
            .delimiter(options.delimiter)
            .has_headers(options.has_headers)
            .from_reader(reader)
            .into_deserialize()
    }
}

struct EncodingReader<R> {
    inner: R,
    decoder: encoding_rs::Decoder,
    input: [u8; 8192],
    input_start: usize,
    input_end: usize,
    eof: bool,
    output: [u8; 32768],
    output_start: usize,
    output_end: usize,
}

impl<R> EncodingReader<R> {
    fn new(inner: R, encoding: &'static Encoding) -> Self {
        Self {
            inner,
            decoder: encoding.new_decoder_without_bom_handling(),
            input: [0; 8192],
            input_start: 0,
            input_end: 0,
            eof: false,
            output: [0; 32768],
            output_start: 0,
            output_end: 0,
        }
    }
}

impl<R: Read> Read for EncodingReader<R> {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        loop {
            if self.output_start < self.output_end {
                let count = (self.output_end - self.output_start).min(target.len());
                target[..count]
                    .copy_from_slice(&self.output[self.output_start..self.output_start + count]);
                self.output_start += count;
                return Ok(count);
            }

            self.output_start = 0;
            self.output_end = 0;
            if self.input_start == self.input_end && !self.eof {
                self.input_end = self.inner.read(&mut self.input)?;
                self.input_start = 0;
                self.eof = self.input_end == 0;
            }

            let (result, read, written, _) = self.decoder.decode_to_utf8(
                &self.input[self.input_start..self.input_end],
                &mut self.output,
                self.eof,
            );
            self.input_start += read;
            self.output_end = written;
            if written > 0 {
                continue;
            }
            if self.eof && self.input_start == self.input_end {
                return Ok(0);
            }
            if read == 0 && result == encoding_rs::CoderResult::OutputFull {
                return Err(io::Error::other("encoding output buffer unexpectedly full"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Row {
        nome: String,
        valor: String,
    }

    #[test]
    fn parses_utf8_csv_as_a_stream_of_typed_rows() {
        let data = "nome,valor\nEmpresa,12.3\n";
        let rows = ResourceParser::csv_rows(data.as_bytes(), CsvOptions::default());
        let parsed = rows.collect::<Result<Vec<Row>, _>>().unwrap();
        assert_eq!(
            parsed,
            vec![Row {
                nome: "Empresa".into(),
                valor: "12.3".into()
            }]
        );
    }

    #[test]
    fn decodes_windows_1252_csv_values() {
        let data = b"nome;valor\nCompanhia;S\xe3o Paulo\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let rows = ResourceParser::csv_rows(&data[..], options);
        let parsed = rows.collect::<Result<Vec<Row>, _>>().unwrap();
        assert_eq!(parsed[0].valor, "São Paulo");
    }

    #[test]
    fn parses_generic_json_values() {
        let value: serde_json::Value = ResourceParser::json(&b"{\"ok\":true}"[..]).unwrap();
        assert_eq!(value["ok"], true);
    }
}
