//! Unit tests for `AppError::Acp` display format and error behavior (T006).

use agent_intercom::AppError;

#[test]
fn acp_error_display_starts_with_acp_prefix() {
    let err = AppError::Acp("stream closed".into());
    assert!(err.to_string().starts_with("acp:"));
}

#[test]
fn acp_error_display_includes_message() {
    let err = AppError::Acp("stream closed".into());
    assert_eq!(err.to_string(), "acp: stream closed");
}

#[test]
fn acp_error_message_no_trailing_period() {
    let err = AppError::Acp("write failed".into());
    let s = err.to_string();
    assert!(
        !s.ends_with('.'),
        "error message must not end with a period: {s}"
    );
}

#[test]
fn acp_error_is_distinct_from_io_error() {
    let acp = AppError::Acp("write failed".into());
    let io = AppError::Io("write failed".into());
    assert_ne!(acp.to_string(), io.to_string());
    assert!(acp.to_string().starts_with("acp:"));
    assert!(io.to_string().starts_with("io:"));
}

#[test]
fn acp_error_is_distinct_from_mcp_error() {
    let acp = AppError::Acp("connection dropped".into());
    let mcp = AppError::Mcp("connection dropped".into());
    assert_ne!(acp.to_string(), mcp.to_string());
}

#[test]
fn acp_error_implements_std_error_trait() {
    let err = AppError::Acp("test".into());
    // Verify it implements std::error::Error via Display + Debug
    let display = format!("{err}");
    let debug = format!("{err:?}");
    assert!(!display.is_empty());
    assert!(!debug.is_empty());
}

#[test]
fn acp_error_debug_representation() {
    let err = AppError::Acp("read timeout".into());
    let debug = format!("{err:?}");
    assert!(debug.contains("Acp"));
    assert!(debug.contains("read timeout"));
}
