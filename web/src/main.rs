use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::get,
    Router,
};
use clap::Parser;
use index_io::IndexIo;
use std::{collections::HashMap, fs::File, io::Read, path::PathBuf, sync::Arc};
use zip::ZipArchive;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    index: String,
    #[arg(long)]
    corpus: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

struct AppState {
    search: index_io::search::SearchIndex,
    corpus: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let state = Arc::new(AppState {
        search: IndexIo::open(&cli.index)?.into_search_index(),
        corpus: PathBuf::from(cli.corpus),
    });
    let app = Router::new()
        .route("/", get(index_page))
        .route("/search", get(search_page))
        .route("/book/{id}", get(book_page))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.port)).await?;
    println!("Listening on http://0.0.0.0:{}", cli.port);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_page() -> Html<&'static str> {
    Html(
        "<html><body>\
        <h1>Gutenindex</h1>\
        <form action=\"/search\">\
        <input name=\"q\" size=\"50\">\
        <input type=\"submit\" value=\"Search\">\
        </form>\
        </body></html>",
    )
}

async fn search_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let q = params.get("q").cloned().unwrap_or_default();
    if q.is_empty() {
        return Html("<html><body><a href=\"/\">Back</a></body></html>".into());
    }
    let terms: Vec<&str> = q.split_whitespace().collect();
    let results = state.search.bm25_search(&terms);
    let mut html = format!("<html><body><h2>Results for &ldquo;{}&rdquo;</h2><ol>", esc(&q));
    for r in results.iter().take(10) {
        let title = state.search.doc_title(r.doc_id).unwrap_or("(untitled)");
        html.push_str(&format!(
            "<li><a href=\"/book/{}\">{}</a></li>",
            r.doc_id,
            esc(title)
        ));
    }
    html.push_str("</ol><a href=\"/\">New search</a></body></html>");
    Html(html)
}

async fn book_page(State(state): State<Arc<AppState>>, Path(doc_id): Path<u32>) -> Html<String> {
    let corpus = state.corpus.clone();
    let content = tokio::task::spawn_blocking(move || read_book(&corpus, doc_id))
        .await
        .unwrap_or(None);
    match content {
        Some(text) => Html(format!(
            "<html><body><a href=\"/\">Search</a><pre>{}</pre></body></html>",
            esc(&text)
        )),
        None => Html(
            "<html><body><p>Book not found.</p><a href=\"/\">Search</a></body></html>".into(),
        ),
    }
}

fn read_book(corpus: &std::path::Path, doc_id: u32) -> Option<String> {
    let bucket = doc_id % 10;
    let f = File::open(corpus.join(format!("{bucket}.zip"))).ok()?;
    let mut archive = ZipArchive::new(f).ok()?;
    let mut entry = archive.by_name(&format!("{doc_id}.txt")).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
