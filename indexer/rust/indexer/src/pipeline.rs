use crate::indexer;
use anyhow::Result;
use index_io::{GlobalIndex, IndexIo};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use zip::ZipArchive;

#[derive(Deserialize, Serialize)]
pub struct IndexerInput {
    pub doc_ids: Vec<usize>,
}

pub fn index_files(
    indexer_input_path: impl AsRef<Path>,
    corpus_dir: impl AsRef<Path>,
    output_db_path: impl AsRef<Path>,
) -> Result<()> {
    let corpus_dir = corpus_dir.as_ref();
    let output_db_path = output_db_path.as_ref();

    let indexer_input: IndexerInput =
        serde_json::from_str(&std::fs::read_to_string(indexer_input_path)?)?;

    let pb = ProgressBar::new(indexer_input.doc_ids.len() as u64);
    let mut global_index: GlobalIndex = GlobalIndex::new();
    let mut doc_lengths: HashMap<usize, u32> = HashMap::new();

    let mut by_bucket: [Vec<usize>; 10] = Default::default();
    for doc_id in indexer_input.doc_ids {
        by_bucket[doc_id % 10].push(doc_id);
    }

    for (bucket, doc_ids) in by_bucket.iter().enumerate() {
        if doc_ids.is_empty() {
            continue;
        }
        let zip_path = corpus_dir.join(format!("{bucket}.zip"));
        let mut archive = ZipArchive::new(File::open(&zip_path)?)?;

        for &doc_id in doc_ids {
            let mut entry = archive.by_name(&format!("{doc_id}.txt"))?;
            let Some(doc_index) = indexer::index(&mut entry)? else {
                pb.inc(1);
                continue;
            };
            let doc_len: u32 = doc_index.values().map(|v| v.len()).sum::<usize>() as u32;
            doc_lengths.insert(doc_id, doc_len);
            for (word, locations) in doc_index {
                global_index
                    .entry(word)
                    .or_default()
                    .push((doc_id, locations));
            }
            pb.inc(1);
        }
    }

    pb.finish();

    IndexIo::write(&global_index, &doc_lengths, output_db_path)?;
    Ok(())
}
