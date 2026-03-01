//! Unit tests for ACP NDJSON codec, stream reader, and outbound serialization (T069–T078).
//!
//! Covers:
//! - T069 (S049): single NDJSON message parses correctly
//! - T070 (S050): batched messages are each parsed
//! - T071 (S051): partial delivery is buffered until newline
//! - T072 (S052): malformed JSON returns parse error
//! - T073 (S053): unknown method is skipped without error
//! - T074 (S054): missing required field returns error
//! - T075 (S055): stream EOF emits `SessionTerminated`
//! - T076 (S056): outbound clearance response serializes correctly
//! - T077 (S057): max line length exceeded returns `AppError::Acp("line too long")`
//! - T078 (S058): empty line is silently skipped

use bytes::BytesMut;
use tokio::sync::mpsc;
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;

use agent_intercom::acp::codec::{AcpCodec, MAX_LINE_BYTES};
use agent_intercom::acp::reader::{parse_inbound_line, run_reader};
use agent_intercom::driver::AgentEvent;
use agent_intercom::AppError;

// ── T069 (S049): Single NDJSON message parses correctly ─────────────────────

/// A complete JSON object on a single newline-terminated line is decoded by
/// `AcpCodec` without error and returned as the line content (without the `\n`).
#[test]
fn single_ndjson_message_parses_correctly() {
    let mut codec = AcpCodec::new();
    let mut buf = BytesMut::from("{\"method\":\"heartbeat\",\"params\":{}}\n");

    let result = codec
        .decode(&mut buf)
        .expect("decode must succeed for a valid NDJSON line");

    assert_eq!(
        result,
        Some("{\"method\":\"heartbeat\",\"params\":{}}".to_owned()),
        "codec must return the line content without the trailing newline"
    );
}

// ── T070 (S050): Batched messages are each parsed ───────────────────────────

/// Two JSON objects delivered in a single buffer are decoded as two separate
/// items by successive `decode` calls.
#[test]
fn batched_messages_are_each_parsed() {
    let mut codec = AcpCodec::new();
    let raw = concat!(
        "{\"method\":\"heartbeat\",\"params\":{}}\n",
        "{\"method\":\"status/update\",\"params\":{\"message\":\"ok\"}}\n",
    );
    let mut buf = BytesMut::from(raw);

    let first = codec.decode(&mut buf).expect("first decode must succeed");
    assert!(first.is_some(), "first line must be decoded");

    let second = codec.decode(&mut buf).expect("second decode must succeed");
    assert!(second.is_some(), "second line must be decoded");

    let third = codec
        .decode(&mut buf)
        .expect("buffer now empty, decode must return None");
    assert!(third.is_none(), "no further lines must be present");
}

// ── T071 (S051): Partial delivery is buffered until newline ─────────────────

/// A JSON object that arrives without its terminating `\n` is not emitted yet;
/// once the newline arrives the complete line is yielded.
#[test]
fn partial_delivery_is_buffered_until_newline() {
    let mut codec = AcpCodec::new();

    // Feed the first fragment — no newline yet.
    let mut buf = BytesMut::from("{\"method\":\"heartbeat\"");
    let result = codec
        .decode(&mut buf)
        .expect("partial decode must not error");
    assert!(
        result.is_none(),
        "partial line must not be emitted before the newline arrives"
    );

    // Append the rest of the line including the newline.
    buf.extend_from_slice(b",\"params\":{}}\n");
    let result = codec
        .decode(&mut buf)
        .expect("decode must succeed after newline");
    assert!(
        result.is_some(),
        "complete line must be emitted after the newline arrives"
    );
}

// ── T072 (S052): Malformed JSON returns parse error ─────────────────────────

/// A line that is not valid JSON returns `AppError::Acp("malformed json: …")`.
#[test]
fn malformed_json_returns_parse_error() {
    let result = parse_inbound_line("sess-001", "not-valid-json{{{");

    match result {
        Err(AppError::Acp(msg)) => assert!(
            msg.contains("malformed json"),
            "error must mention 'malformed json', got: {msg}"
        ),
        other => panic!("expected Err(AppError::Acp), got: {other:?}"),
    }
}

// ── T073 (S053): Unknown method is skipped without error ────────────────────

/// A syntactically valid JSON line with an unrecognised `method` is silently
/// skipped — `parse_inbound_line` returns `Ok(None)`.
#[test]
fn unknown_method_is_skipped() {
    let result = parse_inbound_line("sess-001", r#"{"method":"intercom/unknown","params":{}}"#);

    assert!(
        matches!(result, Ok(None)),
        "unknown method must be silently skipped, got: {result:?}"
    );
}

// ── T074 (S054): Missing required field returns error ───────────────────────

/// A `clearance/request` message that is missing the required `file_path` field
/// returns `AppError::Acp` indicating a missing required field.
#[test]
fn missing_required_field_returns_error() {
    // `file_path` and `risk_level` are required; only `title` is provided.
    let json = r#"{"method":"clearance/request","id":"req-001","params":{"title":"Add module"}}"#;
    let result = parse_inbound_line("sess-001", json);

    assert!(
        matches!(result, Err(AppError::Acp(_))),
        "missing required field must return AppError::Acp, got: {result:?}"
    );
}

// ── T075 (S055): Stream EOF emits SessionTerminated ─────────────────────────

/// Reading to EOF on the agent stream causes `run_reader` to emit
/// `AgentEvent::SessionTerminated` with `reason: "stream closed"`.
#[tokio::test]
async fn stream_eof_emits_session_terminated() {
    let (tx, mut rx) = mpsc::channel(10);
    let cancel = CancellationToken::new();

    // Empty byte slice — immediate EOF.
    let empty: &[u8] = b"";

    run_reader("sess-eof".to_owned(), empty, tx, cancel, None)
        .await
        .expect("run_reader must return Ok(()) on clean EOF");

    let event = rx
        .recv()
        .await
        .expect("SessionTerminated must be emitted after EOF");

    match event {
        AgentEvent::SessionTerminated {
            session_id,
            exit_code,
            reason,
        } => {
            assert_eq!(session_id, "sess-eof");
            assert!(exit_code.is_none(), "stream close must have no exit code");
            assert!(
                reason.contains("stream closed"),
                "reason must contain 'stream closed', got: {reason}"
            );
        }
        other => panic!("expected AgentEvent::SessionTerminated, got: {other:?}"),
    }
}

// ── T076 (S056): Outbound clearance response serializes correctly ────────────

/// A `clearance/response` outbound message serializes to valid NDJSON with the
/// correct `method`, `id`, and `params.status` fields.
#[test]
fn outbound_clearance_response_serializes_correctly() {
    let msg = serde_json::json!({
        "method": "clearance/response",
        "id": "req-001",
        "params": {
            "status": "approved",
            "reason": null
        }
    });

    let serialized = msg.to_string();
    let parsed: serde_json::Value =
        serde_json::from_str(&serialized).expect("serialized message must be valid JSON");

    assert_eq!(parsed["method"], "clearance/response");
    assert_eq!(parsed["id"], "req-001");
    assert_eq!(parsed["params"]["status"], "approved");
    assert!(
        parsed["params"]["reason"].is_null(),
        "reason must be null when not provided"
    );

    // NDJSON requires a single-line encoding — no embedded newlines.
    assert!(
        !serialized.contains('\n'),
        "NDJSON line must not contain embedded newlines"
    );
}

// ── T077 (S057): Max line length exceeded returns error ─────────────────────

/// A line exceeding `MAX_LINE_BYTES` causes `AcpCodec::decode` to return
/// `AppError::Acp` containing `"line too long"`.
#[test]
fn max_line_length_exceeded_returns_error() {
    let mut codec = AcpCodec::new();

    // Build a line that exceeds MAX_LINE_BYTES bytes, followed by a newline.
    let big_line = "a".repeat(MAX_LINE_BYTES + 1) + "\n";
    let mut buf = BytesMut::from(big_line.as_str());

    let result = codec.decode(&mut buf);

    match result {
        Err(AppError::Acp(msg)) => assert!(
            msg.contains("line too long"),
            "error must mention 'line too long', got: {msg}"
        ),
        other => panic!("expected Err(AppError::Acp(\"line too long …\")), got: {other:?}"),
    }
}

// ── T078 (S058): Empty line is silently skipped ──────────────────────────────

/// An empty string (or whitespace-only string) passed to `parse_inbound_line`
/// returns `Ok(None)` — no event is emitted and no error is raised.
#[test]
fn empty_line_is_silently_skipped() {
    let result = parse_inbound_line("sess-001", "");
    assert!(
        matches!(result, Ok(None)),
        "empty string must be silently skipped, got: {result:?}"
    );

    // Whitespace-only lines must also be skipped.
    let result = parse_inbound_line("sess-001", "   ");
    assert!(
        matches!(result, Ok(None)),
        "whitespace-only line must be silently skipped, got: {result:?}"
    );
}
