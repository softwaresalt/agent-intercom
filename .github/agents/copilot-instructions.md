# monocoque-agent-rc Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-10

## Active Technologies
- [e.g., Python 3.11, Swift 5.9, Rust 1.75 or NEEDS CLARIFICATION] + [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION] (002-sqlite-migration)
- [if applicable, e.g., PostgreSQL, CoreData, files or N/A] (002-sqlite-migration)
- Rust stable, edition 2021 + sqlx 0.8 (runtime-tokio, sqlite, json, chrono, macros), axum 0.8, rmcp 0.5, slack-morphism 2.17, tokio 1.37 (002-sqlite-migration)
- SQLite (file-backed via sqlx bundled libsqlite3; WAL journal mode; single-writer pool max_connections=1) (002-sqlite-migration)
- Rust stable, edition 2021 + `rmcp` 0.5, `axum` 0.8, `tokio` 1.37, `sqlx` 0.8, `interprocess` 2.0, `notify` 6.1, `tempfile` 3.10 (001-002-integration-test)
- SQLite in-memory via sqlx (test-only) (001-002-integration-test)

- Rust (stable, edition 2021) + `rmcp` 0.5, `slack-morphism` 2.17, `axum` 0.8, `tokio` 1.37, `serde`/`serde_json`, `diffy` 0.4, `notify` 6.1, `interprocess` 2.0, `clap` 4.5, `tracing`/`tracing-subscriber` 0.3 (001-mcp-remote-agent-server)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test; cargo clippy

## Code Style

Rust (stable, edition 2021): Follow standard conventions

## Recent Changes
- 001-002-integration-test: Added Rust stable, edition 2021 + `rmcp` 0.5, `axum` 0.8, `tokio` 1.37, `sqlx` 0.8, `interprocess` 2.0, `notify` 6.1, `tempfile` 3.10
- 002-sqlite-migration: Added Rust stable, edition 2021 + sqlx 0.8 (runtime-tokio, sqlite, json, chrono, macros), axum 0.8, rmcp 0.5, slack-morphism 2.17, tokio 1.37
- 002-sqlite-migration: Added [e.g., Python 3.11, Swift 5.9, Rust 1.75 or NEEDS CLARIFICATION] + [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION]


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
