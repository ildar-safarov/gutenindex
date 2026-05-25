//! Binary index format (version 3):
//!
//! Index is a directory containing files with fixed names:
//!
//! ```text
//! vocab:
//! +---------------------+
//! | Header   (19 bytes) |
//! |   magic      (4)    |  0x49584442 ("IXDB")
//! |   version    (2)    |  3
//! |   word_count (4)    |
//! |   chunk_count (1)   |
//! |   doc_count  (4)    |  number of indexed documents
//! |   avg_doc_len (4)   |  f32, average document length in words
//! +---------------------+
//! | Vocabulary          |  25 bytes per entry
//! |   word_offset (8)   |  Offset to word string in this file
//! |   word_len    (4)   |
//! |   posting_cnt (4)   |
//! |   chunk_id    (1)   |  Which posting chunk file
//! |   post_offset (8)   |  Offset within that chunk file
//! +---------------------+
//! | Words               |  Raw UTF-8 strings
//! +---------------------+
//!
//! posting chunk files (0 … {chunk_count-1}):
//! +---------------------+
//! | Postings            |  Variable size
//! |   doc_id    (4)     |
//! |   loc_count (4)     |
//! |   locations (8*n)   |
//! +---------------------+
//!
//! doc length file ({base}.doclen):
//! +---------------------+
//! | doc_lengths (4*n)   |  u32 array indexed by doc_id
//! +---------------------+
//! ```

pub mod search;

use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const MAGIC: u32 = 0x49584442;
const VERSION: u16 = 3;
const CHUNK_COUNT: u8 = 8;

const HEADER_SIZE: usize = 4 + 2 + 4 + 1 + 4 + 4; // 19

const WORD_OFFSET_SIZE: usize = 8;
const WORD_LEN_SIZE: usize = 4;
const POSTING_COUNT_SIZE: usize = 4;
const CHUNK_ID_SIZE: usize = 1;
const POSTING_OFFSET_SIZE: usize = 8;
const VOCAB_ENTRY_SIZE: usize =
    WORD_OFFSET_SIZE + WORD_LEN_SIZE + POSTING_COUNT_SIZE + CHUNK_ID_SIZE + POSTING_OFFSET_SIZE;

const DOC_ID_SIZE: usize = 4;
const LOC_COUNT_SIZE: usize = 4;
const LOCATION_SIZE: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct VocabEntry {
    pub word_offset: u64,
    pub word_len: u32,
    pub posting_count: u32,
    pub chunk_id: u8,
    pub posting_offset: u64,
}

/// Memory-mapped index reader.
#[derive(Debug)]
pub struct IndexIo {
    vocab: memmap2::Mmap,
    chunks: Vec<memmap2::Mmap>,
    meta: memmap2::Mmap,
    meta_max_doc_id: u32,
    meta_lengths_base: usize,
    meta_strings_base: usize,
    word_count: usize,
    doc_count: u32,
    avg_doc_len: f32,
    doc_lengths: Vec<u32>,
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
    pub fn open<P: AsRef<Path>>(base: P) -> Result<Self> {
        let base = base.as_ref();

        let vocab_file = File::open(base.join("vocab"))?;
        let vocab = unsafe { memmap2::Mmap::map(&vocab_file)? };

        if vocab.len() < HEADER_SIZE {
            anyhow::bail!("Vocab file too small");
        }

        let magic = Self::read_u32_at(&vocab, 0);
        if magic != MAGIC {
            anyhow::bail!("Invalid magic: 0x{:08X}", magic);
        }

        let version = Self::read_u16_at(&vocab, 4);
        if version != VERSION {
            anyhow::bail!("Unsupported version: {}", version);
        }

        let word_count = Self::read_u32_at(&vocab, 6) as usize;
        let chunk_count = vocab[10] as usize;
        let doc_count = Self::read_u32_at(&vocab, 11);
        let avg_doc_len = f32::from_le_bytes(vocab[15..19].try_into().unwrap());

        let mut chunks = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            let f = File::open(base.join(i.to_string()))?;
            chunks.push(unsafe { memmap2::Mmap::map(&f)? });
        }

        let doclen_data = std::fs::read(base.join("doclen"))?;
        let doc_lengths: Vec<u32> = doclen_data
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();

        let meta_file = File::open(base.join("meta"))?;
        let meta = unsafe { memmap2::Mmap::map(&meta_file)? };
        let meta_max_doc_id = Self::read_u32_at(&meta, 0);
        let n = meta_max_doc_id as usize + 1;
        let meta_lengths_base = 4 + n * 4;
        let meta_strings_base = meta_lengths_base + n * 2;

        Ok(Self {
            vocab, chunks, meta, meta_max_doc_id, meta_lengths_base, meta_strings_base,
            word_count, doc_count, avg_doc_len, doc_lengths,
        })
    }

    pub fn write<P: AsRef<Path>>(
        global_index: &GlobalIndex,
        doc_lengths: &HashMap<usize, u32>,
        titles: &HashMap<usize, String>,
        base: P,
    ) -> Result<()> {
        let base = base.as_ref();
        std::fs::create_dir_all(base)?;
        let chunk_count = CHUNK_COUNT as usize;

        let mut keys: Vec<&String> = global_index.keys().collect();
        keys.sort();
        let word_count = keys.len();

        let doc_count = doc_lengths.len() as u32;
        let total_words: u64 = doc_lengths.values().map(|&l| l as u64).sum();
        let avg_doc_len: f32 = if doc_count > 0 {
            total_words as f32 / doc_count as f32
        } else {
            0.0
        };

        let max_doc_id = doc_lengths.keys().max().copied().unwrap_or(0);
        let mut dl_array = vec![0u32; max_doc_id + 1];
        for (&doc_id, &len) in doc_lengths {
            dl_array[doc_id] = len;
        }

        let chunk_ids: Vec<u8> = (0..word_count).map(|i| (i % chunk_count) as u8).collect();

        let posting_sizes: Vec<usize> = keys
            .iter()
            .map(|key| {
                global_index[*key]
                    .iter()
                    .map(|(_, locs)| DOC_ID_SIZE + LOC_COUNT_SIZE + locs.len() * LOCATION_SIZE)
                    .sum()
            })
            .collect();

        let mut chunk_cursors = vec![0usize; chunk_count];
        let mut posting_offsets = Vec::with_capacity(word_count);
        for (i, &chunk_id) in chunk_ids.iter().enumerate() {
            posting_offsets.push(chunk_cursors[chunk_id as usize]);
            chunk_cursors[chunk_id as usize] += posting_sizes[i];
        }

        let words_start = HEADER_SIZE + word_count * VOCAB_ENTRY_SIZE;
        let mut word_offset = words_start as u64;
        let mut word_offsets = Vec::with_capacity(word_count);
        for key in &keys {
            word_offsets.push(word_offset);
            word_offset += key.len() as u64;
        }

        let vocab_file = File::create(base.join("vocab"))?;
        let mut w = BufWriter::new(&vocab_file);

        w.write_all(&MAGIC.to_le_bytes())?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(word_count as u32).to_le_bytes())?;
        w.write_all(&[CHUNK_COUNT])?;
        w.write_all(&doc_count.to_le_bytes())?;
        w.write_all(&avg_doc_len.to_le_bytes())?;

        for (i, key) in keys.iter().enumerate() {
            w.write_all(&word_offsets[i].to_le_bytes())?;
            w.write_all(&(key.len() as u32).to_le_bytes())?;
            w.write_all(&(global_index[*key].len() as u32).to_le_bytes())?;
            w.write_all(&[chunk_ids[i]])?;
            w.write_all(&(posting_offsets[i] as u64).to_le_bytes())?;
        }

        for key in &keys {
            w.write_all(key.as_bytes())?;
        }
        w.flush()?;
        vocab_file.sync_all()?;

        let mut chunk_writers: Vec<BufWriter<File>> = (0..chunk_count)
            .map(|i| File::create(base.join(i.to_string())).map(BufWriter::new))
            .collect::<Result<_, _>>()?;

        for (i, key) in keys.iter().enumerate() {
            let cw = &mut chunk_writers[chunk_ids[i] as usize];
            for (doc_id, locations) in &global_index[*key] {
                cw.write_all(&(*doc_id as u32).to_le_bytes())?;
                cw.write_all(&(locations.len() as u32).to_le_bytes())?;
                for loc in locations {
                    cw.write_all(&(*loc as u64).to_le_bytes())?;
                }
            }
        }

        for mut cw in chunk_writers {
            cw.flush()?;
            cw.into_inner().unwrap().sync_all()?;
        }

        let doclen_file = File::create(base.join("doclen"))?;
        let mut dw = BufWriter::new(&doclen_file);
        for len in &dl_array {
            dw.write_all(&len.to_le_bytes())?;
        }
        dw.flush()?;
        doclen_file.sync_all()?;

        let meta_max_doc_id = titles.keys().max().copied().unwrap_or(0) as u32;
        let n = meta_max_doc_id as usize + 1;
        let mut meta_offsets = vec![0u32; n];
        let mut meta_lengths = vec![0u16; n];
        let mut meta_strings: Vec<u8> = Vec::new();
        for (&doc_id, title) in titles {
            let bytes = title.as_bytes();
            meta_offsets[doc_id] = meta_strings.len() as u32;
            meta_lengths[doc_id] = bytes.len() as u16;
            meta_strings.extend_from_slice(bytes);
        }

        let meta_file = File::create(base.join("meta"))?;
        let mut mw = BufWriter::new(&meta_file);
        mw.write_all(&meta_max_doc_id.to_le_bytes())?;
        for o in &meta_offsets { mw.write_all(&o.to_le_bytes())?; }
        for l in &meta_lengths { mw.write_all(&l.to_le_bytes())?; }
        mw.write_all(&meta_strings)?;
        mw.flush()?;
        meta_file.sync_all()?;

        Ok(())
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

    pub(crate) fn read_u32_chunk(&self, chunk_id: u8, offset: usize) -> u32 {
        Self::read_u32_at(&self.chunks[chunk_id as usize], offset)
    }

    pub(crate) fn read_u64_chunk(&self, chunk_id: u8, offset: usize) -> u64 {
        Self::read_u64_at(&self.chunks[chunk_id as usize], offset)
    }

    #[must_use]
    pub fn word_count(&self) -> usize {
        self.word_count
    }

    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.doc_count
    }

    #[must_use]
    pub fn avg_doc_len(&self) -> f32 {
        self.avg_doc_len
    }

    pub(crate) fn doc_len(&self, doc_id: u32) -> u32 {
        self.doc_lengths.get(doc_id as usize).copied().unwrap_or(0)
    }

    pub fn doc_title(&self, doc_id: u32) -> Option<&str> {
        if doc_id > self.meta_max_doc_id {
            return None;
        }
        let i = doc_id as usize;
        let offset = Self::read_u32_at(&self.meta, 4 + i * 4) as usize;
        let length = u16::from_le_bytes(
            self.meta[self.meta_lengths_base + i * 2..self.meta_lengths_base + i * 2 + 2]
                .try_into()
                .unwrap(),
        ) as usize;
        if length == 0 {
            return None;
        }
        std::str::from_utf8(
            &self.meta[self.meta_strings_base + offset..self.meta_strings_base + offset + length],
        )
        .ok()
    }

    pub(crate) fn get_vocab_entry(&self, idx: usize) -> VocabEntry {
        let start = HEADER_SIZE + idx * VOCAB_ENTRY_SIZE;
        let d = &self.vocab;
        VocabEntry {
            word_offset: Self::read_u64_at(d, start),
            word_len: Self::read_u32_at(d, start + WORD_OFFSET_SIZE),
            posting_count: Self::read_u32_at(d, start + WORD_OFFSET_SIZE + WORD_LEN_SIZE),
            chunk_id: d[start + WORD_OFFSET_SIZE + WORD_LEN_SIZE + POSTING_COUNT_SIZE],
            posting_offset: Self::read_u64_at(
                d,
                start + WORD_OFFSET_SIZE + WORD_LEN_SIZE + POSTING_COUNT_SIZE + CHUNK_ID_SIZE,
            ),
        }
    }

    pub(crate) fn get_word_bytes(&self, idx: usize) -> &[u8] {
        let entry = self.get_vocab_entry(idx);
        let start = entry.word_offset as usize;
        &self.vocab[start..start + entry.word_len as usize]
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
