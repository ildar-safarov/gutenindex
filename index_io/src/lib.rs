//! Binary index file format:
//!
//! ```text
//! +------------------+
//! | Header (10 bytes)|
//! |   magic (4)      |  0x49584442 ("IXDB")
//! |   version (2)    |  Currently 1
//! |   word_count (4) |  Number of vocabulary entries
//! +------------------+
//! | Vocabulary       |  24 bytes per entry
//! |   word_offset (8)|  Offset to word string
//! |   word_len (4)   |  Length of word string
//! |   posting_cnt (4)|  Number of postings
//! |   post_offset (8)|  Offset to posting data
//! +------------------+
//! | Postings         |  Variable size
//! |   doc_id (4)     |
//! |   loc_count (4)  |
//! |   locations (8*n)|
//! +------------------+
//! | Words            |  Raw UTF-8 strings
//! +------------------+
//! ```

pub mod search;

use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const MAGIC: u32 = 0x49584442;
const VERSION: u16 = 1;

const MAGIC_SIZE: usize = 4;
const VERSION_SIZE: usize = 2;
const WORD_COUNT_SIZE: usize = 4;
const HEADER_SIZE: usize = MAGIC_SIZE + VERSION_SIZE + WORD_COUNT_SIZE;

const WORD_OFFSET_SIZE: usize = 8;
const WORD_LEN_SIZE: usize = 4;
const POSTING_COUNT_SIZE: usize = 4;
const POSTING_OFFSET_SIZE: usize = 8;
const VOCAB_ENTRY_SIZE: usize =
    WORD_OFFSET_SIZE + WORD_LEN_SIZE + POSTING_COUNT_SIZE + POSTING_OFFSET_SIZE;

const DOC_ID_SIZE: usize = 4;
const LOC_COUNT_SIZE: usize = 4;
const LOCATION_SIZE: usize = 8;

/// A vocabulary entry containing metadata for a word in the index.
#[derive(Debug, Clone, Copy)]
pub struct VocabEntry {
    /// Byte offset to the word string in the file
    pub word_offset: u64,
    /// Length of the word string in bytes
    pub word_len: u32,
    /// Number of postings for this word
    pub posting_count: u32,
    /// Byte offset to the posting data in the file
    pub posting_offset: u64,
}

/// Memory-mapped index reader.
#[derive(Debug)]
pub struct IndexIo {
    data: memmap2::Mmap,
    word_count: usize,
    vocab_offset: usize,
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub doc_id: u32,
    pub locations: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub word: String,
    pub postings: Vec<Posting>,
}

pub type GlobalIndex = HashMap<String, Vec<(usize, Vec<usize>)>>;

impl IndexIo {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let data = unsafe { memmap2::Mmap::map(&file)? };

        if data.len() < HEADER_SIZE {
            anyhow::bail!(
                "File too small: {} bytes, minimum {} required",
                data.len(),
                HEADER_SIZE
            );
        }

        let magic = Self::read_u32_at(&data, 0);
        if magic != MAGIC {
            anyhow::bail!(
                "Invalid magic number: expected 0x{:08X}, got 0x{:08X}",
                MAGIC,
                magic
            );
        }

        let version = Self::read_u16_at(&data, MAGIC_SIZE);
        if version != VERSION {
            anyhow::bail!("Unsupported version: {}", version);
        }

        let word_count = Self::read_u32_at(&data, MAGIC_SIZE + VERSION_SIZE) as usize;
        let vocab_offset = HEADER_SIZE;

        let min_size = HEADER_SIZE + word_count * VOCAB_ENTRY_SIZE;
        if data.len() < min_size {
            anyhow::bail!(
                "File too small for vocabulary: {} bytes, minimum {} required",
                data.len(),
                min_size
            );
        }

        Ok(Self {
            data,
            word_count,
            vocab_offset,
        })
    }

    fn read_u16_at(data: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
    }

    fn read_u32_at(data: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
    }

    fn read_u64_at(data: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
    }

    pub(crate) fn read_u32(&self, offset: usize) -> u32 {
        Self::read_u32_at(&self.data, offset)
    }

    pub(crate) fn read_u64(&self, offset: usize) -> u64 {
        Self::read_u64_at(&self.data, offset)
    }

    pub fn write<P: AsRef<Path>>(global_index: &GlobalIndex, path: P) -> Result<()> {
        let path = path.as_ref();

        let word_count = global_index.len();

        let mut keys: Vec<&String> = global_index.keys().collect();
        keys.sort();

        let file = File::create(path)?;
        let mut writer = BufWriter::new(&file);

        writer.write_all(&MAGIC.to_le_bytes())?;
        writer.write_all(&VERSION.to_le_bytes())?;
        writer.write_all(&(word_count as u32).to_le_bytes())?;

        let mut posting_sizes: Vec<usize> = Vec::with_capacity(keys.len());
        for key in &keys {
            let postings = &global_index[*key];
            let mut size = 0;
            for (_, locations) in postings {
                size += DOC_ID_SIZE + LOC_COUNT_SIZE + locations.len() * LOCATION_SIZE;
            }
            posting_sizes.push(size);
        }

        let postings_start = (HEADER_SIZE + word_count * VOCAB_ENTRY_SIZE) as u64;
        let total_posting_size: usize = posting_sizes.iter().sum();
        let words_start = postings_start + total_posting_size as u64;

        let mut word_offset = words_start;
        let mut posting_offset = postings_start;
        for (i, key) in keys.iter().enumerate() {
            writer.write_all(&word_offset.to_le_bytes())?;
            writer.write_all(&(key.len() as u32).to_le_bytes())?;
            writer.write_all(&(global_index[*key].len() as u32).to_le_bytes())?;
            writer.write_all(&posting_offset.to_le_bytes())?;

            word_offset += key.len() as u64;
            posting_offset += posting_sizes[i] as u64;
        }

        for key in &keys {
            let postings = &global_index[*key];
            for (doc_id, locations) in postings {
                writer.write_all(&(*doc_id as u32).to_le_bytes())?;
                writer.write_all(&(locations.len() as u32).to_le_bytes())?;
                for loc in locations {
                    writer.write_all(&(*loc as u64).to_le_bytes())?;
                }
            }
        }

        for key in &keys {
            writer.write_all(key.as_bytes())?;
        }

        writer.flush()?;
        file.sync_all()?;

        Ok(())
    }

    #[must_use]
    pub fn word_count(&self) -> usize {
        self.word_count
    }

    pub(crate) fn get_vocab_entry(&self, idx: usize) -> VocabEntry {
        let start = self.vocab_offset + idx * VOCAB_ENTRY_SIZE;
        VocabEntry {
            word_offset: self.read_u64(start),
            word_len: self.read_u32(start + WORD_OFFSET_SIZE),
            posting_count: self.read_u32(start + WORD_OFFSET_SIZE + WORD_LEN_SIZE),
            posting_offset: self
                .read_u64(start + WORD_OFFSET_SIZE + WORD_LEN_SIZE + POSTING_COUNT_SIZE),
        }
    }

    pub(crate) fn get_word_bytes(&self, idx: usize) -> &[u8] {
        let entry = self.get_vocab_entry(idx);
        let start = entry.word_offset as usize;
        &self.data[start..start + entry.word_len as usize]
    }

    #[must_use]
    pub fn get_word(&self, idx: usize) -> String {
        String::from_utf8_lossy(self.get_word_bytes(idx)).to_string()
    }

    #[must_use]
    pub fn into_search_index(self) -> search::SearchIndex {
        search::SearchIndex::new(self)
    }
}
