# Semantic Chat

A local chat app with a **semantic cache**: similar questions reuse a previous answer instead of calling the remote model again.

The Rust binary (Axum) serves both the JSON API and the static UI in `static/`.

## Requirements

- Rust toolchain (`cargo`)
- Network on first run (downloads `all-MiniLM-L6-v2` from Hugging Face into the local HF cache)
- Chat Completions credentials (any OpenAI-compatible provider)

## Configure Chat Completions

Set these environment variables before starting the server:

| Variable | Meaning | Example |
| --- | --- | --- |
| `CHAT_API_BASE_URL` | API base URL (no trailing `/v1`) | `https://api.deepseek.com` or `https://api.openai.com` |
| `CHAT_API_KEY` | Bearer token | your API key |
| `CHAT_MODEL` | Model id | `deepseek-chat` or `gpt-4o-mini` |

### Example (zsh / bash)

```bash
export CHAT_API_BASE_URL="https://api.deepseek.com"
export CHAT_API_KEY="sk-..."
export CHAT_MODEL="deepseek-chat"
```

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

## How it works

1. You send a message in a UI chat.
2. The server embeds the text with local MiniLM.
3. The in-memory semantic cache looks for a close match (cosine similarity ≥ 0.92).
4. **Hit** → return the cached answer (also appended to the current chat).
5. **Miss** → call Chat Completions, store the pair in the cache, return the reply.

The semantic cache is **global** across sidebar chats; chat history is per session.
