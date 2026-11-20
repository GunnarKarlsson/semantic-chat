mod cache;
mod chat_completions_api;
mod db;
mod embedder;
mod models;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

use cache::{load_shared, SharedSemanticCache};
use chat_completions_api::{
    ChatCompletionsApiClient, CompletionsMessage, SharedChatCompletionsApiClient,
};
use db::SharedDb;
use embedder::{Embedder, SharedEmbedder};
use models::{
    load_shared_store, ChatSession, ChatStoreError, ChatSummary, CreateChatResponse, Message,
    Role, SendMessageRequest, SendMessageResponse, SettingsResponse, SharedChatStore,
};

#[derive(Clone)]
struct AppState {
    /// Shared with cache/chats; kept for future settings writes.
    #[allow(dead_code)]
    db: SharedDb,
    embedder: SharedEmbedder,
    cache: SharedSemanticCache,
    chat_completions_api: Option<SharedChatCompletionsApiClient>,
    chats: SharedChatStore,
    /// Loaded from SQLite `settings` at startup.
    similarity_threshold: f32,
}

#[tokio::main]
async fn main() {
    match dotenvy::dotenv() {
        Ok(path) => println!("loaded env from {}", path.display()),
        Err(err) if err.not_found() => {
            println!("no .env file found (using process environment only)");
        }
        Err(err) => {
            eprintln!("warning: could not load .env: {err}");
        }
    }

    let db = match db::open_default() {
        Ok((db, path)) => {
            println!("sqlite ready at {}", path.display());
            db
        }
        Err(err) => {
            eprintln!("failed to open sqlite database: {err}");
            std::process::exit(1);
        }
    };

    println!("loading embedding model (all-MiniLM-L6-v2) on CPU...");
    println!(
        "note: first run may download model files from Hugging Face; later runs use the local cache"
    );

    let embedder = match Embedder::load() {
        Ok(model) => {
            println!("embedding model ready");
            Arc::new(model)
        }
        Err(err) => {
            eprintln!("failed to load embedding model: {err}");
            eprintln!(
                "if this is the first run, check network access to Hugging Face, then try again"
            );
            std::process::exit(1);
        }
    };

    let similarity_threshold = {
        let conn = db.lock().expect("db lock poisoned");
        match db::load_similarity_threshold(&conn) {
            Ok(threshold) => {
                println!("similarity threshold {threshold}");
                threshold
            }
            Err(err) => {
                eprintln!("failed to load similarity threshold: {err}");
                std::process::exit(1);
            }
        }
    };

    let cache = match load_shared(db.clone()) {
        Ok(cache) => {
            let count = cache.read().expect("cache lock poisoned").len();
            println!("semantic cache ready ({count} persisted entries)");
            cache
        }
        Err(err) => {
            eprintln!("failed to load semantic cache from sqlite: {err}");
            std::process::exit(1);
        }
    };

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
            println!(
                "set CHAT_API_BASE_URL, CHAT_API_KEY, and CHAT_MODEL to enable replies on cache miss"
            );
            None
        }
    };

    let chats = match load_shared_store(db.clone()) {
        Ok(chats) => {
            let count = chats.read().expect("chat store lock poisoned").len();
            println!("chat store ready ({count} persisted chats)");
            chats
        }
        Err(err) => {
            eprintln!("failed to load chats from sqlite: {err}");
            std::process::exit(1);
        }
    };

    let static_dir = static_dir_path();
    if !static_dir.is_dir() {
        eprintln!(
            "warning: static directory not found at {} (UI assets will 404)",
            static_dir.display()
        );
    } else {
        println!("serving UI from {}", static_dir.display());
    }

    let state = AppState {
        db,
        embedder,
        cache,
        chat_completions_api,
        chats,
        similarity_threshold,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/messages", post(send_message))
        .route("/api/chats", get(list_chats).post(create_chat))
        .route("/api/chats/{id}", get(get_chat))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/clear-cache", post(clear_cache))
        .fallback_service(
            ServeDir::new(static_dir).append_index_html_on_directories(true),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("failed to bind to {addr}: {err}");
            std::process::exit(1);
        });

    println!("listening on http://localhost:3000");
    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("server error: {err}");
        std::process::exit(1);
    }
}

fn static_dir_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("static")
}

async fn health() -> &'static str {
    "ok"
}

async fn get_settings(State(state): State<AppState>) -> Json<SettingsResponse> {
    let cache_entry_count = state.cache.read().expect("cache lock poisoned").len();
    Json(SettingsResponse {
        similarity_threshold: state.similarity_threshold,
        cache_entry_count,
    })
}

async fn clear_cache(State(state): State<AppState>) -> Result<Json<SettingsResponse>, ApiError> {
    let cache_entry_count = {
        let mut cache = state.cache.write().expect("cache lock poisoned");
        let before = cache.len();
        cache
            .clear()
            .map_err(|e| ApiError::Internal(format!("failed to clear cache: {e}")))?;
        println!("cleared semantic cache ({before} entries removed)");
        cache.len()
    };

    Ok(Json(SettingsResponse {
        similarity_threshold: state.similarity_threshold,
        cache_entry_count,
    }))
}

async fn list_chats(State(state): State<AppState>) -> Json<Vec<ChatSummary>> {
    let chats = state.chats.read().expect("chat store lock poisoned");
    Json(chats.list())
}

async fn create_chat(
    State(state): State<AppState>,
) -> Result<Json<CreateChatResponse>, ApiError> {
    let mut chats = state.chats.write().expect("chat store lock poisoned");
    let session = chats
        .create()
        .map_err(|e| ApiError::Internal(format!("failed to persist chat: {e}")))?;
    Ok(Json(CreateChatResponse::from(&session)))
}

async fn get_chat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ChatSession>, ApiError> {
    let chats = state.chats.read().expect("chat store lock poisoned");
    let session = chats
        .get(id)
        .cloned()
        .ok_or(ApiError::NotFound(format!("chat not found: {id}")))?;
    Ok(Json(session))
}

async fn send_message(
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, ApiError> {
    let message = request.message.trim().to_string();
    if message.is_empty() {
        return Err(ApiError::BadRequest("message must not be empty".into()));
    }

    let prior_messages = {
        let chats = state.chats.read().expect("chat store lock poisoned");
        let session = chats.get(request.chat_id).ok_or(ApiError::NotFound(format!(
            "chat not found: {}",
            request.chat_id
        )))?;
        session.messages.clone()
    };

    let embedder = state.embedder.clone();
    let embed_text = message.clone();
    let embedding = tokio::task::spawn_blocking(move || embedder.embed(&embed_text))
        .await
        .map_err(|e| ApiError::Internal(format!("embedding task failed: {e}")))?
        .map_err(|e| ApiError::Internal(format!("embedding failed: {e}")))?;

    // Only use the semantic cache for the first turn in a thread. Follow-ups need
    // full Completions history, which a single-prompt cache entry does not capture.
    let cache_hit = if prior_messages.is_empty() {
        let cache = state.cache.read().expect("cache lock poisoned");
        cache.search(&embedding, state.similarity_threshold)
    } else {
        None
    };

    let (reply, from_cache) = if let Some(entry) = cache_hit {
        println!(
            "cache hit for chat {} (entry from chat {}, prompt {:?}, at {})",
            request.chat_id, entry.chat_id, entry.prompt, entry.timestamp
        );
        (entry.response, true)
    } else {
        println!(
            "cache miss for chat {} (prior turns: {})",
            request.chat_id,
            prior_messages.len()
        );
        let client = state.chat_completions_api.as_ref().ok_or_else(|| {
            ApiError::ServiceUnavailable(
                "chat completions API is not configured (set CHAT_API_BASE_URL, CHAT_API_KEY, CHAT_MODEL)".into(),
            )
        })?;

        let mut completions_messages: Vec<CompletionsMessage> = prior_messages
            .iter()
            .map(|m| match m.role {
                Role::User => CompletionsMessage::user(m.content.clone()),
                Role::Assistant => CompletionsMessage::assistant(m.content.clone()),
            })
            .collect();
        completions_messages.push(CompletionsMessage::user(message.clone()));

        println!(
            "calling chat completions API (model {}, {} messages)",
            client.model(),
            completions_messages.len()
        );
        let reply = client
            .complete(&completions_messages)
            .await
            .map_err(|e| ApiError::Internal(format!("chat completions failed: {e}")))?;
        println!(
            "chat completions reply received for chat {} ({} chars)",
            request.chat_id,
            reply.len()
        );

        // Cache standalone Q&A from first-turn misses only.
        if prior_messages.is_empty() {
            let mut cache = state.cache.write().expect("cache lock poisoned");
            cache
                .insert(embedding, message.clone(), reply.clone(), request.chat_id)
                .map_err(|e| ApiError::Internal(format!("failed to persist cache entry: {e}")))?;
            println!("stored new cache entry for chat {}", request.chat_id);
        }

        (reply, false)
    };

    // Always append to the *current* UI chat, including on cache hit.
    let title = {
        let mut chats = state.chats.write().expect("chat store lock poisoned");
        chats
            .append_message(request.chat_id, Message::user(message))
            .map_err(map_chat_store_error)?;
        chats
            .append_message(request.chat_id, Message::assistant(reply.clone()))
            .map_err(map_chat_store_error)?;
        chats
            .get(request.chat_id)
            .map(|session| session.title.clone())
            .ok_or_else(|| ApiError::NotFound(format!("chat not found: {}", request.chat_id)))?
    };

    Ok(Json(SendMessageResponse {
        chat_id: request.chat_id,
        reply,
        cache_hit: from_cache,
        title,
    }))
}

fn map_chat_store_error(err: ChatStoreError) -> ApiError {
    match err {
        ChatStoreError::NotFound(id) => ApiError::NotFound(format!("chat not found: {id}")),
        ChatStoreError::Db(err) => {
            ApiError::Internal(format!("failed to persist chat message: {err}"))
        }
    }
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    NotFound(String),
    ServiceUnavailable(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::ServiceUnavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
            ApiError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, message).into_response()
    }
}
