//! `SurrealDB` embedded database connection and schema bootstrap.

use std::fs;

use surrealdb::engine::local::{Db, Mem, RocksDb};
use surrealdb::Surreal;

use crate::{AppError, GlobalConfig, Result};

use super::schema;

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
    schema::apply_schema(&db).await?;
    Ok(db)
}
