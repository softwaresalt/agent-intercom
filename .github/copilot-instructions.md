# monocoque-agent-rem Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-09

## Active Technologies

- Rust (stable, edition 2021) + `rmcp` 0.5 (official MCP SDK), `slack-morphism` (Slack Socket Mode), `axum` 0.8 (HTTP/SSE transport), `tokio` (async runtime), `serde`/`serde_json`, `diffy` 0.4 (diff/patch), `notify` (fs watcher), `tracing`/`tracing-subscriber` (001-mcp-remote-agent-server)

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

- 001-mcp-remote-agent-server: Added Rust (stable, edition 2021) + `rmcp` 0.5 (official MCP SDK), `slack-morphism` (Slack Socket Mode), `axum` 0.8 (HTTP/SSE transport), `tokio` (async runtime), `serde`/`serde_json`, `diffy` 0.4 (diff/patch), `notify` (fs watcher), `tracing`/`tracing-subscriber`

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
