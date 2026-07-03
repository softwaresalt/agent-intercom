//! Model for .intercom operator numbered-queue items.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A numbered item in the operator's `.intercom` personal queue.
///
/// Items are assigned a stable 1-based number at creation and never
/// renumbered when other items are removed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueItem {
    /// Stable 1-based item number. Never reused after removal.
    pub number: u32,
    /// Item text as entered by the operator.
    pub text: String,
    /// UTC creation timestamp.
    pub created_at: DateTime<Utc>,
}
