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
pub mod wait;

use crate::models::session::Session;
use crate::{AppError, Result};

/// Verify that the acting Slack user is the owner of a session.
///
/// Implements FR-031: all session-modifying actions MUST verify that the
/// acting user matches `session.owner_user_id`. When `owner_user_id` is
/// empty (e.g., an MCP session created without a designated operator), the
/// check is skipped.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] when the acting user is not the owner.
pub fn check_session_ownership(session: &Session, acting_user_id: &str) -> Result<()> {
    // Empty owner means the session was created without a designated operator
    // (common for MCP sessions). Skip the check to stay backward-compatible.
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
