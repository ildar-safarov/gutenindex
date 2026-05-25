use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the index file
    #[arg(short, long)]
    index: String,
    /// Word to search for
    #[arg(short, long)]
    word: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let idx = index_io::IndexIo::open(&cli.index)?.into_search_index();

    if let Some(result) = idx.search(&cli.word) {
        println!("Found word: {}", result.word);
        println!("Postings count: {}", result.postings.len());
        for posting in result.postings.iter().take(5) {
            println!(
                "  doc_id: {}, locations: {:?}",
                posting.doc_id,
                &posting.locations[..posting.locations.len().min(5)]
            );
        }
    } else {
        println!("Word '{}' not found", cli.word);
    }

    Ok(())
}
