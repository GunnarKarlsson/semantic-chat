# Semantic Chat

A local chat app with a **semantic cache**: similar questions reuse a previous answer instead of calling the remote model again.

The Rust binary (Axum) serves both the JSON API and the static UI in `static/`. Chats, cache entries, and settings are stored in a local SQLite file.

## Requirements

- Rust toolchain (`cargo`)
- Network on first run (downloads `all-MiniLM-L6-v2` from Hugging Face into the local HF cache)
- Chat Completions credentials (any OpenAI-compatible provider)

## Configure Chat Completions

Copy `.env.example` to `.env` (or edit the existing `.env`) in the project root:

| Variable | Meaning | Example |
| --- | --- | --- |
| `CHAT_API_BASE_URL` | API base URL (no trailing `/v1`) | `https://api.deepseek.com` or `https://api.openai.com` |
| `CHAT_API_KEY` | Bearer token | your API key |
| `CHAT_MODEL` | Model id | `deepseek-chat` or `gpt-4o-mini` |

```bash
cp .env.example .env
# edit .env and replace the placeholder key
```

`cargo run` loads `.env` automatically. You can also export the same variables in your shell instead.

Without these, the server still starts (embeddings + UI chats work), but a cache **miss** returns HTTP 503 until Completions is configured.

## Run

From the project root:

```bash
cargo run
```

Then open [http://localhost:3000](http://localhost:3000).

Useful checks:

- Health: [http://localhost:3000/health](http://localhost:3000/health) → `ok`
- First startup may take a while while the embedding model downloads; later runs reuse the cache under `~/.cache/huggingface/` (typical on macOS/Linux).

## Data on disk

SQLite lives at `data/semantic-chat.db` (created on first run). That directory is gitignored.

It stores:

- Semantic cache entries (embeddings + prompt/response pairs)
- UI chats and messages
- Settings (including the similarity threshold)

## UI extras

- **Settings (gear in the sidebar):** shows how many answers are cached; **Clear cache** wipes cache entries only (chats stay).
- **Delete chat:** the × on each sidebar row removes that chat and its messages. The global semantic cache is unchanged.

## How it works

1. You send a message in a UI chat.
2. The server embeds the text with local MiniLM.
3. The semantic cache looks for a close match (cosine similarity from settings, default 0.92).
4. **Hit** → return the cached answer (also appended to the current chat).
5. **Miss** → call Chat Completions, store the pair in the cache, return the reply.

The semantic cache is **global** across sidebar chats; chat history is per session. Both survive restarts via SQLite.
