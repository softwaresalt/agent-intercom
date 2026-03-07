//! Unit tests for `ServerMode` CLI enum parsing (T015).
//!
//! Covers scenarios S001 (MCP default), S002 (ACP explicit),
//! S003 (invalid value rejected), S006 (possible values have names).

use agent_intercom::mode::ServerMode;
use clap::ValueEnum as _;

#[test]
fn server_mode_default_is_mcp() {
    let mode = ServerMode::default();
    assert_eq!(mode, ServerMode::Mcp, "default mode must be Mcp (S001)");
}

#[test]
fn server_mode_mcp_parsed_from_string() {
    let mode = ServerMode::from_str("mcp", false).expect("mcp is a valid mode");
    assert_eq!(mode, ServerMode::Mcp);
}

#[test]
fn server_mode_acp_parsed_from_string() {
    let mode = ServerMode::from_str("acp", false).expect("acp is a valid mode (S002)");
    assert_eq!(mode, ServerMode::Acp);
}

#[test]
fn server_mode_invalid_value_rejected() {
    let result = ServerMode::from_str("xyz", false);
    assert!(result.is_err(), "invalid mode must be rejected (S003)");
}

#[test]
fn server_mode_mcp_possible_value_name_is_mcp() {
    let val = ServerMode::Mcp
        .to_possible_value()
        .expect("has possible value (S006)");
    assert_eq!(val.get_name(), "mcp");
}

#[test]
fn server_mode_acp_possible_value_name_is_acp() {
    let val = ServerMode::Acp
        .to_possible_value()
        .expect("has possible value (S006)");
    assert_eq!(val.get_name(), "acp");
}

#[test]
fn server_mode_all_variants_have_possible_values() {
    for mode in [ServerMode::Mcp, ServerMode::Acp] {
        assert!(
            mode.to_possible_value().is_some(),
            "mode {mode:?} must have a ValueEnum possible value"
        );
    }
}

#[test]
fn server_mode_is_copy() {
    let mode = ServerMode::Acp;
    let copy = mode;
    assert_eq!(mode, copy);
}
