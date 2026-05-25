use index_io::{GlobalIndex, IndexIo};
use crate::indexer;
use anyhow::Result;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
pub struct IndexerInput {
    pub files: Vec<String>,
}

fn extract_doc_id(file_path: &str) -> Result<usize> {
    let path = PathBuf::from(file_path);

    let file_name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", file_path))?;

    file_name.parse::<usize>().map_err(|_| {
        anyhow::anyhow!(
            "Cannot parse '{}' as document id from '{}'",
            file_name,
            file_path
        )
    })
}

pub fn index_files(
    indexer_input_path: impl AsRef<Path>,
    output_db_path: impl AsRef<Path>,
) -> Result<()> {
    let indexer_input_path = indexer_input_path.as_ref();
    let output_db_path = output_db_path.as_ref();

    let indexer_input_str = std::fs::read_to_string(indexer_input_path)?;
    let indexer_input: IndexerInput = serde_json::from_str(&indexer_input_str)?;

    let input_dir = indexer_input_path.parent().unwrap_or(Path::new("."));

    let mut global_index: GlobalIndex = GlobalIndex::new();
    let total_files = indexer_input.files.len() as u64;
    let pb = ProgressBar::new(total_files);

    for file in indexer_input.files {
        let doc_id = extract_doc_id(&file)?;
        let doc_index = indexer::index(input_dir.join(&file))?;

        for (word, locations) in doc_index {
            global_index
                .entry(word)
                .or_default()
                .push((doc_id, locations));
        }

        pb.inc(1);
    }

    pb.finish();

    IndexIo::write(&global_index, output_db_path)?;

    Ok(())
}
