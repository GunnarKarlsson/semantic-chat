# Security Policy

## Supported versions

Security fixes are applied to the latest code on `main`. If tagged releases exist, the most recent release is also considered supported.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Report them privately using [GitHub Security Advisories](https://github.com/GunnarKarlsson/semantic-chat/security/advisories/new):

1. Go to the repository’s **Security** tab.
2. Choose **Advisories** → **New draft security advisory** (or use the link above).
3. Include a clear description, steps to reproduce, affected versions if known, and any suggested fix.

We aim to acknowledge reports promptly, typically within a few days. After triage, we will work with you on a fix and coordinated disclosure when appropriate.

## Scope notes

This project stores chat history and semantic-cache entries locally in SQLite, embeds text with a local model, and may call a user-configured OpenAI-compatible Chat Completions API on cache miss.

Reports related to secret leakage (API keys, `.env`), unsafe handling of credentials, path traversal around the `data/` directory, or XSS via rendered assistant Markdown are especially welcome.

Out of scope: third-party model-provider outages, Hugging Face model-host issues, and vulnerabilities solely in upstream crates once a fixed release is available and we have upgraded.
