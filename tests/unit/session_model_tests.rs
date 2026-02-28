//! Unit tests for `ProtocolMode` serde serialization (T005).

use agent_intercom::models::session::ProtocolMode;

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
