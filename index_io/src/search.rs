use crate::{IndexIo, Posting, SearchResult};
use std::cmp::Ordering;

const DOC_ID_SIZE: usize = 4;
const LOC_COUNT_SIZE: usize = 4;
const LOCATION_SIZE: usize = 8;

/// Search index providing fast word lookups via binary search.
#[derive(Debug)]
pub struct SearchIndex {
    index: IndexIo,
}

impl SearchIndex {
    pub fn new(index: IndexIo) -> Self {
        Self { index }
    }

    /// Returns `Some(SearchResult)` if found, `None` otherwise.
    #[must_use]
    pub fn search(&self, word: &str) -> Option<SearchResult> {
        let mut left = 0;
        let mut right = self.index.word_count();

        while left < right {
            let mid = left + (right - left) / 2;
            let word_bytes = self.index.get_word_bytes(mid);
            let mid_word = std::str::from_utf8(word_bytes).ok()?;

            match mid_word.cmp(word) {
                Ordering::Equal => {
                    return Some(self.get_postings(mid));
                }
                Ordering::Less => left = mid + 1,
                Ordering::Greater => right = mid,
            }
        }

        None
    }

    /// Uses O(1) direct offset lookup instead of scanning previous entries.
    fn get_postings(&self, idx: usize) -> SearchResult {
        let entry = self.index.get_vocab_entry(idx);
        let mut offset = entry.posting_offset as usize;

        let mut postings = Vec::with_capacity(entry.posting_count as usize);

        for _ in 0..entry.posting_count {
            let doc_id = self.index.read_u32(offset);
            let loc_count = self.index.read_u32(offset + DOC_ID_SIZE) as usize;
            offset += DOC_ID_SIZE + LOC_COUNT_SIZE;

            let mut locations = Vec::with_capacity(loc_count);
            for _ in 0..loc_count {
                locations.push(self.index.read_u64(offset));
                offset += LOCATION_SIZE;
            }

            postings.push(Posting { doc_id, locations });
        }

        let word = self.index.get_word(idx);
        SearchResult { word, postings }
    }

    pub fn iter_words(&self) -> impl Iterator<Item = String> + '_ {
        (0..self.index.word_count()).map(move |i| self.index.get_word(i))
    }
}
