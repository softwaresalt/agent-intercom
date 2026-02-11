//! Domain model module declarations.

use serde::{Deserialize, Deserializer};

pub mod approval;
pub mod checkpoint;
pub mod policy;
pub mod progress;
pub mod prompt;
pub mod session;
pub mod stall;

/// Deserialize a `SurrealDB` `Thing` record ID as a plain string key.
///
/// `SurrealDB` 1.x returns record IDs as `Thing { tb, id }` in its internal
/// serialization format. This function deserializes the `Thing` and extracts
/// the plain key string, stripping both the table prefix and any
/// angle-bracket wrapping from complex IDs (e.g., UUIDs with hyphens).
pub(crate) fn deserialize_surreal_id<'de, D>(
    deserializer: D,
) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let thing = surrealdb::sql::Thing::deserialize(deserializer)?;
    match thing.id {
        surrealdb::sql::Id::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}
