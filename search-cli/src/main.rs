use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the index base (without extension)
    #[arg(short, long)]
    index: String,
    /// Single word lookup — returns JSON array of postings with locations
    #[arg(short, long)]
    word: Option<String>,
    /// BM25 query — returns JSON array of {doc_id, score} sorted by relevance
    #[arg(short, long)]
    query: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let idx = index_io::IndexIo::open(&cli.index)?.into_search_index();

    match (cli.word, cli.query) {
        (Some(word), _) => {
            let postings = match idx.search(&word) {
                Some(result) => result
                    .postings
                    .iter()
                    .map(|p| serde_json::json!({"doc_id": p.doc_id, "locations": p.locations}))
                    .collect::<Vec<_>>(),
                None => vec![],
            };
            println!("{}", serde_json::to_string(&postings)?);
        }
        (_, Some(query)) => {
            let terms: Vec<&str> = query.split_whitespace().collect();
            let results = idx.bm25_search(&terms);
            let json: Vec<_> = results
                .iter()
                .map(|r| serde_json::json!({"doc_id": r.doc_id, "score": r.score}))
                .collect();
            println!("{}", serde_json::to_string(&json)?);
        }
        _ => {
            eprintln!("Provide either --word or --query");
            std::process::exit(1);
        }
    }

    Ok(())
}
