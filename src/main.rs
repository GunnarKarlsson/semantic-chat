mod cache;
mod chat_completions_api;
mod embedder;
mod models;

use std::sync::Arc;

use axum::{routing::get, Router};
use tower_http::services::ServeDir;
use uuid::Uuid;

use cache::{new_shared, SharedSemanticCache, DEFAULT_THRESHOLD};
use chat_completions_api::{ChatCompletionsApiClient, SharedChatCompletionsApiClient};
use embedder::{Embedder, SharedEmbedder};
use models::{
    new_shared_store, CreateChatResponse, Message, SendMessageRequest, SendMessageResponse,
    SharedChatStore,
};

#[derive(Clone)]
struct AppState {
    /// Wired into routes in a later step.
    #[allow(dead_code)]
    embedder: SharedEmbedder,
    /// Wired into routes in a later step.
    #[allow(dead_code)]
    cache: SharedSemanticCache,
    /// Wired into routes in a later step. Absent when env is not configured.
    #[allow(dead_code)]
    chat_completions_api: Option<SharedChatCompletionsApiClient>,
    /// Wired into routes in a later step.
    #[allow(dead_code)]
    chats: SharedChatStore,
}

#[tokio::main]
async fn main() {
    println!("loading embedding model (all-MiniLM-L6-v2)...");
    let embedder = Arc::new(Embedder::load().expect("failed to load embedding model"));
    let probe = embedder
        .embed("ping")
        .expect("failed to run embedding smoke test");
    println!("embedding model ready ({} dims)", probe.len());

    let cache = new_shared();
    {
        let mut guard = cache.write().expect("cache lock poisoned");
        guard.insert(probe.clone(), "ping", "pong", Uuid::nil());
        let hit = guard
            .search(&probe, DEFAULT_THRESHOLD)
            .expect("cache smoke test missed exact embedding");
        assert_eq!(hit.prompt, "ping");
        assert_eq!(hit.response, "pong");
        assert_eq!(hit.chat_id, Uuid::nil());
        assert_eq!(hit.embedding.len(), probe.len());
        let _ = hit.timestamp;
        println!("semantic cache ready");
    }

    let chat_completions_api = match ChatCompletionsApiClient::from_env() {
        Ok(client) => {
            println!(
                "chat completions API client ready (model {})",
                client.model()
            );
            Some(Arc::new(client))
        }
        Err(err) => {
            println!("chat completions API client not configured: {err}");
            None
        }
    };

    let chats = new_shared_store();
    {
        let mut guard = chats.write().expect("chat store lock poisoned");
        let session = guard.create();
        let id = session.id;
        guard
            .append_message(id, Message::user("hello"))
            .expect("failed to append user message");
        guard
            .append_message(id, Message::assistant("hi"))
            .expect("failed to append assistant message");
        let listed = guard.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "hello");
        let session = guard.get(id).expect("chat missing after create");
        assert_eq!(session.messages.len(), 2);
        let _ = CreateChatResponse::from(session);
        let req = SendMessageRequest {
            chat_id: id,
            message: "hello".to_string(),
        };
        assert_eq!(req.chat_id, id);
        assert_eq!(req.message, "hello");
        let resp = SendMessageResponse {
            chat_id: id,
            reply: "hi".to_string(),
            cache_hit: false,
        };
        assert_eq!(resp.reply, "hi");
        assert!(!resp.cache_hit);
        println!("chat store ready");
    }

    let state = AppState {
        embedder,
        cache,
        chat_completions_api,
        chats,
    };

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
