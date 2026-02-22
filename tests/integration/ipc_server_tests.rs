//! Integration tests for IPC server command dispatch and authentication.
//!
//! Validates:
//! - S053: Valid auth token accepted
//! - S054: Invalid auth token rejected
//! - S055: Missing auth token rejected
//! - S057: `list` command returns active sessions
//! - S059: `approve` resolves pending approval via oneshot
//! - S060: `reject` resolves with reason via oneshot
//! - S062: `resume` resolves pending wait via oneshot
//! - S064: `mode` command changes session operational mode
//!
//! FR-008 — IPC Server Command Dispatch
