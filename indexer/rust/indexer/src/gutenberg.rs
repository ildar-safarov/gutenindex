use std::fs::File;
use std::io::{self, BufRead, BufReader};

use anyhow::{Context, Result};

static HEADER_FIELDS: &[&str] = &["Title: ", "Author: ", "Character set encoding: "];

#[derive(Debug, Default)]
pub struct GutenbergTxtHeader {
    pub title: String,
    pub author: String,
    pub encoding: String,
    pub header_len: Option<usize>,
}

pub fn read_header(path: &str) -> Result<GutenbergTxtHeader> {
    let file = File::open(path).context("Failed to open file")?;
    read_header_from_reader(file)
}

pub fn read_header_from_reader<R: io::Read>(reader: R) -> Result<GutenbergTxtHeader> {
    let mut reader = BufReader::new(reader);

    let mut result = GutenbergTxtHeader::default();
    let mut line_count = 0;
    let mut header_len: usize = 0;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim_end_matches(|c| c == '\n' || c == '\r');

        for (field_idx, &field) in HEADER_FIELDS.iter().enumerate() {
            if trimmed.starts_with(field) {
                let value = trimmed[field.len()..].to_string();
                match field_idx {
                    0 => result.title = value,
                    1 => result.author = value,
                    2 => result.encoding = value,
                    _ => unreachable!("There is no proper setter for field {}", field),
                }
            }
        }

        header_len += bytes_read;
        line_count += 1;

        if (trimmed.contains("START OF") && trimmed.contains("PROJECT GUTENBERG EBOOK"))
            || line_count >= 100
        {
            break;
        }
    }

    if line_count < 100 {
        result.header_len = Some(header_len);
    } else {
        result.header_len = None;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::gutenberg::read_header_from_reader;
    use crate::test_data::NIECES_ON_VACATION;
    use std::io::Cursor;

    #[test]
    fn test_read_header() {
        let cursor = Cursor::new(NIECES_ON_VACATION);

        let header = read_header_from_reader(cursor).expect("Failed to read header");

        assert_eq!(header.title, "Aunt Jane's Nieces on Vacation");
        assert_eq!(header.author, "Edith Van Dyne");
        assert_eq!(header.encoding, "ASCII");

        assert!(header.header_len.is_some());
    }
}
