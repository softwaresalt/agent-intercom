//! Unit tests for `SessionStatus` model helpers.

use agent_intercom::models::session::SessionStatus;

/// `as_str` returns the correct `snake_case` database value for each variant.
#[test]
fn test_session_status_as_str_values() {
    assert_eq!(SessionStatus::Created.as_str(), "created");
    assert_eq!(SessionStatus::Active.as_str(), "active");
    assert_eq!(SessionStatus::Paused.as_str(), "paused");
    assert_eq!(SessionStatus::Terminated.as_str(), "terminated");
    assert_eq!(SessionStatus::Interrupted.as_str(), "interrupted");
}
