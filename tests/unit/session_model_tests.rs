//! Unit tests for `ProtocolMode` serde serialization (T005) and session title
//! truncation (T156 / HITL-002 / FR-049).

use agent_intercom::models::session::{truncate_session_title, ProtocolMode};

#[test]
fn protocol_mode_mcp_serializes_to_snake_case() {
    let json = serde_json::to_string(&ProtocolMode::Mcp).expect("serialize Mcp");
    assert_eq!(json, "\"mcp\"");
}

#[test]
fn protocol_mode_acp_serializes_to_snake_case() {
    let json = serde_json::to_string(&ProtocolMode::Acp).expect("serialize Acp");
    assert_eq!(json, "\"acp\"");
}

#[test]
fn protocol_mode_mcp_round_trips() {
    let original = ProtocolMode::Mcp;
    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: ProtocolMode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, ProtocolMode::Mcp);
}

#[test]
fn protocol_mode_acp_round_trips() {
    let original = ProtocolMode::Acp;
    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: ProtocolMode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, ProtocolMode::Acp);
}

#[test]
fn protocol_mode_deserializes_mcp_string() {
    let pm: ProtocolMode = serde_json::from_str("\"mcp\"").expect("deserialize mcp");
    assert_eq!(pm, ProtocolMode::Mcp);
}

#[test]
fn protocol_mode_deserializes_acp_string() {
    let pm: ProtocolMode = serde_json::from_str("\"acp\"").expect("deserialize acp");
    assert_eq!(pm, ProtocolMode::Acp);
}

#[test]
fn protocol_mode_invalid_string_fails_deserialization() {
    let result: Result<ProtocolMode, _> = serde_json::from_str("\"stdio\"");
    assert!(result.is_err(), "unknown mode should fail to deserialize");
}

#[test]
fn protocol_mode_both_variants_are_distinct() {
    assert_ne!(ProtocolMode::Mcp, ProtocolMode::Acp);
}

// ── T156 / S115 S116 ──────────────────────────────────────────────────────────

/// A short prompt is returned unchanged.
#[test]
fn truncate_session_title_short_prompt_unchanged() {
    let title = truncate_session_title("Hello world");
    assert_eq!(title, "Hello world");
}

/// A prompt at exactly 80 characters is returned unchanged.
#[test]
fn truncate_session_title_exactly_80_chars_unchanged() {
    let prompt = "a".repeat(80);
    let title = truncate_session_title(&prompt);
    assert_eq!(title.len(), 80, "80-char prompt must not be truncated");
    assert_eq!(title, prompt);
}

/// A prompt longer than 80 characters is truncated and ends with `"..."`.
#[test]
fn truncate_session_title_over_80_chars_appends_ellipsis() {
    let prompt = "a".repeat(100);
    let title = truncate_session_title(&prompt);
    assert!(
        title.ends_with("..."),
        "truncated title must end with '...'"
    );
    // Total length is at most 80 chars: 77 content chars + "...".
    assert_eq!(title.len(), 80, "truncated title must be exactly 80 chars");
    assert_eq!(&title[..77], &"a".repeat(77));
}

/// An empty prompt produces an empty title.
#[test]
fn truncate_session_title_empty_prompt() {
    let title = truncate_session_title("");
    assert_eq!(title, "");
}
