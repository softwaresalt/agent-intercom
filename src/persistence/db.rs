//! `SurrealDB` embedded database connection and schema bootstrap.

use std::fs;

use surrealdb::engine::local::{Db, Mem, RocksDb};
use surrealdb::Surreal;

use crate::{AppError, GlobalConfig, Result};

/// Alias for the shared `SurrealDB` client.
pub type Database = Surreal<Db>;

/// Connect to `SurrealDB` using `RocksDB` (production) or in-memory (tests) backends and apply schema.
///
/// # Errors
///
/// Returns `AppError::Db` if the connection or schema application fails.
pub async fn connect(config: &GlobalConfig, use_memory: bool) -> Result<Database> {
    let db = if use_memory {
        Surreal::new::<Mem>(()).await?
    } else {
        let db_path = config.db_path();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| AppError::Db(format!("failed to create db dir: {err}")))?;
        }
        Surreal::new::<RocksDb>(db_path).await?
    };

    db.use_ns("monocoque").use_db("agent_rem").await?;
    apply_schema(&db).await?;
    Ok(db)
}

async fn apply_schema(db: &Database) -> Result<()> {
    let schema = r"
DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD owner_user_id ON TABLE session TYPE string;
DEFINE FIELD status ON TABLE session TYPE string;
DEFINE FIELD prompt ON TABLE session TYPE option<string>;
DEFINE FIELD mode ON TABLE session TYPE string;
DEFINE FIELD created_at ON TABLE session TYPE datetime;
DEFINE FIELD updated_at ON TABLE session TYPE datetime;
DEFINE FIELD last_tool ON TABLE session TYPE option<string>;
DEFINE FIELD nudge_count ON TABLE session TYPE int;
DEFINE FIELD stall_paused ON TABLE session TYPE bool;

DEFINE TABLE approval_request SCHEMAFULL;
DEFINE FIELD session_id ON TABLE approval_request TYPE string;
DEFINE FIELD title ON TABLE approval_request TYPE string;
DEFINE FIELD description ON TABLE approval_request TYPE option<string>;
DEFINE FIELD diff_content ON TABLE approval_request TYPE string;
DEFINE FIELD file_path ON TABLE approval_request TYPE string;
DEFINE FIELD risk_level ON TABLE approval_request TYPE string;
DEFINE FIELD status ON TABLE approval_request TYPE string;
DEFINE FIELD original_hash ON TABLE approval_request TYPE string;
DEFINE FIELD slack_ts ON TABLE approval_request TYPE option<string>;
DEFINE FIELD created_at ON TABLE approval_request TYPE datetime;
DEFINE FIELD consumed_at ON TABLE approval_request TYPE option<datetime>;

DEFINE TABLE checkpoint SCHEMAFULL;
DEFINE FIELD session_id ON TABLE checkpoint TYPE string;
DEFINE FIELD label ON TABLE checkpoint TYPE option<string>;
DEFINE FIELD session_state ON TABLE checkpoint TYPE object;
DEFINE FIELD file_hashes ON TABLE checkpoint TYPE object;
DEFINE FIELD created_at ON TABLE checkpoint TYPE datetime;

DEFINE TABLE continuation_prompt SCHEMAFULL;
DEFINE FIELD session_id ON TABLE continuation_prompt TYPE string;
DEFINE FIELD prompt_text ON TABLE continuation_prompt TYPE string;
DEFINE FIELD prompt_type ON TABLE continuation_prompt TYPE string;
DEFINE FIELD elapsed_seconds ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD actions_taken ON TABLE continuation_prompt TYPE option<int>;
DEFINE FIELD decision ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD instruction ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD slack_ts ON TABLE continuation_prompt TYPE option<string>;
DEFINE FIELD created_at ON TABLE continuation_prompt TYPE datetime;

DEFINE TABLE stall_alert SCHEMAFULL;
DEFINE FIELD session_id ON TABLE stall_alert TYPE string;
DEFINE FIELD last_tool ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD last_activity_at ON TABLE stall_alert TYPE datetime;
DEFINE FIELD idle_seconds ON TABLE stall_alert TYPE int;
DEFINE FIELD nudge_count ON TABLE stall_alert TYPE int;
DEFINE FIELD status ON TABLE stall_alert TYPE string;
DEFINE FIELD nudge_message ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD slack_ts ON TABLE stall_alert TYPE option<string>;
DEFINE FIELD created_at ON TABLE stall_alert TYPE datetime;
";

    db.query(schema).await?;
    Ok(())
}
