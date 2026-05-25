use crate::gutenberg::read_header_from_reader;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Cursor, Read};

pub(crate) fn index(mut reader: impl Read) -> Result<Option<HashMap<String, Vec<usize>>>> {
    let mut content = Vec::new();
    reader.read_to_end(&mut content)?;

    let header = read_header_from_reader(Cursor::new(&content))?;

    if !header.encoding.contains("ASCII") {
        return Ok(None);
    }

    let start = header.header_len.unwrap_or(0);
    let mut result = HashMap::new();
    index_internal(Cursor::new(&content[start..]), &mut result)?;
    Ok(Some(result))
}

fn index_internal<T: Read>(reader: T, result: &mut HashMap<String, Vec<usize>>) -> Result<()> {
    let mut word_mode = false;
    let mut word_position: usize = 0;
    let mut word_bytes: Vec<u8> = Vec::new();

    let mut add_word_to_result = |word_bytes: &Vec<u8>, word_position| {
        let word_string = String::from_utf8(word_bytes.clone())?.to_lowercase();
        result.entry(word_string).or_default().push(word_position);
        Ok::<(), anyhow::Error>(())
    };

    for (position, byte_result) in reader.bytes().enumerate() {
        let byte = byte_result?;
        if byte.is_ascii_alphabetic() {
            if !word_mode {
                word_mode = true;
                word_position = position;
            }
            word_bytes.push(byte);
        } else if word_mode {
            word_mode = false;
            add_word_to_result(&word_bytes, word_position)?;
            word_bytes.clear();
        }
    }

    if word_mode {
        add_word_to_result(&word_bytes, word_position)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::index_internal;
    use std::{collections::HashMap, io::Cursor};

    fn expected_index(v: &[(&str, &[usize])]) -> HashMap<String, Vec<usize>> {
        v.into_iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_vec()))
            .collect()
    }

    #[test]
    fn test_index_normal() {
        let reader = Cursor::new(String::from("Hello, Rust! Hello"));
        let mut index = HashMap::new();
        index_internal(reader, &mut index).unwrap();
        assert_eq!(
            expected_index(&[("hello", &[0, 13]), ("rust", &[7])]),
            index
        );
    }

    #[test]
    fn test_index_empty() {
        let reader = Cursor::new(String::from(""));
        let mut index = HashMap::new();
        index_internal(reader, &mut index).unwrap();
        assert_eq!(expected_index(&[]), index);

        let reader2 = Cursor::new(String::from("12_345 @$&"));
        let mut index2 = HashMap::new();
        index_internal(reader2, &mut index2).unwrap();
        assert_eq!(expected_index(&[]), index2);
    }
}
