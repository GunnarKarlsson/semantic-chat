# Contributing

Thank you for your interest in contributing to Semantic Chat. This document explains how to contribute to the project.

## Good first contributions

These areas are a solid place to start:

- **UI polish** — clearer empty/error states, accessibility, Markdown rendering edge cases.
- **Cache quality** — better hit/miss logging, threshold UX, or tests around similarity edge cases.
- **Docs** — clearer setup notes for different Chat Completions providers.

If you are unsure whether an idea fits, open an issue first and describe the intended user-facing change.

## Discuss internals before large work

Changes that touch core internals — for example embedding model loading, SQLite schema migrations, or the semantic-cache write-through path — should be discussed with the maintainer before you invest significant time. Open an issue or draft PR with the problem and proposed approach so scope and design can be agreed early.

## Development setup

See the [README](README.md) for prerequisites and how to configure and run the app:

```bash
cp .env.example .env
# edit .env with your Chat Completions credentials
cargo run
```

The project uses the toolchain in `rust-toolchain.toml` (currently Rust 1.89.0 with `clippy`, `rustfmt`, and `rust-analyzer`).

## Before you open a pull request

### Keep your branch current with `main`

Always rebase or merge the latest `main` into your branch before opening or updating a PR:

```bash
git fetch origin
git merge origin/main
# or: git rebase origin/main
```

Resolve any conflicts locally, re-run the checks below, then push.

### Run the same checks as CI

GitHub Actions runs [`.github/workflows/ci.yml`](.github/workflows/ci.yml). Locally, prefer the helper script:

```bash
./scripts/ci.sh
```

That runs `cargo fmt --check`, `clippy` (warnings as errors), `build`, `test`, and `cargo audit`.

You can also run the steps individually:

```bash
cargo fmt --all -- --check
cargo build --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo audit
```

Install [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) if needed (`cargo install cargo-audit --locked`). Advisory ignores for transitive candle/`zip` issues live in [`.cargo/audit.toml`](.cargo/audit.toml)—revisit when candle-core upgrades. Fix formatting with `cargo fmt --all` if the fmt check fails. Do not submit with Clippy warnings; CI treats warnings as errors (`-D warnings`).

## Pull request guidelines

- Prefer small, focused PRs that are easy to review.
- Describe **what** changed and **why** in the PR body. Link related issues when applicable.
- Match existing naming: UI chats vs Chat Completions client stay separate.
- Do not commit secrets — for example API keys, a filled-in `.env`, or `data/semantic-chat.db`.
- Ensure CI checks pass on the tip of your branch.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE) that covers this project.
