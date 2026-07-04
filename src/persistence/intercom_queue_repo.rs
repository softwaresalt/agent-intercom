//! File-backed persistence for the `.intercom` numbered queue.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::models::intercom_queue::QueueItem;
use crate::{AppError, Result};

/// Per-queue-file locks that serialize read-modify-write operations on the same
/// `.intercom` queue file within the process (PR #18 review P1). This guards
/// against concurrent `/arc queue` handlers racing on the load-modify-save
/// sequence and losing updates. Scope is in-process; a single server owns a
/// workspace's `.intercom` queue, so cross-process locking is not required.
static QUEUE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

/// Return the shared lock guarding writes to `path`, creating it on first use.
fn queue_lock(path: &Path) -> Arc<Mutex<()>> {
    let registry = QUEUE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().unwrap_or_else(PoisonError::into_inner);
    map.entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueStore {
    items: Vec<QueueItem>,
    next_number: u32,
}

impl Default for QueueStore {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next_number: 1,
        }
    }
}

/// Repository for reading and mutating the `.intercom` numbered queue.
#[derive(Debug, Clone)]
pub struct IntercomQueueRepo {
    queue_file: PathBuf,
}

impl IntercomQueueRepo {
    /// Create a repo rooted at the provided `.intercom` directory.
    #[must_use]
    pub fn new(intercom_dir: &Path) -> Self {
        Self {
            queue_file: intercom_dir.join("queue.json"),
        }
    }

    /// The workspace root that contains this `.intercom` directory.
    ///
    /// Returns the grandparent of the queue file (`<root>/.intercom/queue.json`) —
    /// the directory where the backlogit workspace (`.backlogit`) is expected to
    /// live. Returns `None` when the path has no such ancestor.
    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.queue_file.parent().and_then(Path::parent)
    }

    /// Add a new queue item.
    ///
    /// # Errors
    ///
    /// Returns an error when queue state cannot be loaded or saved.
    pub fn add(&self, text: &str) -> Result<QueueItem> {
        let lock = queue_lock(&self.queue_file);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        let mut store = self.load()?;
        let item = QueueItem {
            number: store.next_number,
            text: text.to_owned(),
            created_at: Utc::now(),
        };
        store.items.push(item.clone());
        store.next_number = store
            .next_number
            .checked_add(1)
            .ok_or_else(|| AppError::Config("queue number overflow".to_owned()))?;
        self.save(&store)?;
        Ok(item)
    }

    /// List all queue items in insertion order.
    ///
    /// # Errors
    ///
    /// Returns an error when queue state cannot be loaded.
    pub fn list(&self) -> Result<Vec<QueueItem>> {
        let lock = queue_lock(&self.queue_file);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        Ok(self.load()?.items)
    }

    /// Replace the text of an existing queue item.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` when the item does not exist, or an error
    /// when queue state cannot be loaded or saved.
    pub fn replace(&self, n: u32, text: &str) -> Result<QueueItem> {
        let lock = queue_lock(&self.queue_file);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        let mut store = self.load()?;
        let item = store
            .items
            .iter_mut()
            .find(|item| item.number == n)
            .ok_or_else(|| AppError::NotFound(format!("queue item {n} not found")))?;
        text.clone_into(&mut item.text);
        let updated = item.clone();
        self.save(&store)?;
        Ok(updated)
    }

    /// Remove an existing queue item.
    ///
    /// # Errors
    ///
    /// Returns `AppError::NotFound` when the item does not exist, or an error
    /// when queue state cannot be loaded or saved.
    pub fn remove(&self, n: u32) -> Result<QueueItem> {
        let lock = queue_lock(&self.queue_file);
        let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        let mut store = self.load()?;
        let index = store
            .items
            .iter()
            .position(|item| item.number == n)
            .ok_or_else(|| AppError::NotFound(format!("queue item {n} not found")))?;
        let removed = store.items.remove(index);
        self.save(&store)?;
        Ok(removed)
    }

    fn load(&self) -> Result<QueueStore> {
        if !self.queue_file.exists() {
            return Ok(QueueStore::default());
        }

        let raw = fs::read_to_string(&self.queue_file).map_err(|err| {
            AppError::Io(format!(
                "failed to read queue file {}: {err}",
                self.queue_file.display()
            ))
        })?;
        let mut store: QueueStore = serde_json::from_str(&raw).map_err(|err| {
            AppError::Io(format!(
                "failed to parse queue file {}: {err}",
                self.queue_file.display()
            ))
        })?;
        normalize_store(&mut store);
        Ok(store)
    }

    fn save(&self, store: &QueueStore) -> Result<()> {
        if let Some(parent) = self.queue_file.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AppError::Io(format!(
                    "failed to create queue directory {}: {err}",
                    parent.display()
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(store).map_err(|err| {
            AppError::Io(format!(
                "failed to serialize queue file {}: {err}",
                self.queue_file.display()
            ))
        })?;
        fs::write(&self.queue_file, content).map_err(|err| {
            AppError::Io(format!(
                "failed to write queue file {}: {err}",
                self.queue_file.display()
            ))
        })
    }
}

fn normalize_store(store: &mut QueueStore) {
    let expected_next = store
        .items
        .iter()
        .map(|item| item.number)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    if store.next_number < expected_next {
        store.next_number = expected_next;
    }

    if store.next_number == 0 {
        store.next_number = 1;
    }
}
