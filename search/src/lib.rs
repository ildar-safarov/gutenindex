use index_io::IndexIo;
pub use index_io::{Posting, WordPostings};
use std::cmp::Ordering;
use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;

#[derive(Debug, Clone)]
pub struct ScoredDoc {
    pub doc_id: u32,
    pub score: f32,
}

#[derive(Debug)]
pub struct SearchIndex {
    index: IndexIo,
}

impl SearchIndex {
    pub fn new(index: IndexIo) -> Self {
        Self { index }
    }

    #[must_use]
    pub fn search(&self, word: &str) -> Option<WordPostings> {
        let word = word.to_lowercase();
        let mut left = 0;
        let mut right = self.index.word_count();

        while left < right {
            let mid = left + (right - left) / 2;
            let word_bytes = self.index.get_word_bytes(mid);
            let mid_word = std::str::from_utf8(word_bytes).ok()?;

            match mid_word.cmp(&word) {
                Ordering::Equal => return Some(self.index.get_postings(mid)),
                Ordering::Less => left = mid + 1,
                Ordering::Greater => right = mid,
            }
        }

        None
    }

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
                let tf_norm = tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * dl / avgdl));
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

    pub fn doc_title(&self, doc_id: u32) -> Option<&str> {
        self.index.doc_title(doc_id)
    }

    pub fn iter_words(&self) -> impl Iterator<Item = String> + '_ {
        (0..self.index.word_count()).map(move |i| self.index.get_word(i))
    }
}
