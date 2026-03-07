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
