use crate::{IndexIo, Posting, SearchResult};
use std::cmp::Ordering;
use std::collections::HashMap;

const DOC_ID_SIZE: usize = 4;
const LOC_COUNT_SIZE: usize = 4;
const LOCATION_SIZE: usize = 8;

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// Search index providing fast word lookups via binary search.
#[derive(Debug)]
pub struct SearchIndex {
    index: IndexIo,
}

#[derive(Debug, Clone)]
pub struct ScoredDoc {
    pub doc_id: u32,
    pub score: f32,
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
                Ordering::Equal => return Some(self.get_postings(mid)),
                Ordering::Less => left = mid + 1,
                Ordering::Greater => right = mid,
            }
        }

        None
    }

    /// BM25 ranking over one or more query terms. Returns docs sorted by score descending.
    pub fn bm25_search(&self, terms: &[&str]) -> Vec<ScoredDoc> {
        let n = self.index.doc_count() as f32;
        let avgdl = self.index.avg_doc_len();
        let mut scores: HashMap<u32, f32> = HashMap::new();

        for &term in terms {
            let Some(result) = self.search(term) else { continue };

            let df = result.postings.len() as f32;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

            for posting in &result.postings {
                let tf = posting.locations.len() as f32;
                let dl = self.index.doc_len(posting.doc_id) as f32;
                let tf_norm = if avgdl > 0.0 {
                    tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * dl / avgdl))
                } else {
                    tf * (K1 + 1.0) / (tf + K1)
                };
                *scores.entry(posting.doc_id).or_insert(0.0) += idf * tf_norm;
            }
        }

        let mut ranked: Vec<ScoredDoc> = scores
            .into_iter()
            .map(|(doc_id, score)| ScoredDoc { doc_id, score })
            .collect();
        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        ranked
    }

    /// Uses O(1) direct offset lookup instead of scanning previous entries.
    fn get_postings(&self, idx: usize) -> SearchResult {
        let entry = self.index.get_vocab_entry(idx);
        let chunk_id = entry.chunk_id;
        let mut offset = entry.posting_offset as usize;

        let mut postings = Vec::with_capacity(entry.posting_count as usize);

        for _ in 0..entry.posting_count {
            let doc_id = self.index.read_u32_chunk(chunk_id, offset);
            let loc_count = self.index.read_u32_chunk(chunk_id, offset + DOC_ID_SIZE) as usize;
            offset += DOC_ID_SIZE + LOC_COUNT_SIZE;

            let mut locations = Vec::with_capacity(loc_count);
            for _ in 0..loc_count {
                locations.push(self.index.read_u64_chunk(chunk_id, offset));
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
