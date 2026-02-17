//! `SQLite` database connection and schema bootstrap.

use std::fs;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::{AppError, Result};

use super::schema;

/// Alias for the shared `SQLite` connection pool.
pub type Database = SqlitePool;

/// Connect to a file-backed `SQLite` database and apply schema.
///
/// Creates parent directories if they do not exist. Enables WAL journal
/// mode and restricts the pool to a single writer connection (FR-014).
///
/// # Errors
///
/// Returns `AppError::Db` if the connection or schema application fails.
pub async fn connect(path: &str) -> Result<Database> {
    let db_path = std::path::Path::new(path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| AppError::Db(format!("failed to create db dir: {err}")))?;
    }

    let opts = SqliteConnectOptions::from_str(path)
        .map_err(|err| AppError::Db(format!("invalid db path: {err}")))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    schema::bootstrap_schema(&pool).await?;
    Ok(pool)
}

/// Connect to an in-memory `SQLite` database and apply schema.
///
/// Uses `min_connections(1)` to keep the database alive for the
/// lifetime of the pool (FR-004, US3).
///
/// # Errors
///
/// Returns `AppError::Db` if the connection or schema application fails.
pub async fn connect_memory() -> Result<Database> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|err| AppError::Db(format!("invalid memory uri: {err}")))?;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .connect_with(opts)
        .await?;

    schema::bootstrap_schema(&pool).await?;
    Ok(pool)
}
