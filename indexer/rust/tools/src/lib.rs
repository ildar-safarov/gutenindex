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
) -> Result<()> {
    let txt_files: Vec<_> = WalkDir::new(library_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();

    let mut ascii_books = Vec::new();

    let pb = ProgressBar::new(txt_files.len() as u64);
    for entry in txt_files {
        let path_str = entry.path().to_str().context("Invalid path")?;

        if let Ok(header) = gutenberg::read_header(path_str)
            && header.encoding.contains("ASCII")
        {
            ascii_books.push(path_str.to_string());
        }
        pb.inc(1);
    }
    pb.finish();

    ascii_books.sort();

    let config = IndexerInput { files: ascii_books };

    let file = File::create(save_indexer_input_json_to)?;
    serde_json::to_writer_pretty(file, &config)?;

    Ok(())
}
