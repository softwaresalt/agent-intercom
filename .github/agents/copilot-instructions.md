# agent-intercom Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-03-07

## Active Technologies
- Rust stable, edition 2021 + rmcp 0.13 (MCP), slack-morphism 2.17 (Slack), sqlx 0.8 (SQLite), tokio 1.37 (async), uuid 1.7 (ID generation) (007-acp-correctness-mobile)
- SQLite via sqlx (file-based prod, in-memory tests) (007-acp-correctness-mobile)

- Rust stable, edition 2021 + rmcp 0.13, axum 0.8, slack-morphism 2.17, tokio 1.37, sqlx 0.8, sha2 0.10 (006-acp-event-wiring)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test; cargo clippy

## Code Style

Rust stable, edition 2021: Follow standard conventions

## Recent Changes
- 007-acp-correctness-mobile: Added Rust stable, edition 2021 + rmcp 0.13 (MCP), slack-morphism 2.17 (Slack), sqlx 0.8 (SQLite), tokio 1.37 (async), uuid 1.7 (ID generation)

- 006-acp-event-wiring: Added Rust stable, edition 2021 + rmcp 0.13, axum 0.8, slack-morphism 2.17, tokio 1.37, sqlx 0.8, sha2 0.10

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
