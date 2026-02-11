//! `SurrealDB` schema definitions and bootstrap logic.
//!
//! All table definitions use `SCHEMAFULL` mode. Schema is applied
//! idempotently with `IF NOT EXISTS` on every server startup.

use crate::Result;

use super::db::Database;

/// Apply all table and field definitions to the connected database.
///
/// Uses `IF NOT EXISTS` so re-execution is safe across restarts.
///
/// # Errors
///
/// Returns `AppError::Db` if any schema statement fails.
pub async fn apply_schema(db: &Database) -> Result<()> {
    let ddl = r"
DEFINE TABLE IF NOT EXISTS session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS owner_user_id ON TABLE session TYPE string;
DEFINE FIELD IF NOT EXISTS workspace_root ON TABLE session TYPE string;
DEFINE FIELD IF NOT EXISTS status ON TABLE session TYPE string
    ASSERT $value IN ['created', 'active', 'paused', 'terminated', 'interrupted'];
DEFINE FIELD IF NOT EXISTS prompt ON TABLE session TYPE option<string>;
DEFINE FIELD IF NOT EXISTS mode ON TABLE session TYPE string
    ASSERT $value IN ['remote', 'local', 'hybrid'];
DEFINE FIELD IF NOT EXISTS created_at ON TABLE session TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE session TYPE datetime;
DEFINE FIELD IF NOT EXISTS terminated_at ON TABLE session TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS last_tool ON TABLE session TYPE option<string>;
DEFINE FIELD IF NOT EXISTS nudge_count ON TABLE session TYPE int;
DEFINE FIELD IF NOT EXISTS stall_paused ON TABLE session TYPE bool;
DEFINE FIELD IF NOT EXISTS progress_snapshot ON TABLE session TYPE option<array>;

DEFINE TABLE IF NOT EXISTS approval_request SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS session_id ON TABLE approval_request TYPE string;
DEFINE FIELD IF NOT EXISTS title ON TABLE approval_request TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE approval_request TYPE option<string>;
DEFINE FIELD IF NOT EXISTS diff_content ON TABLE approval_request TYPE string;
DEFINE FIELD IF NOT EXISTS file_path ON TABLE approval_request TYPE string;
DEFINE FIELD IF NOT EXISTS risk_level ON TABLE approval_request TYPE string
    ASSERT $value IN ['low', 'high', 'critical'];
DEFINE FIELD IF NOT EXISTS status ON TABLE approval_request TYPE string
    ASSERT $value IN ['pending', 'approved', 'rejected', 'expired', 'consumed', 'interrupted'];
DEFINE FIELD IF NOT EXISTS original_hash ON TABLE approval_request TYPE string;
DEFINE FIELD IF NOT EXISTS slack_ts ON TABLE approval_request TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE approval_request TYPE datetime;
DEFINE FIELD IF NOT EXISTS consumed_at ON TABLE approval_request TYPE option<datetime>;

DEFINE TABLE IF NOT EXISTS checkpoint SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS session_id ON TABLE checkpoint TYPE string;
DEFINE FIELD IF NOT EXISTS label ON TABLE checkpoint TYPE option<string>;
DEFINE FIELD IF NOT EXISTS session_state ON TABLE checkpoint TYPE object;
DEFINE FIELD IF NOT EXISTS file_hashes ON TABLE checkpoint TYPE object;
DEFINE FIELD IF NOT EXISTS workspace_root ON TABLE checkpoint TYPE string;
DEFINE FIELD IF NOT EXISTS progress_snapshot ON TABLE checkpoint TYPE option<array>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE checkpoint TYPE datetime;

DEFINE TABLE IF NOT EXISTS continuation_prompt SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS session_id ON TABLE continuation_prompt TYPE string;
DEFINE FIELD IF NOT EXISTS prompt_text ON TABLE continuation_prompt TYPE string;
DEFINE FIELD IF NOT EXISTS prompt_type ON TABLE continuation_prompt TYPE string
    ASSERT $value IN ['continuation', 'clarification', 'error_recovery', 'resource_warning'];
DEFINE FIELD IF NOT EXISTS elapsed_seconds ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD IF NOT EXISTS actions_taken ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD IF NOT EXISTS decision ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD IF NOT EXISTS instruction ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD IF NOT EXISTS slack_ts ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE continuation_prompt TYPE datetime;

DEFINE TABLE IF NOT EXISTS stall_alert SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS session_id ON TABLE stall_alert TYPE string;
DEFINE FIELD IF NOT EXISTS last_tool ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_activity_at ON TABLE stall_alert TYPE datetime;
DEFINE FIELD IF NOT EXISTS idle_seconds ON TABLE stall_alert TYPE int;
DEFINE FIELD IF NOT EXISTS nudge_count ON TABLE stall_alert TYPE int;
DEFINE FIELD IF NOT EXISTS status ON TABLE stall_alert TYPE string
    ASSERT $value IN ['pending', 'nudged', 'self_recovered', 'escalated', 'dismissed'];
DEFINE FIELD IF NOT EXISTS nudge_message ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD IF NOT EXISTS progress_snapshot ON TABLE stall_alert TYPE option<array>;
DEFINE FIELD IF NOT EXISTS slack_ts ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE stall_alert TYPE datetime;
";

    db.query(ddl).await?;
    Ok(())
}
