//! Unit tests for ACP event handler wiring — Phase 2: Shared block builder extraction.
//!
//! Scenarios S042–S046 verify that `slack::blocks` exposes the shared Slack message
//! builders used by both MCP tool handlers and ACP event handlers.

use agent_intercom::models::approval::RiskLevel;
use agent_intercom::models::prompt::PromptType;
use agent_intercom::slack::blocks;

// ── S042: build_approval_blocks structure ────────────────────────────────────

/// S042a — `build_approval_blocks` returns at least one block (header) for minimal input.
#[test]
fn build_approval_blocks_minimal_returns_blocks() {
    let result = blocks::build_approval_blocks(
        "Add retry_count field",
        None,
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n+pub mod retry;",
        "src/lib.rs",
        RiskLevel::Low,
    );
    assert!(
        !result.is_empty(),
        "build_approval_blocks must return at least one block"
    );
}

/// S042b — `build_approval_blocks` includes a description block when description is Some.
#[test]
fn build_approval_blocks_with_description_has_more_blocks() {
    let without_desc =
        blocks::build_approval_blocks("Title", None, "diff line", "src/lib.rs", RiskLevel::Low);
    let with_desc = blocks::build_approval_blocks(
        "Title",
        Some("This change adds retry logic"),
        "diff line",
        "src/lib.rs",
        RiskLevel::Low,
    );
    assert!(
        with_desc.len() > without_desc.len(),
        "approval blocks with description must have more blocks than without"
    );
}

/// S042c — `build_approval_blocks` serializes to JSON containing the title text.
#[test]
fn build_approval_blocks_json_contains_title() {
    let result = blocks::build_approval_blocks(
        "Deploy config change",
        None,
        "+ new line",
        "config/server.toml",
        RiskLevel::High,
    );
    let json = serde_json::to_string(&result).expect("serialise blocks");
    assert!(
        json.contains("Deploy config change"),
        "serialised blocks must contain the title text"
    );
}

/// S042d — `build_approval_blocks` serializes to JSON containing the file path.
#[test]
fn build_approval_blocks_json_contains_file_path() {
    let result = blocks::build_approval_blocks(
        "Update schema",
        None,
        "+ col text",
        "persistence/schema.rs",
        RiskLevel::Critical,
    );
    let json = serde_json::to_string(&result).expect("serialise blocks");
    assert!(
        json.contains("persistence/schema.rs"),
        "serialised blocks must contain the file path"
    );
}

// ── S043: diff truncation at INLINE_DIFF_THRESHOLD ───────────────────────────

/// S043a — A diff with fewer lines than `INLINE_DIFF_THRESHOLD` is rendered inline.
#[test]
fn build_approval_blocks_short_diff_rendered_inline() {
    // 5 lines — well under threshold
    let short_diff = "line1\nline2\nline3\nline4\nline5";
    let result =
        blocks::build_approval_blocks("Title", None, short_diff, "src/lib.rs", RiskLevel::Low);
    let json = serde_json::to_string(&result).expect("serialise blocks");
    // Inline diff uses a code block fence (```), large diffs use file-count message
    assert!(
        json.contains("line1"),
        "short diff must be rendered inline: content must appear in blocks"
    );
}

/// S043b — A diff with more lines than `INLINE_DIFF_THRESHOLD` is replaced by a line-count indicator.
#[test]
fn build_approval_blocks_long_diff_shows_line_count() {
    // Build a diff with INLINE_DIFF_THRESHOLD + 5 lines
    let long_diff: String = (0..blocks::INLINE_DIFF_THRESHOLD + 5)
        .map(|i| format!("+ added line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = blocks::build_approval_blocks(
        "Big change",
        None,
        &long_diff,
        "src/large.rs",
        RiskLevel::Low,
    );
    let json = serde_json::to_string(&result).expect("serialise blocks");
    // Long diffs should NOT render the raw content inline
    assert!(
        !json.contains("added line 0"),
        "long diff content must not be inlined"
    );
    // Should instead show a line count indicator
    assert!(
        json.contains("lines") || json.contains("Diff"),
        "long diff must show a line-count or file indicator"
    );
}

/// S043c — `INLINE_DIFF_THRESHOLD` is exactly 20.
#[test]
fn inline_diff_threshold_is_twenty() {
    assert_eq!(
        blocks::INLINE_DIFF_THRESHOLD,
        20,
        "INLINE_DIFF_THRESHOLD must be 20"
    );
}

// ── S044: build_prompt_blocks structure ──────────────────────────────────────

/// S044a — `build_prompt_blocks` returns at least two blocks (header + prompt text).
#[test]
fn build_prompt_blocks_returns_multiple_blocks() {
    let result = blocks::build_prompt_blocks(
        "Should I continue with the refactoring?",
        PromptType::Continuation,
        None,
        None,
        "prompt-abc-123",
    );
    assert!(
        result.len() >= 2,
        "build_prompt_blocks must return at least 2 blocks (header + text)"
    );
}

/// S044b — `build_prompt_blocks` JSON contains the prompt text.
#[test]
fn build_prompt_blocks_json_contains_prompt_text() {
    let prompt = "Ready to deploy? Please confirm.";
    let result = blocks::build_prompt_blocks(
        prompt,
        PromptType::Clarification,
        Some(120),
        Some(5),
        "prompt-xyz",
    );
    let json = serde_json::to_string(&result).expect("serialise blocks");
    assert!(
        json.contains("Ready to deploy"),
        "blocks must contain prompt text"
    );
}

/// S044c — `build_prompt_blocks` with `elapsed_seconds` includes elapsed time in output.
#[test]
fn build_prompt_blocks_includes_elapsed_context() {
    let result = blocks::build_prompt_blocks(
        "Continue?",
        PromptType::Continuation,
        Some(300),
        None,
        "p-1",
    );
    let json = serde_json::to_string(&result).expect("serialise blocks");
    assert!(
        json.contains("300"),
        "blocks must include elapsed seconds context"
    );
}

/// S044d — `build_prompt_blocks` without elapsed or actions omits context block.
#[test]
fn build_prompt_blocks_no_context_when_none() {
    let with_context = blocks::build_prompt_blocks(
        "Continue?",
        PromptType::Continuation,
        Some(10),
        Some(3),
        "p-2",
    );
    let without_context =
        blocks::build_prompt_blocks("Continue?", PromptType::Continuation, None, None, "p-3");
    assert!(
        with_context.len() > without_context.len(),
        "blocks with context must have more blocks than those without"
    );
}

// ── S045: prompt_type helpers ─────────────────────────────────────────────────

/// S045a — `prompt_type_label` returns distinct labels for each prompt type.
#[test]
fn prompt_type_label_returns_distinct_labels() {
    let continuation = blocks::prompt_type_label(PromptType::Continuation);
    let clarification = blocks::prompt_type_label(PromptType::Clarification);
    let error_recovery = blocks::prompt_type_label(PromptType::ErrorRecovery);
    let resource_warning = blocks::prompt_type_label(PromptType::ResourceWarning);

    let labels = [
        continuation,
        clarification,
        error_recovery,
        resource_warning,
    ];
    let unique_count = labels
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(
        unique_count, 4,
        "all four prompt types must have distinct labels"
    );
}

/// S045b — `prompt_type_icon` returns non-empty strings for each prompt type.
#[test]
fn prompt_type_icon_returns_non_empty_for_all_types() {
    for pt in [
        PromptType::Continuation,
        PromptType::Clarification,
        PromptType::ErrorRecovery,
        PromptType::ResourceWarning,
    ] {
        let icon = blocks::prompt_type_icon(pt);
        assert!(
            !icon.is_empty(),
            "prompt_type_icon must be non-empty for {pt:?}"
        );
    }
}

// ── S046: truncate_text in blocks ─────────────────────────────────────────────

/// S046a — `blocks::truncate_text` returns full string when shorter than limit.
#[test]
fn blocks_truncate_text_short_passthrough() {
    let result = blocks::truncate_text("hello", 100);
    assert_eq!(result, "hello");
}

/// S046b — `blocks::truncate_text` appends ellipsis when over limit.
#[test]
fn blocks_truncate_text_long_adds_ellipsis() {
    let result = blocks::truncate_text("hello world", 8);
    assert!(
        result.ends_with("..."),
        "truncated text must end with ellipsis"
    );
    assert!(result.len() <= 8, "result must not exceed max_len");
}

/// S046c — `blocks::truncate_text` does not split multibyte characters.
#[test]
fn blocks_truncate_text_multibyte_safe() {
    // "café" — 'é' is 2 bytes. Truncating at 6 must not split it.
    let result = blocks::truncate_text("café world", 7);
    assert!(
        std::str::from_utf8(result.as_bytes()).is_ok(),
        "result must be valid UTF-8"
    );
    assert!(result.ends_with("..."), "must end with ellipsis");
}

// ── MCP/ACP output equivalence ────────────────────────────────────────────────

/// S046d — Calling `build_approval_blocks` directly (as ACP will) produces the same
/// result as what the MCP `ask_approval` handler produced previously.
///
/// This is a structural equivalence test — both callers use the same shared function,
/// so output must be identical for identical inputs.
#[test]
fn approval_blocks_output_is_deterministic() {
    let params = (
        "Same title",
        Some("Same description"),
        "+ same diff line",
        "src/same.rs",
        RiskLevel::High,
    );
    let call1 = blocks::build_approval_blocks(params.0, params.1, params.2, params.3, params.4);
    let call2 = blocks::build_approval_blocks(params.0, params.1, params.2, params.3, params.4);
    let json1 = serde_json::to_string(&call1).expect("serialise call1");
    let json2 = serde_json::to_string(&call2).expect("serialise call2");
    assert_eq!(json1, json2, "build_approval_blocks must be deterministic");
}

/// S046e — Calling `build_prompt_blocks` is deterministic across repeated calls.
#[test]
fn prompt_blocks_output_is_deterministic() {
    let call1 = blocks::build_prompt_blocks(
        "Continue with deployment?",
        PromptType::Continuation,
        Some(60),
        Some(10),
        "det-prompt-1",
    );
    let call2 = blocks::build_prompt_blocks(
        "Continue with deployment?",
        PromptType::Continuation,
        Some(60),
        Some(10),
        "det-prompt-1",
    );
    let json1 = serde_json::to_string(&call1).expect("serialise call1");
    let json2 = serde_json::to_string(&call2).expect("serialise call2");
    assert_eq!(json1, json2, "build_prompt_blocks must be deterministic");
}

// ── S018–S023: Risk level parse-or-default semantics ─────────────────────────

use agent_intercom::models::approval::parse_risk_level;

/// S018 — "low" (lowercase) parses to `RiskLevel::Low`.
#[test]
fn parse_risk_level_low() {
    assert_eq!(parse_risk_level("low"), RiskLevel::Low);
}

/// S019 — "high" (lowercase) parses to `RiskLevel::High`.
#[test]
fn parse_risk_level_high() {
    assert_eq!(parse_risk_level("high"), RiskLevel::High);
}

/// S020 — "critical" (lowercase) parses to `RiskLevel::Critical`.
#[test]
fn parse_risk_level_critical() {
    assert_eq!(parse_risk_level("critical"), RiskLevel::Critical);
}

/// S021 — Unknown string "extreme" defaults to `RiskLevel::Low`.
#[test]
fn parse_risk_level_unknown_defaults_to_low() {
    assert_eq!(parse_risk_level("extreme"), RiskLevel::Low);
}

/// S022 — Empty string defaults to `RiskLevel::Low`.
#[test]
fn parse_risk_level_empty_defaults_to_low() {
    assert_eq!(parse_risk_level(""), RiskLevel::Low);
}

/// S023 — Mixed-case "High" and "LOW" both default to `RiskLevel::Low`
/// (matching is case-sensitive per FR-011).
#[test]
fn parse_risk_level_mixed_case_defaults_to_low() {
    assert_eq!(
        parse_risk_level("High"),
        RiskLevel::Low,
        "matching is case-sensitive: 'High' must default to Low"
    );
    assert_eq!(
        parse_risk_level("LOW"),
        RiskLevel::Low,
        "matching is case-sensitive: 'LOW' must default to Low"
    );
}

// ── S030–S035: SHA-256 hash computation (path safety) ────────────────────────

use agent_intercom::mcp::tools::util::compute_file_hash;

/// S030 — Non-existent file path returns the `"new_file"` sentinel.
#[tokio::test]
async fn hash_nonexistent_returns_new_file_sentinel() {
    let path = std::path::Path::new("/this/path/does/not/exist/abc123.rs");
    let hash = compute_file_hash(path)
        .await
        .expect("hash computation must not error for missing file");
    assert_eq!(
        hash, "new_file",
        "missing file must return 'new_file' sentinel"
    );
}

/// S031 — Empty file returns the SHA-256 digest of zero bytes (not `"new_file"`).
#[tokio::test]
async fn hash_empty_file_returns_sha256_of_empty_bytes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("empty.txt");
    tokio::fs::write(&path, b"")
        .await
        .expect("write empty file");
    let hash = compute_file_hash(&path).await.expect("hash empty file");
    // SHA-256 of empty input is known: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb924\
         27ae41e4649b934ca495991b7852b855",
        "empty file must hash to SHA-256 of zero bytes"
    );
}

/// S032 — File with known content returns a reproducible hex digest.
#[tokio::test]
async fn hash_file_with_content_returns_hex_digest() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("content.txt");
    tokio::fs::write(&path, b"hello world")
        .await
        .expect("write content file");
    let hash = compute_file_hash(&path).await.expect("hash content file");
    // Not "new_file"
    assert_ne!(hash, "new_file", "existing file must not return 'new_file'");
    // Must be a 64-char hex string (SHA-256)
    assert_eq!(hash.len(), 64, "SHA-256 hex digest must be 64 characters");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be hex characters only"
    );
}

/// S033 — Path traversal ("../../etc/passwd") is rejected by `validate_workspace_path`.
#[test]
fn path_traversal_rejected_by_path_safety() {
    use agent_intercom::diff::validate_workspace_path;
    let dir = tempfile::tempdir().expect("temp dir");
    let result = validate_workspace_path(dir.path(), "../../etc/passwd");
    assert!(
        result.is_err(),
        "path traversal must be rejected by validate_workspace_path"
    );
}

/// S034 — Absolute path outside workspace is rejected by `validate_workspace_path`.
#[test]
fn absolute_path_outside_workspace_rejected() {
    use agent_intercom::diff::validate_workspace_path;
    let dir = tempfile::tempdir().expect("temp dir");
    let outside = if cfg!(windows) {
        "C:\\Windows\\System32\\bad.exe"
    } else {
        "/etc/shadow"
    };
    let result = validate_workspace_path(dir.path(), outside);
    assert!(
        result.is_err(),
        "absolute path outside workspace must be rejected"
    );
}

/// S035 — Valid relative path within workspace succeeds.
#[test]
fn valid_path_within_workspace_succeeds() {
    use agent_intercom::diff::validate_workspace_path;
    let dir = tempfile::tempdir().expect("temp dir");
    // Should not error — even if file doesn't exist, path is valid relative to workspace
    // (path_safety only rejects traversal, not missing files)
    let result = validate_workspace_path(dir.path(), "src/main.rs");
    assert!(
        result.is_ok(),
        "valid relative path within workspace must succeed: {result:?}"
    );
}

// ── S024–S029: prompt_type parse-or-default semantics ────────────────────────

use agent_intercom::models::prompt::parse_prompt_type;

/// S024 — `"continuation"` maps to `PromptType::Continuation`.
#[test]
fn parse_prompt_type_continuation() {
    assert_eq!(
        parse_prompt_type("continuation"),
        PromptType::Continuation,
        "S024: 'continuation' must map to PromptType::Continuation"
    );
}

/// S025 — `"clarification"` maps to `PromptType::Clarification`.
#[test]
fn parse_prompt_type_clarification() {
    assert_eq!(
        parse_prompt_type("clarification"),
        PromptType::Clarification,
        "S025: 'clarification' must map to PromptType::Clarification"
    );
}

/// S026 — `"error_recovery"` maps to `PromptType::ErrorRecovery`.
#[test]
fn parse_prompt_type_error_recovery() {
    assert_eq!(
        parse_prompt_type("error_recovery"),
        PromptType::ErrorRecovery,
        "S026: 'error_recovery' must map to PromptType::ErrorRecovery"
    );
}

/// S027 — `"resource_warning"` maps to `PromptType::ResourceWarning`.
#[test]
fn parse_prompt_type_resource_warning() {
    assert_eq!(
        parse_prompt_type("resource_warning"),
        PromptType::ResourceWarning,
        "S027: 'resource_warning' must map to PromptType::ResourceWarning"
    );
}

/// S028 — Unknown string `"custom_agent_query"` defaults to `PromptType::Continuation`.
#[test]
fn parse_prompt_type_unknown_defaults_to_continuation() {
    assert_eq!(
        parse_prompt_type("custom_agent_query"),
        PromptType::Continuation,
        "S028: unknown string must default to PromptType::Continuation"
    );
}

/// S029 — Empty string defaults to `PromptType::Continuation`.
#[test]
fn parse_prompt_type_empty_defaults_to_continuation() {
    assert_eq!(
        parse_prompt_type(""),
        PromptType::Continuation,
        "S029: empty string must default to PromptType::Continuation"
    );
}

// ── S056: PromptForwarded→ContinuationPrompt field mapping ───────────────────

use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision};

/// S056 — `ContinuationPrompt::new(...)` created from a `PromptForwarded` event
/// has the correct field values: `session_id`/`prompt_text` copied directly,
/// `elapsed_seconds=None`, `actions_taken=None`, `decision=None`, `slack_ts=None`.
#[test]
fn prompt_forwarded_field_mapping() {
    let session_id = "session:acp-02".to_owned();
    let prompt_text = "Should I continue with the refactoring?".to_owned();
    let prompt_type = parse_prompt_type("clarification");

    let prompt = ContinuationPrompt::new(
        session_id.clone(),
        prompt_text.clone(),
        prompt_type,
        None, // elapsed_seconds — ACP-specific: always None in event handler
        None, // actions_taken — ACP-specific: always None in event handler
    );

    assert_eq!(
        prompt.session_id, session_id,
        "S056: session_id must be copied"
    );
    assert_eq!(
        prompt.prompt_text, prompt_text,
        "S056: prompt_text must be copied"
    );
    assert_eq!(
        prompt.prompt_type,
        PromptType::Clarification,
        "S056: prompt_type must be parsed enum"
    );
    assert!(
        prompt.elapsed_seconds.is_none(),
        "S056: elapsed_seconds must be None (ACP-specific)"
    );
    assert!(
        prompt.actions_taken.is_none(),
        "S056: actions_taken must be None (ACP-specific)"
    );
    assert!(
        prompt.decision.is_none(),
        "S056: decision must default to None"
    );
    assert!(
        prompt.instruction.is_none(),
        "S056: instruction must default to None"
    );
    assert!(
        prompt.slack_ts.is_none(),
        "S056: slack_ts must default to None"
    );
    assert!(
        !prompt.id.is_empty(),
        "S056: id must be generated (non-empty)"
    );

    // PromptDecision variants are accessible (compile check)
    let _: PromptDecision = PromptDecision::Continue;
}

// ── S055: ClearanceRequested→ApprovalRequest field mapping ────────────────────

use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus};

/// S055 — `ApprovalRequest::new(...)` maps all `ClearanceRequested` fields correctly.
#[test]
fn approval_request_field_mapping() {
    let session_id = "session:acp-01".to_owned();
    let title = "Add retry logic".to_owned();
    let description = Some("Adds retry_count field".to_owned());
    let diff = "+pub retry_count: u32,".to_owned();
    let file_path = "src/models/approval.rs".to_owned();
    let risk_level = RiskLevel::High;
    let original_hash = "abc123deadbeef".to_owned();

    let req = ApprovalRequest::new(
        session_id.clone(),
        title.clone(),
        description.clone(),
        diff.clone(),
        file_path.clone(),
        risk_level,
        original_hash.clone(),
    );

    assert_eq!(req.session_id, session_id, "session_id must be copied");
    assert_eq!(req.title, title, "title must be copied");
    assert_eq!(req.description, description, "description must be copied");
    assert_eq!(req.diff_content, diff, "diff_content must be copied");
    assert_eq!(req.file_path, file_path, "file_path must be copied");
    assert_eq!(req.risk_level, risk_level, "risk_level must be copied");
    assert_eq!(
        req.original_hash, original_hash,
        "original_hash must be copied"
    );
    assert_eq!(
        req.status,
        ApprovalStatus::Pending,
        "status must default to Pending"
    );
    assert!(
        req.consumed_at.is_none(),
        "consumed_at must default to None"
    );
    assert!(req.slack_ts.is_none(), "slack_ts must default to None");
    // id must be a non-empty string
    assert!(!req.id.is_empty(), "id must be generated (non-empty)");
}
