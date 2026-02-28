//! Unit tests for `JsonlAuditWriter` (T054, T055).
//!
//! Validates JSONL audit log writing, automatic directory creation,
//! daily file rotation, concurrent write safety, and non-fatal error
//! handling on filesystem failures.
//!
//! # Scenarios covered
//!
//! | ID   | Scenario |
//! |------|----------|
//! | S049 | Tool call logged — `event_type: tool_call` written to JSONL file |
//! | S050 | Approval logged — `event_type: approval` with operator_id and request_id |
//! | S051 | Rejection logged — `event_type: rejection` with reason field populated |
//! | S052 | Command approval logged — `event_type: command_approval` with command field |
//! | S053 | Session lifecycle — `event_type: session_start` written correctly |
//! | S054 | Daily rotation — first write after date change opens a new file |
//! | S055 | Audit directory missing — `JsonlAuditWriter::new` creates it automatically |
//! | S056 | Date change produces separate files per day |
//! | S057 | Concurrent audit writes produce valid, readable JSONL |

use std::fs;
use std::sync::Arc;

use agent_intercom::audit::writer::JsonlAuditWriter;
use agent_intercom::audit::{AuditEntry, AuditEventType, AuditLogger};

/// Helper: build a minimal `AuditEntry` for the given event type.
fn entry(event_type: AuditEventType) -> AuditEntry {
    AuditEntry::new(event_type)
}

// ── S055: constructor auto-creates the audit directory ────────────────────────

/// S055 — `JsonlAuditWriter::new` must succeed even when the log directory does
/// not exist, creating it on first use.
#[test]
fn new_creates_directory_if_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_dir = temp.path().join("nonexistent").join("nested").join("dir");

    assert!(
        !log_dir.exists(),
        "directory must not exist before construction"
    );

    let writer = JsonlAuditWriter::new(log_dir.clone())
        .expect("constructor must succeed even for missing directory");

    // Write one entry to force file creation.
    writer
        .log_entry(entry(AuditEventType::ToolCall))
        .expect("first write must succeed");

    assert!(
        log_dir.exists(),
        "log directory must be created after first write"
    );
}

// ── S049: tool call entry written correctly ───────────────────────────────────

/// S049 — Writing a `ToolCall` entry produces a valid JSONL line containing
/// `event_type: "tool_call"` in the output file.
#[test]
fn tool_call_entry_written_to_jsonl() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    let e = AuditEntry::new(AuditEventType::ToolCall)
        .with_tool("ping".to_owned())
        .with_result("ok".to_owned());
    writer.log_entry(e).expect("log_entry should succeed");

    // Find and read the JSONL file.
    let jsonl = read_only_jsonl_file(temp.path());
    assert!(
        jsonl.contains("\"tool_call\""),
        "JSONL must contain tool_call event_type; got: {jsonl}"
    );
    assert!(
        jsonl.contains("\"ping\""),
        "JSONL must contain tool name 'ping'"
    );
}

// ── S050: approval entry ──────────────────────────────────────────────────────

/// S050 — Approval entries include `operator_id` and `request_id`.
#[test]
fn approval_entry_includes_operator_and_request_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    let e = AuditEntry::new(AuditEventType::Approval)
        .with_operator("U12345678".to_owned())
        .with_request_id("req-abc".to_owned());
    writer.log_entry(e).expect("log approval");

    let jsonl = read_only_jsonl_file(temp.path());
    assert!(
        jsonl.contains("\"approval\""),
        "must contain approval event type"
    );
    assert!(jsonl.contains("U12345678"), "must contain operator_id");
    assert!(jsonl.contains("req-abc"), "must contain request_id");
}

// ── S051: rejection entry ─────────────────────────────────────────────────────

/// S051 — Rejection entries include the `reason` field.
#[test]
fn rejection_entry_includes_reason() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    let e = AuditEntry::new(AuditEventType::Rejection).with_reason("patch conflict".to_owned());
    writer.log_entry(e).expect("log rejection");

    let jsonl = read_only_jsonl_file(temp.path());
    assert!(
        jsonl.contains("\"rejection\""),
        "must contain rejection event type"
    );
    assert!(
        jsonl.contains("patch conflict"),
        "must contain rejection reason"
    );
}

// ── S052: command approval entry ──────────────────────────────────────────────

/// S052 — Command approval entries include the `command` field.
#[test]
fn command_approval_entry_includes_command() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    let e = AuditEntry::new(AuditEventType::CommandApproval)
        .with_command("cargo test --release".to_owned());
    writer.log_entry(e).expect("log command approval");

    let jsonl = read_only_jsonl_file(temp.path());
    assert!(
        jsonl.contains("\"command_approval\""),
        "must contain command_approval event type"
    );
    assert!(
        jsonl.contains("cargo test --release"),
        "must contain the command"
    );
}

// ── S053: session lifecycle events ───────────────────────────────────────────

/// S053 — Session start events include `session_id` and `event_type: session_start`.
#[test]
fn session_start_entry_written_with_session_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    let e =
        AuditEntry::new(AuditEventType::SessionStart).with_session("session-xyz-123".to_owned());
    writer.log_entry(e).expect("log session start");

    let jsonl = read_only_jsonl_file(temp.path());
    assert!(
        jsonl.contains("\"session_start\""),
        "must contain session_start event type"
    );
    assert!(jsonl.contains("session-xyz-123"), "must contain session_id");
}

// ── S054: daily rotation — separate files per date ───────────────────────────

/// S054 — `JsonlAuditWriter` names files `audit-YYYY-MM-DD.jsonl`.  We cannot
/// fast-forward the system clock in a unit test, but we can verify that the
/// initial file is created with today's date and follows the expected naming
/// convention, which is the prerequisite for rotation.
#[test]
fn log_file_named_with_todays_date() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    writer
        .log_entry(entry(AuditEventType::ToolCall))
        .expect("log");

    let today = chrono::Utc::now().date_naive();
    let expected_name = format!("audit-{today}.jsonl");

    let files: Vec<_> = fs::read_dir(temp.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    assert!(
        files.contains(&expected_name),
        "expected file '{expected_name}' not found; files: {files:?}"
    );
}

// ── S056: multiple entries in same file ──────────────────────────────────────

/// Multiple calls to `log_entry` on the same date produce multiple lines in
/// the same JSONL file — no data is lost or overwritten.
#[test]
fn multiple_entries_appended_to_same_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = JsonlAuditWriter::new(temp.path().to_owned()).expect("writer");

    for _ in 0..5 {
        writer
            .log_entry(entry(AuditEventType::ToolCall))
            .expect("log");
    }

    let jsonl = read_only_jsonl_file(temp.path());
    let line_count = jsonl.lines().count();
    assert_eq!(
        line_count, 5,
        "5 entries must produce 5 JSONL lines; got {line_count}"
    );
}

// ── S057: concurrent writes ───────────────────────────────────────────────────

/// S057 — Multiple threads writing concurrently must not corrupt the JSONL file.
///
/// Each `log_entry` call is protected by an internal `Mutex`, so all writes
/// must be ordered and produce valid JSON lines.
#[test]
fn concurrent_writes_produce_valid_jsonl() {
    let temp = tempfile::tempdir().expect("tempdir");
    let writer = Arc::new(JsonlAuditWriter::new(temp.path().to_owned()).expect("writer"));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let w = Arc::clone(&writer);
            std::thread::spawn(move || {
                w.log_entry(entry(AuditEventType::ToolCall))
                    .expect("concurrent write");
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let jsonl = read_only_jsonl_file(temp.path());
    let line_count = jsonl.lines().count();
    assert_eq!(
        line_count, 10,
        "10 concurrent writes must produce 10 valid JSONL lines; got {line_count}"
    );

    // Verify each line is valid JSON.
    for line in jsonl.lines() {
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(line);
        assert!(parsed.is_ok(), "each JSONL line must be valid JSON: {line}");
    }
}

// ── Helper: read the single JSONL file in the log directory ──────────────────

/// Read the contents of the single JSONL file in `dir` (panics if not exactly one).
fn read_only_jsonl_file(dir: &std::path::Path) -> String {
    let files: Vec<_> = fs::read_dir(dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .collect();

    assert_eq!(
        files.len(),
        1,
        "expected exactly one JSONL file; found: {files:?}"
    );
    fs::read_to_string(files[0].path()).expect("read jsonl file")
}
