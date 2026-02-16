//! `SurrealDB` schema definitions and bootstrap logic.
//!
//! All table definitions use `SCHEMAFULL` mode. Schema is applied
//! on every server startup; `DEFINE` statements in `SurrealDB` are
//! safe to re-run — re-defining an existing entity overwrites it with
//! the same definition, so the result is convergent.

use crate::Result;

use super::db::Database;

/// Apply all table and field definitions to the connected database.
///
/// Re-execution is safe: `SurrealDB` `DEFINE` statements overwrite
/// existing definitions with the same shape, producing a convergent result.
///
/// # Errors
///
/// Returns `AppError::Db` if any schema statement fails.
pub async fn apply_schema(db: &Database) -> Result<()> {
    let ddl = r"
DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD owner_user_id ON TABLE session TYPE string;
DEFINE FIELD workspace_root ON TABLE session TYPE string;
DEFINE FIELD status ON TABLE session TYPE string
    ASSERT $value IN ['created', 'active', 'paused', 'terminated', 'interrupted'];
DEFINE FIELD prompt ON TABLE session TYPE option<string>;
DEFINE FIELD mode ON TABLE session TYPE string
    ASSERT $value IN ['remote', 'local', 'hybrid'];
DEFINE FIELD created_at ON TABLE session;
DEFINE FIELD updated_at ON TABLE session;
DEFINE FIELD terminated_at ON TABLE session;
DEFINE FIELD last_tool ON TABLE session TYPE option<string>;
DEFINE FIELD nudge_count ON TABLE session TYPE int;
DEFINE FIELD stall_paused ON TABLE session TYPE bool;
DEFINE FIELD progress_snapshot ON TABLE session TYPE option<array>;
DEFINE FIELD progress_snapshot.* ON TABLE session TYPE object;
DEFINE FIELD progress_snapshot.*.label ON TABLE session TYPE string;
DEFINE FIELD progress_snapshot.*.status ON TABLE session TYPE string;

DEFINE TABLE approval_request SCHEMAFULL;
DEFINE FIELD session_id ON TABLE approval_request TYPE string;
DEFINE FIELD title ON TABLE approval_request TYPE string;
DEFINE FIELD description ON TABLE approval_request TYPE option<string>;
DEFINE FIELD diff_content ON TABLE approval_request TYPE string;
DEFINE FIELD file_path ON TABLE approval_request TYPE string;
DEFINE FIELD risk_level ON TABLE approval_request TYPE string
    ASSERT $value IN ['low', 'high', 'critical'];
DEFINE FIELD status ON TABLE approval_request TYPE string
    ASSERT $value IN ['pending', 'approved', 'rejected', 'expired', 'consumed', 'interrupted'];
DEFINE FIELD original_hash ON TABLE approval_request TYPE string;
DEFINE FIELD slack_ts ON TABLE approval_request TYPE option<string>;
DEFINE FIELD created_at ON TABLE approval_request;
DEFINE FIELD consumed_at ON TABLE approval_request;

DEFINE TABLE checkpoint SCHEMAFULL;
DEFINE FIELD session_id ON TABLE checkpoint TYPE string;
DEFINE FIELD label ON TABLE checkpoint TYPE option<string>;
DEFINE FIELD session_state ON TABLE checkpoint FLEXIBLE TYPE object;
DEFINE FIELD file_hashes ON TABLE checkpoint FLEXIBLE TYPE object;
DEFINE FIELD workspace_root ON TABLE checkpoint TYPE string;
DEFINE FIELD progress_snapshot ON TABLE checkpoint TYPE option<array>;
DEFINE FIELD progress_snapshot.* ON TABLE checkpoint TYPE object;
DEFINE FIELD progress_snapshot.*.label ON TABLE checkpoint TYPE string;
DEFINE FIELD progress_snapshot.*.status ON TABLE checkpoint TYPE string;
DEFINE FIELD created_at ON TABLE checkpoint;

DEFINE TABLE continuation_prompt SCHEMAFULL;
DEFINE FIELD session_id ON TABLE continuation_prompt TYPE string;
DEFINE FIELD prompt_text ON TABLE continuation_prompt TYPE string;
DEFINE FIELD prompt_type ON TABLE continuation_prompt TYPE string
    ASSERT $value IN ['continuation', 'clarification', 'error_recovery', 'resource_warning'];
DEFINE FIELD elapsed_seconds ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD actions_taken ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD decision ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD instruction ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD slack_ts ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD created_at ON TABLE continuation_prompt;

DEFINE TABLE stall_alert SCHEMAFULL;
DEFINE FIELD session_id ON TABLE stall_alert TYPE string;
DEFINE FIELD last_tool ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD last_activity_at ON TABLE stall_alert;
DEFINE FIELD idle_seconds ON TABLE stall_alert TYPE int;
DEFINE FIELD nudge_count ON TABLE stall_alert TYPE int;
DEFINE FIELD status ON TABLE stall_alert TYPE string
    ASSERT $value IN ['pending', 'nudged', 'self_recovered', 'escalated', 'dismissed'];
DEFINE FIELD nudge_message ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD progress_snapshot ON TABLE stall_alert TYPE option<array>;
DEFINE FIELD progress_snapshot.* ON TABLE stall_alert TYPE object;
DEFINE FIELD progress_snapshot.*.label ON TABLE stall_alert TYPE string;
DEFINE FIELD progress_snapshot.*.status ON TABLE stall_alert TYPE string;
DEFINE FIELD slack_ts ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD created_at ON TABLE stall_alert;
";

    db.query(ddl).await?;
    Ok(())
}
