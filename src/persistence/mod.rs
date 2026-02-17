//! Persistence layer modules.

pub mod approval_repo;
pub mod checkpoint_repo;
pub mod db;
pub mod prompt_repo;
pub mod retention;
pub mod schema;
pub mod session_repo;
pub mod stall_repo;

/// Re-export the database pool type for convenience.
pub use sqlx::SqlitePool;
