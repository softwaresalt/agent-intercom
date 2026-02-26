//! JSONL audit log writer with daily file rotation.

use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{NaiveDate, Utc};
use tracing::warn;

use super::{AuditEntry, AuditLogger};
use crate::Result;

/// Internal state protected by a mutex.
struct WriterState {
    current_date: NaiveDate,
    writer: BufWriter<fs::File>,
}

/// A daily-rotating JSONL audit log writer.
///
/// Appends one JSON object per line to `<log_dir>/audit-YYYY-MM-DD.jsonl`.
/// Automatically opens a new file when the calendar date changes between writes.
pub struct JsonlAuditWriter {
    log_dir: PathBuf,
    state: Mutex<Option<WriterState>>,
}

impl JsonlAuditWriter {
    /// Construct a writer that stores logs in `log_dir`.
    ///
    /// Creates `log_dir` and all parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AppError::Config`] if the directory cannot be created.
    pub fn new(log_dir: PathBuf) -> crate::Result<Self> {
        fs::create_dir_all(&log_dir).map_err(|e| {
            crate::AppError::Config(format!(
                "failed to create audit log directory {}: {e}",
                log_dir.display()
            ))
        })?;
        Ok(Self {
            log_dir,
            state: Mutex::new(None),
        })
    }

    fn open_for_date(log_dir: &Path, date: NaiveDate) -> crate::Result<BufWriter<fs::File>> {
        let file_name = format!("audit-{date}.jsonl");
        let path = log_dir.join(file_name);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                crate::AppError::Config(format!("failed to open audit log {}: {e}", path.display()))
            })?;
        Ok(BufWriter::new(file))
    }
}

impl AuditLogger for JsonlAuditWriter {
    fn log_entry(&self, entry: AuditEntry) -> Result<()> {
        let today = Utc::now().date_naive();

        let mut guard = self
            .state
            .lock()
            .map_err(|_| crate::AppError::Config("audit writer mutex poisoned".to_string()))?;

        let needs_rotation = guard.as_ref().is_none_or(|s| s.current_date != today);

        if needs_rotation {
            let new_writer = Self::open_for_date(&self.log_dir, today)?;
            *guard = Some(WriterState {
                current_date: today,
                writer: new_writer,
            });
        }

        if let Some(state) = guard.as_mut() {
            let line = serde_json::to_string(&entry).map_err(|e| {
                crate::AppError::Config(format!("failed to serialize audit entry: {e}"))
            })?;
            if let Err(e) = writeln!(state.writer, "{line}") {
                warn!("failed to write audit log entry: {e}");
                return Err(crate::AppError::Config(format!("audit write failed: {e}")));
            }
            if let Err(e) = state.writer.flush() {
                warn!("failed to flush audit log: {e}");
                return Err(crate::AppError::Config(format!("audit flush failed: {e}")));
            }
        }

        Ok(())
    }
}
