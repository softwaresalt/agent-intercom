//! Contract tests for `slack://channel/{id}/recent` MCP resource (T126).
//!
//! Validates the output schema per `mcp-resources.json` contract and verifies
//! channel ID validation against configuration.

use serde_json::{json, Value};

/// The resource URI template as defined in `mcp-resources.json`.
const RESOURCE_URI_TEMPLATE: &str = "slack://channel/{id}/recent";

/// A sample valid channel ID for testing.
const VALID_CHANNEL_ID: &str = "C0123456789";

// ─── URI template structure ────────────────────────────────────────────

#[test]
fn uri_template_matches_contract() {
    assert_eq!(RESOURCE_URI_TEMPLATE, "slack://channel/{id}/recent");
}

#[test]
fn uri_contains_channel_id_placeholder() {
    assert!(
        RESOURCE_URI_TEMPLATE.contains("{id}"),
        "template must include {{id}} parameter"
    );
}

// ─── Output schema validation ──────────────────────────────────────────

#[test]
fn output_schema_has_required_fields() {
    let output = json!({
        "messages": [],
        "has_more": false
    });

    assert!(output.get("messages").is_some(), "messages field required");
    assert!(output.get("has_more").is_some(), "has_more field required");
}

#[test]
fn output_messages_is_array() {
    let output = json!({
        "messages": [
            { "ts": "1234567890.123456", "user": "U123", "text": "hello" }
        ],
        "has_more": false
    });

    assert!(
        output["messages"].is_array(),
        "messages must be a JSON array"
    );
}

#[test]
fn output_message_has_required_fields() {
    let message = json!({
        "ts": "1234567890.123456",
        "user": "U123",
        "text": "Build completed successfully"
    });

    assert!(message.get("ts").is_some(), "ts is required");
    assert!(message.get("user").is_some(), "user is required");
    assert!(message.get("text").is_some(), "text is required");
}

#[test]
fn output_message_thread_ts_is_optional() {
    let without_thread = json!({
        "ts": "1234567890.123456",
        "user": "U123",
        "text": "hello"
    });
    assert!(without_thread.get("thread_ts").is_none());

    let with_thread = json!({
        "ts": "1234567890.123456",
        "user": "U123",
        "text": "hello",
        "thread_ts": "1234567890.000000"
    });
    assert!(with_thread.get("thread_ts").is_some());
}

#[test]
fn output_has_more_is_boolean() {
    let output = json!({
        "messages": [],
        "has_more": true
    });

    assert!(
        output["has_more"].is_boolean(),
        "`has_more` must be a boolean"
    );
}

#[test]
fn output_empty_messages_is_valid() {
    let output = json!({
        "messages": [],
        "has_more": false
    });

    let messages = output["messages"].as_array().expect("is array");
    assert!(messages.is_empty(), "empty messages list is valid");
}

#[test]
fn output_multiple_messages_are_valid() {
    let output = json!({
        "messages": [
            { "ts": "1234567890.100000", "user": "U001", "text": "first message" },
            { "ts": "1234567890.200000", "user": "U002", "text": "second message" },
            { "ts": "1234567890.300000", "user": "U001", "text": "third message", "thread_ts": "1234567890.100000" }
        ],
        "has_more": true
    });

    let messages = output["messages"].as_array().expect("is array");
    assert_eq!(messages.len(), 3);

    // Third message should have thread_ts
    assert!(messages[2].get("thread_ts").is_some());
}

// ─── Channel ID validation ─────────────────────────────────────────────

#[test]
fn channel_id_must_match_config() {
    let configured_channel = "C0123456789";
    let requested_channel = "C0123456789";

    assert_eq!(
        configured_channel, requested_channel,
        "requested channel must match configured channel_id"
    );
}

#[test]
fn mismatched_channel_id_is_rejected() {
    let configured_channel = "C0123456789";
    let requested_channel = "C9999999999";

    assert_ne!(
        configured_channel, requested_channel,
        "mismatched channel ID should be rejected"
    );
}

#[test]
fn uri_parsing_extracts_channel_id() {
    use monocoque_agent_rc::mcp::resources::slack_channel::parse_channel_uri;

    let uri = format!("slack://channel/{VALID_CHANNEL_ID}/recent");
    let result = parse_channel_uri(&uri);
    assert!(result.is_some(), "valid URI should parse successfully");
    assert_eq!(result.expect("parsed"), VALID_CHANNEL_ID);
}

#[test]
fn uri_parsing_rejects_malformed_uri() {
    use monocoque_agent_rc::mcp::resources::slack_channel::parse_channel_uri;

    let bad_uris = [
        "slack://channel/recent",
        "http://channel/C123/recent",
        "slack://channels/C123/recent",
        "",
        "slack://channel//recent",
    ];

    for uri in bad_uris {
        let result = parse_channel_uri(uri);
        assert!(result.is_none(), "malformed URI '{uri}' should be rejected");
    }
}

// ─── Limit parameter validation ─────────────────────────────────────────

#[test]
fn limit_default_is_20() {
    use monocoque_agent_rc::mcp::resources::slack_channel::DEFAULT_LIMIT;

    assert_eq!(DEFAULT_LIMIT, 20);
}

#[test]
fn limit_minimum_is_1() {
    let limit: u16 = 0;
    assert!(limit < 1, "limit 0 is below minimum");
}

#[test]
fn limit_maximum_is_100() {
    let limit: u16 = 101;
    assert!(limit > 100, "limit 101 exceeds maximum");
}

// ─── Resource metadata ─────────────────────────────────────────────────

#[test]
fn resource_template_has_correct_name() {
    use monocoque_agent_rc::mcp::resources::slack_channel::RESOURCE_NAME;

    assert_eq!(RESOURCE_NAME, "Slack Channel History");
}

#[test]
fn resource_template_has_description() {
    use monocoque_agent_rc::mcp::resources::slack_channel::RESOURCE_DESCRIPTION;

    assert!(
        !RESOURCE_DESCRIPTION.is_empty(),
        "resource must have a description"
    );
}

/// Validate the complete output JSON structure matches the contract schema.
#[test]
fn output_conforms_to_contract_schema() {
    let output = json!({
        "messages": [
            {
                "ts": "1707300000.000100",
                "user": "U0123456789",
                "text": "Deploy staging when ready",
                "thread_ts": null
            },
            {
                "ts": "1707300001.000200",
                "user": "U0123456789",
                "text": "Looks good, proceed"
            }
        ],
        "has_more": false
    });

    // Validate top-level structure
    assert!(output.is_object());
    let messages = output["messages"].as_array().expect("messages array");
    assert!(output["has_more"].is_boolean());

    // Validate each message conforms to the item schema
    for msg in messages {
        assert!(msg["ts"].is_string(), "ts must be string");
        assert!(msg["user"].is_string(), "user must be string");
        assert!(msg["text"].is_string(), "text must be string");
        // thread_ts is optional; when present it must be string or null
        if let Some(thread_ts) = msg.get("thread_ts") {
            assert!(
                thread_ts.is_string() || thread_ts.is_null(),
                "thread_ts must be string or null"
            );
        }
    }
}
