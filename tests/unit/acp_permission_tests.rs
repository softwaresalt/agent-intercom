//! Unit tests for ACP `session/request_permission` conformance parsing (T8.1).
//!
//! Verifies the standard ACP permission request (ADR-0016) is parsed into an
//! [`AgentEvent::PermissionRequested`] instead of being silently dropped.

use agent_intercom::acp::reader::parse_inbound_line;
use agent_intercom::driver::AgentEvent;

#[test]
fn parses_session_request_permission_into_permission_requested() {
    let line = r#"{"jsonrpc":"2.0","id":"req-1","method":"session/request_permission","params":{"sessionId":"agent-sess-1","toolCall":{"title":"Write config","kind":"edit","locations":[{"path":"config/server.toml"}]},"options":[{"optionId":"allow-once","name":"Allow","kind":"allow_once"},{"optionId":"reject-once","name":"Reject","kind":"reject_once"}]}}"#;

    let event = parse_inbound_line("sess-int-1", line)
        .expect("parse must succeed")
        .expect("must emit an event");

    match event {
        AgentEvent::PermissionRequested {
            request_id,
            session_id,
            title,
            file_path,
            options,
            ..
        } => {
            assert_eq!(request_id, "req-1");
            assert_eq!(session_id, "sess-int-1");
            assert_eq!(title, "Write config");
            assert_eq!(file_path, "config/server.toml");
            assert_eq!(options.len(), 2);
            assert_eq!(options[0].option_id, "allow-once");
            assert_eq!(options[0].kind, "allow_once");
            assert_eq!(options[1].option_id, "reject-once");
        }
        other => panic!("expected PermissionRequested, got {other:?}"),
    }
}

#[test]
fn session_request_permission_missing_id_errors() {
    let line = r#"{"jsonrpc":"2.0","method":"session/request_permission","params":{"sessionId":"a","toolCall":{},"options":[]}}"#;

    let result = parse_inbound_line("sess-int-2", line);

    assert!(
        result.is_err(),
        "a permission request without `id` must error"
    );
}

#[test]
fn session_request_permission_missing_title_and_location_uses_defaults() {
    let line = r#"{"jsonrpc":"2.0","id":"req-3","method":"session/request_permission","params":{"sessionId":"a","toolCall":{},"options":[{"optionId":"allow","name":"Allow","kind":"allow_once"}]}}"#;

    let event = parse_inbound_line("sess-int-3", line)
        .expect("parse must succeed")
        .expect("must emit an event");

    match event {
        AgentEvent::PermissionRequested {
            title, file_path, ..
        } => {
            assert!(
                !title.is_empty(),
                "title must fall back to a non-empty default"
            );
            assert_eq!(file_path, "", "no tool-call location → empty file_path");
        }
        other => panic!("expected PermissionRequested, got {other:?}"),
    }
}

#[test]
fn parses_numeric_json_rpc_id() {
    // Real conformant ACP agents (e.g. copilot --acp) use numeric JSON-RPC ids.
    let line = r#"{"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{"sessionId":"a","toolCall":{"title":"Run tests"},"options":[{"optionId":"allow","name":"Allow","kind":"allow_once"}]}}"#;

    let event = parse_inbound_line("sess-int-num", line)
        .expect("parse must succeed for a numeric id")
        .expect("must emit an event");

    match event {
        AgentEvent::PermissionRequested {
            request_id,
            request_id_raw,
            ..
        } => {
            assert_eq!(request_id, "7", "numeric id is keyed as its decimal string");
            assert!(
                request_id_raw.is_number(),
                "raw id must be preserved as a number"
            );
            assert_eq!(request_id_raw, serde_json::json!(7));
        }
        other => panic!("expected PermissionRequested, got {other:?}"),
    }
}
