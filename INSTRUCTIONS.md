**Project Background / Intro for Cursor**

You are building a standalone semantic chat application in Rust called `semantic-chat`.

The core idea is a chat interface backed by a **semantic cache**. When a user sends a message, the system first checks whether a semantically similar question has already been answered. If it finds a close enough match (using embeddings + cosine similarity), it returns the cached answer instantly. If not, it calls an OpenAI-compatible Chat Completions API, stores the new question/answer pair in the cache, and returns the response.

Example of a Chat Completions integration (reference for the HTTP shape only):
https://github.com/GunnarKarlsson/android-dashboard

**Key technical components for Semantic Chat:**
- Local embedding model: `all-MiniLM-L6-v2` (via `sentence-transformers-rs` / Candle) running on the server
- In-memory semantic cache with brute-force cosine similarity (threshold ~0.92)
- OpenAI-compatible Chat Completions backend (called only on cache miss), configured via env
- Simple multi-chat support (list of previous conversations in a sidebar)

**User experience:**
- On first load, the user sees a clean start page with a single chat input box centered on the screen.
- After they type a message and press Enter, the app switches to the full chat interface.
- The full interface has:
  - A left sidebar showing previous chats (click to load)
  - A “+” button to start a new chat
  - The main area showing the current conversation
  - An input box at the bottom
- No delete functionality for now.

The application is a single Rust binary (Axum) that serves both the API and a simple static frontend. Everything (embeddings, cache, chat history) lives in memory on the server.

Your job is to implement this system cleanly, following the step-by-step instructions that will be provided. Prioritize clarity, correctness, and a pleasant minimal UI.

**Hard rule — each step must compile:** After finishing any instruction, run `cargo build` and fix errors before stopping. The binary must build successfully at the end of every step, even if later features are still stubs. Do not leave the tree in a broken compile state between steps.

Here are **10 step-by-step instructions** you can give to Cursor one by one (or in groups). They take everything from this thread and turn it into a standalone Rust chat app with a semantic cache.

---

**Instruction 1 – Project Setup**

Create a new Rust binary project named `semantic-chat`.  
Use Axum for the web server.  
Add these dependencies to `Cargo.toml`:

- axum
- tokio (full)
- serde + serde_json
- tower-http (cors + fs)
- sentence-transformers-rs
- candle-core
- reqwest (json)
- uuid
- chrono

Create the basic folder structure:
- `src/main.rs`
- `src/cache.rs`
- `src/embedder.rs`
- `src/chat_completions_api.rs`
- `src/models.rs`
- `static/` folder for frontend files

---

**Instruction 2 – Local Embedding Model**

Implement `src/embedder.rs`.  
Load `sentence-transformers/all-MiniLM-L6-v2` using `sentence-transformers-rs` on CPU.  
Expose a method `embed(&self, text: &str) -> Result<Vec<f32>>` that returns a 384-dimensional normalized vector.  
Handle model loading once at startup and share it safely (Arc).

---

**Instruction 3 – Semantic Cache**

Implement `src/cache.rs`.  
Create an in-memory `SemanticCache` that stores:
- embedding: `Vec<f32>`
- prompt
- response
- timestamp
- chat_id

Use brute-force cosine similarity.  
Add methods:
- `search(query_embedding, threshold) -> Option<CacheEntry>`
- `insert(...)`

Default threshold = 0.92.  
Make it thread-safe with `Arc<RwLock<...>>`.

---

**Instruction 4 – Chat Completions Client**

Implement `src/chat_completions_api.rs`.  
Create an async client for an OpenAI-compatible Chat Completions endpoint (`POST /v1/chat/completions`).  
On cache miss, send the user message and return the assistant reply.  
Configure everything from environment variables (no vendor-specific names in code):
- `CHAT_API_BASE_URL` — base URL (e.g. `https://api.deepseek.com` or `https://api.openai.com`)
- `CHAT_API_KEY` — bearer token
- `CHAT_MODEL` — model id (e.g. `deepseek-chat`, `gpt-4o-mini`)

Handle basic errors gracefully.

---

**Instruction 5 – Data Models & Chat Sessions**

In `src/models.rs` define:
- `ChatSession` (id, title, created_at, messages)
- `Message` (role, content, timestamp)
- Request/response structs for the API

In the main app state, keep a `HashMap<Uuid, ChatSession>` protected by a `RwLock`.  
Support creating a new chat and appending messages.

---

**Instruction 6 – Backend API Routes**

In `src/main.rs` set up an Axum server with these routes:

- `GET /` → serve the frontend
- `POST /api/chat` → main chat endpoint (does semantic cache lookup, falls back to Chat Completions)
- `GET /api/chats` → list all chat sessions (id + title + created_at)
- `GET /api/chats/:id` → get full message history of one chat
- `POST /api/chats` → create a new empty chat

The `/api/chat` endpoint must:
1. Embed the user message
2. Search the semantic cache
3. On hit → return cached response
4. On miss → call Chat Completions, store result in cache, return response

---

**Instruction 7 – Frontend Structure**

Create a clean single-page frontend in `static/`:

- `index.html`
- `style.css`
- `app.js`

Requirements:
- On first load: show a centered chat input box in the middle of the screen (start page).
- After the user types something and presses Enter: switch to the full chat interface.
- Full interface has:
  - Left sidebar listing previous chats
  - Main area showing the current chat thread
  - Input box at the bottom
  - A “+” button in the sidebar that starts a new chat

No delete functionality for now.

---

**Instruction 8 – Frontend Logic (Start Page + Chat View)**

In `app.js` implement:

- Start page behavior: one input in the center. On Enter, create a new chat, send the first message, then switch to the full chat layout.
- Sidebar: fetch `/api/chats` and render the list. Clicking a chat loads its messages via `/api/chats/:id`.
- “+” button: creates a new chat and clears the main view.
- Chat view: display messages (user right-aligned or styled differently from assistant), auto-scroll to bottom, and send new messages to `/api/chat`.

Keep the UI simple, clean, and modern (dark theme preferred).

---

**Instruction 9 – Wire Cache to Chats**

Update the cache and chat logic so that:
- Each cache entry is associated with a `chat_id` (optional but useful).
- When a semantic cache hit occurs, the response is still appended to the current chat session.
- The left sidebar titles are auto-generated from the first user message (truncated).

Ensure that switching between chats works correctly and the semantic cache is global (shared across chats).

---

**Instruction 10 – Final Polish & Run Instructions**

- Add basic CORS support.
- Serve static files correctly from the `static/` folder.
- Add a simple health check route.
- In `README.md` write clear run instructions:
  - How to set `CHAT_API_BASE_URL`, `CHAT_API_KEY`, and `CHAT_MODEL`
  - `cargo run`
  - Open `http://localhost:3000`
- Make sure the first-run model download of `all-MiniLM-L6-v2` is handled gracefully.
- Add a few helpful log messages (cache hit / cache miss / Chat Completions call).

---

You can now paste these instructions into Cursor one by one (or several at a time).  
Each instruction is self-contained enough for Cursor to implement the next piece of the system.  
After every instruction: `cargo build` must succeed before moving on.