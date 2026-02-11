# ADR-0001: Credential Loading via Keyring with Environment Variable Fallback

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 2 (Foundational), Task T006

## Context

The server needs Slack API tokens (app token for Socket Mode, bot token for message posting) at runtime. Storing tokens in the TOML config file creates a security risk — config files are often committed to version control or stored on disk in plaintext. The spec (FR-036) requires that tokens not reside in the configuration file.

## Decision

Tokens are loaded at runtime via a two-tier strategy:

1. **OS keychain first** — `keyring` crate with service name `monocoque-agent-rem` and key names `slack_app_token` / `slack_bot_token`. The keychain lookup runs inside `tokio::task::spawn_blocking` because the `keyring` crate performs synchronous I/O.
2. **Environment variable fallback** — `SLACK_APP_TOKEN` / `SLACK_BOT_TOKEN` checked if the keychain lookup fails or returns an empty string.

The `SlackConfig` struct uses `#[serde(skip)]` on both token fields so deserialization never reads them from TOML, and `load_credentials()` is an explicit async method called during bootstrap.

## Consequences

**Positive**:
- Tokens never appear in config files or version control.
- Environment variables provide a familiar CI/CD-friendly fallback.
- OS keychain is the most secure option for developer workstations.

**Negative**:
- Requires the `keyring` crate dependency (~25 KB, platform-specific backends).
- `spawn_blocking` adds a minor overhead on first startup.
- Users must populate the keychain manually or set env vars.

**Risks**:
- Keychain access may fail on headless Linux without a desktop keyring daemon; the env-var fallback mitigates this.
