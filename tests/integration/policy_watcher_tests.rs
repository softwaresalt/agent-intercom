//! Integration tests for policy hot-reload via `PolicyWatcher`.
//!
//! Validates:
//! - S045: Register loads initial policy from settings.json
//! - S046: File modification is detected and policy cache updated
//! - S047: File deletion falls back to deny-all default
//! - S048: Malformed JSON file falls back to deny-all default
//! - S049: Unregister stops watching for changes
//! - S050: Multiple workspaces have independent policies
//!
//! FR-007 — Policy Hot-Reload
