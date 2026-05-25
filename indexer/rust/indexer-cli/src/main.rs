use anyhow::Result;
use clap::{Parser, Subcommand};
use tools::collect_ascii_books;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Index files from a JSON file
    Index {
        /// Path to the JSON file containing the list of files to index
        #[arg(short, long)]
        indexer_input: String,
        /// Path to the output index file
        #[arg(short, long)]
        output_db: String,
    },
    /// General utilities
    Tools {
        #[command(subcommand)]
        command: ToolsCommands,
    },
}

#[derive(Subcommand)]
enum ToolsCommands {
    /// Collect ASCII books from a library directory
    CollectAscii {
        /// Path to the library directory
        #[arg(short, long)]
        library_dir: String,
        /// Path to save the indexer input JSON file to
        #[arg(long)]
        save_indexer_input_json_to: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Index {
            indexer_input,
            output_db,
        }) => {
            indexer::pipeline::index_files(indexer_input, output_db)?;
        }
        Some(Commands::Tools { command }) => match command {
            ToolsCommands::CollectAscii {
                library_dir,
                save_indexer_input_json_to,
            } => {
                collect_ascii_books(library_dir, save_indexer_input_json_to)?;
            }
        },
        None => {
            println!("No command provided");
        }
    }

    Ok(())
}
