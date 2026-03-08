//! Slack interaction handler sub-modules.
//!
//! Also exposes shared helpers for session ownership verification (FR-031 /
//! T068c) that are used by all interactive action handlers.

pub mod approval;
pub mod command_approve;
pub mod modal;
pub mod nudge;
pub mod prompt;
pub mod steer;
pub mod task;
pub mod thread_reply;
pub mod wait;

use crate::models::session::Session;
use crate::{AppError, Result};

/// Verify that the acting Slack user is the owner of a session.
///
/// Implements FR-031: all session-modifying actions MUST verify that the
/// acting user matches `session.owner_user_id`. When `owner_user_id` is
/// empty (e.g., an MCP session created without a designated operator), the
/// check is skipped intentionally: MCP sessions initiated via stdio or the
/// HTTP transport do not have a Slack user context at creation time, so any
/// authorized Slack user may interact with them. If your deployment requires
/// strict ownership for all sessions, ensure `owner_user_id` is set at
/// session creation time.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] when the acting user is not the owner.
pub fn check_session_ownership(session: &Session, acting_user_id: &str) -> Result<()> {
    // Empty owner means the session was created without a designated operator
    // (common for MCP sessions). Skip the check intentionally to allow any
    // authorized Slack user to interact with operator-less sessions.
    if session.owner_user_id.is_empty() {
        return Ok(());
    }

    if session.owner_user_id == acting_user_id {
        return Ok(());
    }

    Err(AppError::Unauthorized(format!(
        "this session belongs to <@{}>; only the session owner can perform this action",
        session.owner_user_id
    )))
}
