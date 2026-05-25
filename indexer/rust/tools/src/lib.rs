use anyhow::Result;
use indexer::gutenberg;
use indexer::pipeline::IndexerInput;
use indicatif::ProgressBar;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

pub fn collect_ascii_books(
    corpus_dir: impl AsRef<Path>,
    save_indexer_input_json_to: impl AsRef<Path>,
    limit: Option<usize>,
) -> Result<()> {
    let corpus_dir = corpus_dir.as_ref();

    let total: u64 = (0..10u8)
        .filter_map(|b| File::open(corpus_dir.join(format!("{b}.zip"))).ok())
        .filter_map(|f| ZipArchive::new(f).ok())
        .map(|a| a.len() as u64)
        .sum();

    let pb = ProgressBar::new(total);
    let mut ascii_books: Vec<usize> = Vec::new();

    for bucket in 0..10u8 {
        let zip_path = corpus_dir.join(format!("{bucket}.zip"));
        if !zip_path.exists() {
            continue;
        }
        let mut archive = ZipArchive::new(File::open(&zip_path)?)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_owned();
            pb.inc(1);

            let Some(doc_id) = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<usize>().ok())
            else {
                continue;
            };

            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;

            if let Ok(header) = gutenberg::read_header_from_reader(Cursor::new(&content))
                && header.encoding.contains("ASCII")
            {
                ascii_books.push(doc_id);
            }
        }
    }
    pb.finish();

    ascii_books.sort();
    if let Some(n) = limit {
        ascii_books.truncate(n);
    }

    let config = IndexerInput { doc_ids: ascii_books };
    serde_json::to_writer_pretty(File::create(save_indexer_input_json_to)?, &config)?;

    Ok(())
}
