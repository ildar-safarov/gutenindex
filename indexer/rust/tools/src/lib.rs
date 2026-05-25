use anyhow::{Context, Result};
use indexer::gutenberg;
use indexer::pipeline::IndexerInput;
use indicatif::ProgressBar;
use std::fs::File;
use std::path::Path;
use walkdir::WalkDir;

pub fn collect_ascii_books(
    library_dir: impl AsRef<Path>,
    save_indexer_input_json_to: impl AsRef<Path>,
    limit: Option<usize>,
) -> Result<()> {
    let txt_files: Vec<_> = WalkDir::new(library_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();

    let mut ascii_books: Vec<usize> = Vec::new();

    let pb = ProgressBar::new(txt_files.len() as u64);
    for entry in txt_files {
        let path_str = entry.path().to_str().context("Invalid path")?;

        if let Ok(header) = gutenberg::read_header(path_str)
            && header.encoding.contains("ASCII")
        {
            if let Some(doc_id) = entry.path().file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<usize>().ok()) {
                ascii_books.push(doc_id);
            }
        }
        pb.inc(1);
    }
    pb.finish();

    ascii_books.sort();
    if let Some(n) = limit {
        ascii_books.truncate(n);
    }

    let config = IndexerInput { doc_ids: ascii_books };

    let file = File::create(save_indexer_input_json_to)?;
    serde_json::to_writer_pretty(file, &config)?;

    Ok(())
}
