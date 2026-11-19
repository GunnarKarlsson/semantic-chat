mod cache;
mod embedder;
mod llm;
mod models;

use std::sync::Arc;

use axum::{routing::get, Router};
use tower_http::services::ServeDir;

use embedder::{Embedder, SharedEmbedder};

#[derive(Clone)]
struct AppState {
    /// Wired into routes in a later step.
    #[allow(dead_code)]
    embedder: SharedEmbedder,
}

#[tokio::main]
async fn main() {
    println!("loading embedding model (all-MiniLM-L6-v2)...");
    let embedder = Arc::new(Embedder::load().expect("failed to load embedding model"));
    let probe = embedder
        .embed("ping")
        .expect("failed to run embedding smoke test");
    println!("embedding model ready ({} dims)", probe.len());

    let state = AppState { embedder };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    println!("listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.expect("server error");
}
