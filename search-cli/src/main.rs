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

    let postings = match idx.search(&cli.word) {
        Some(result) => result
            .postings
            .iter()
            .map(|p| serde_json::json!({"doc_id": p.doc_id, "locations": p.locations}))
            .collect::<Vec<_>>(),
        None => vec![],
    };

    println!("{}", serde_json::to_string(&postings)?);

    Ok(())
}
