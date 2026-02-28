//! Auto-approve suggestion handler for manual approval suggestions (T066).
//!
//! When an operator manually approves a terminal command, this module can
//! offer a one-click button to persist a regex pattern for that command to
//! the workspace's `.intercom/settings.json` policy file, enabling future
//! auto-approval.

use std::path::Path;
use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackBlock, SlackHistoryMessage, SlackInteractionActionInfo,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::slack::blocks;

/// Build Slack block kit blocks for an auto-approve suggestion prompt.
///
/// Returns a vec with an explanatory text section and an action button
/// that the operator can click to persist the pattern.
#[must_use]
pub fn suggestion_blocks(command: &str) -> Vec<SlackBlock> {
    vec![
        blocks::text_section(&format!(
            "💡 *Auto-approve suggestion* — would you like to automatically approve \
             commands matching `{command}` in future sessions?"
        )),
        blocks::auto_approve_suggestion_button(command),
    ]
}

/// Generate a safe regex pattern that anchors to the significant prefix of a command.
///
/// For commands that dispatch through subcommands (`cargo`, `git`, `npm`, etc.) the
/// pattern captures `base subcommand`.  For simple OS commands (`DEL`, `rmdir`,
/// `Copy-Item`, …) only the base command name is captured.  Everything after the
/// anchor is wildcarded to permit any combination of flags and file arguments while
/// still blocking shell-metacharacter injection.
///
/// # Examples
///
/// ```
/// # use agent_intercom::slack::handlers::command_approve::generate_pattern;
/// assert!(generate_pattern("DEL /F /Q file.txt").starts_with("^DEL"));
/// assert!(generate_pattern("cargo test --release").starts_with("^cargo test"));
/// assert!(generate_pattern("git add src/main.rs").starts_with("^git add"));
/// assert!(generate_pattern("rmdir /S folder").starts_with("^rmdir"));
/// ```
#[must_use]
pub fn generate_pattern(command: &str) -> String {
    /// Commands that dispatch through a subcommand (the second token carries
    /// semantic meaning and should be included in the anchor).
    const MULTI_LEVEL_COMMANDS: &[&str] = &[
        "cargo",
        "git",
        "npm",
        "npx",
        "yarn",
        "pnpm",
        "dotnet",
        "docker",
        "kubectl",
        "az",
        "aws",
        "gcloud",
        "pwsh",
        "powershell",
        "pip",
        "python",
        "python3",
        "node",
        "deno",
        "rustup",
    ];

    let tokens: Vec<&str> = command.split_whitespace().collect();
    let anchor = match tokens.as_slice() {
        [] => command.to_owned(),
        [base] => (*base).to_owned(),
        [base, sub, ..] => {
            let base_lower = base.to_lowercase();
            if MULTI_LEVEL_COMMANDS.iter().any(|&c| c == base_lower) {
                // Include the subcommand, e.g. "cargo test", "git add".
                format!("{base} {sub}")
            } else {
                // Simple command — wildcard all flags/filenames.
                (*base).to_owned()
            }
        }
    };

    let escaped = regex::escape(&anchor);
    format!(r"^{escaped}(\s[^;|&`]*)?$")
}

/// Write `command`'s auto-approve pattern to `settings_path`.
///
/// Appends the generated regex to the `chat.tools.terminal.autoApprove` array
/// in `.intercom/settings.json`.  This is the same key used in the VS Code
/// `*.code-workspace` file, so both agent-intercom and Copilot Chat share a
/// single canonical list without maintaining separate sections.
///
/// If `settings_path` does not exist it is created with a minimal JSON
/// skeleton.  Duplicate patterns are silently ignored.
///
/// # Errors
///
/// Returns `crate::AppError` on I/O or JSON serialisation failures.
pub fn write_pattern_to_settings(settings_path: &Path, command: &str) -> crate::Result<()> {
    // Load existing settings or start with an empty object.
    let mut root: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path)
            .map_err(|e| crate::AppError::Config(format!("read settings: {e}")))?;
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let pattern = generate_pattern(command);

    // Write to `chat.tools.terminal.autoApprove` as a map — the same format
    // used by VS Code in *.code-workspace and .vscode/settings.json.
    let obj = root
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("settings root is not an object".into()))?;
    let commands_val = obj
        .entry("chat.tools.terminal.autoApprove")
        .or_insert_with(|| serde_json::json!({}));
    let map = commands_val.as_object_mut().ok_or_else(|| {
        crate::AppError::Config("chat.tools.terminal.autoApprove is not an object".into())
    })?;
    if !map.contains_key(&pattern) {
        map.insert(
            pattern,
            serde_json::json!({ "approve": true, "matchCommandLine": true }),
        );
    }

    // Write back atomically via a temp file in the same directory.
    let parent = settings_path.parent().unwrap_or(std::path::Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| crate::AppError::Config(format!("create temp file: {e}")))?;
    serde_json::to_writer_pretty(tmp.as_file(), &root)
        .map_err(|e| crate::AppError::Config(format!("serialise settings: {e}")))?;
    tmp.persist(settings_path)
        .map_err(|e| crate::AppError::Config(format!("persist settings: {e}")))?;

    Ok(())
}

/// Strip a JSONC line comment (`// …`) from `line`, preserving `//` that
/// appears inside a string literal (e.g. `"http://example.com"`).
///
/// Returns the portion of `line` before the first comment-starting `//` that
/// is not enclosed in double quotes.  The original string is returned unchanged
/// if no such `//` is found.
fn strip_jsonc_line_comment(line: &str) -> std::borrow::Cow<'_, str> {
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if !in_string && c == '/' && chars.get(i + 1) == Some(&'/') {
            // Truncate here — collect chars up to position `i`.
            return std::borrow::Cow::Owned(chars[..i].iter().collect());
        }
    }
    std::borrow::Cow::Borrowed(line)
}

/// Find the matching closing `}` for a `{` at `open_pos`, correctly handling
/// nested braces and JSON string escaping.  Returns `None` if the structure
/// is malformed or unbalanced.
fn find_matching_brace(text: &str, open_pos: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in text[open_pos..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if !in_string {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(open_pos + i);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Attempt to insert a new auto-approve pattern entry into a JSONC file
/// without destroying user-maintained comments.
///
/// Locates the `"chat.tools.terminal.autoApprove"` key in the raw text,
/// finds the boundaries of its `{ … }` value object via brace-matching,
/// and inserts the new entry before the closing `}`.  Returns `None` if
/// the key is not found or the structure is unexpected, signalling the
/// caller to fall back to a full (comment-stripping) rewrite.
fn try_insert_pattern_preserving(raw: &str, pattern: &str) -> Option<String> {
    let key_needle = "\"chat.tools.terminal.autoApprove\"";
    let key_idx = raw.find(key_needle)?;

    // Find the ':' after the key.
    let after_key = key_idx + key_needle.len();
    let colon_offset = raw[after_key..].find(':')?;
    let after_colon = after_key + colon_offset + 1;

    // Find opening '{' of the value object.
    let brace_offset = raw[after_colon..].find('{')? + after_colon;

    // Find matching '}'.
    let close_pos = find_matching_brace(raw, brace_offset)?;

    // Derive indentation from the line containing the closing brace.
    let line_start = raw[..close_pos].rfind('\n').map_or(0, |p| p + 1);
    let base_indent: String = raw[line_start..close_pos]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let entry_indent = format!("{base_indent}    ");

    // Build the JSON entry text.  The pattern may contain regex back-slash
    // escapes (`\s`, `\.`, …), which must be double-escaped inside a JSON
    // string.  Serialise via serde_json to guarantee correct escaping.
    let key_json = serde_json::to_string(pattern).unwrap_or_else(|_| format!("\"{pattern}\""));
    let entry = format!(
        "{entry_indent}{key_json}: {{\n\
         {entry_indent}    \"approve\": true,\n\
         {entry_indent}    \"matchCommandLine\": true\n\
         {entry_indent}}}"
    );

    // Check whether there are existing entries between { and }.
    let inner = raw[brace_offset + 1..close_pos].trim();
    let has_entries = !inner.is_empty();

    let mut result = String::with_capacity(raw.len() + entry.len() + 16);

    if has_entries {
        // Find the last non-whitespace character before the closing brace to
        // decide whether a trailing comma is needed.
        let content_end = raw[..close_pos]
            .rfind(|c: char| !c.is_whitespace())
            .map_or(close_pos, |p| p + 1);
        result.push_str(&raw[..content_end]);
        if !raw[..content_end].ends_with(',') {
            result.push(',');
        }
        result.push('\n');
        result.push_str(&entry);
        result.push('\n');
        result.push_str(&raw[close_pos..]);
    } else {
        result.push_str(&raw[..=brace_offset]);
        result.push('\n');
        result.push_str(&entry);
        result.push('\n');
        result.push_str(&raw[close_pos..]);
    }

    Some(result)
}

/// Find the first `*.code-workspace` file in `workspace_root` and write the
/// generated pattern to its `settings.chat.tools.terminal.autoApprove` map.
///
/// This allows VS Code GitHub Copilot Chat to pick up the same auto-approve
/// rule without the operator manually editing the workspace file.
///
/// Returns `Ok(true)` if a workspace file was found and updated, `Ok(false)` if
/// none was found (not an error — workspace files are optional), or an error if
/// a workspace file was found but could not be parsed or written.
///
/// # Errors
///
/// Returns `crate::AppError` on I/O or JSON parse/serialisation failures.
pub fn write_pattern_to_workspace_file(
    workspace_root: &Path,
    command: &str,
) -> crate::Result<bool> {
    // Find the first *.code-workspace file in the workspace root (not recursive).
    let workspace_file = std::fs::read_dir(workspace_root)
        .map_err(|e| crate::AppError::Config(format!("read workspace root: {e}")))?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("code-workspace"));

    let Some(ws_path) = workspace_file else {
        return Ok(false);
    };

    let raw = std::fs::read_to_string(&ws_path)
        .map_err(|e| crate::AppError::Config(format!("read workspace file: {e}")))?;
    // Strip JSONC line comments before parsing. Scan char-by-char so that `//`
    // inside string literals (e.g. URLs like "http://...") is preserved correctly.
    let stripped = raw
        .lines()
        .map(|line| strip_jsonc_line_comment(line).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    let mut root: serde_json::Value = serde_json::from_str(&stripped)
        .map_err(|e| crate::AppError::Config(format!("parse workspace file: {e}")))?;

    let pattern = generate_pattern(command);

    // Navigate to (or create) settings.chat.tools.terminal.autoApprove.
    let obj = root
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("workspace root is not an object".into()))?;
    let settings = obj
        .entry("settings")
        .or_insert_with(|| serde_json::json!({}));
    let settings_obj = settings
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("settings is not an object".into()))?;
    let auto_approve = settings_obj
        .entry("chat.tools.terminal.autoApprove")
        .or_insert_with(|| serde_json::json!({}));
    let map = auto_approve
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("autoApprove is not an object".into()))?;

    // Early return if pattern already present — skips the full file rewrite
    // and preserves any user-maintained JSONC comments in the workspace file.
    if map.contains_key(&pattern) {
        return Ok(true);
    }

    // ── Comment-preserving insertion (RI-05) ───────────────
    // Try to insert the new entry into the raw (comment-preserved) file text
    // via targeted brace-matching rather than re-serialising the entire JSON.
    // Falls back to a full rewrite only if the autoApprove section is absent.
    if let Some(modified) = try_insert_pattern_preserving(&raw, &pattern) {
        let parent = ws_path.parent().unwrap_or(std::path::Path::new("."));
        let tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| crate::AppError::Config(format!("create temp file: {e}")))?;
        std::io::Write::write_all(&mut tmp.as_file(), modified.as_bytes())
            .map_err(|e| crate::AppError::Config(format!("write workspace file: {e}")))?;
        tmp.persist(&ws_path)
            .map_err(|e| crate::AppError::Config(format!("persist workspace file: {e}")))?;
        return Ok(true);
    }

    // Fallback: the autoApprove section does not exist yet — create it via
    // full JSON rewrite (this strips JSONC comments on first auto-approve).
    warn!("autoApprove section not found in workspace file; falling back to full rewrite");

    map.insert(
        pattern,
        serde_json::json!({ "approve": true, "matchCommandLine": true }),
    );

    // Write back atomically.
    let parent = ws_path.parent().unwrap_or(std::path::Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| crate::AppError::Config(format!("create temp file: {e}")))?;
    serde_json::to_writer_pretty(tmp.as_file(), &root)
        .map_err(|e| crate::AppError::Config(format!("serialise workspace file: {e}")))?;
    tmp.persist(&ws_path)
        .map_err(|e| crate::AppError::Config(format!("persist workspace file: {e}")))?;

    Ok(true)
}

/// Write `command`'s auto-approve pattern to `.vscode/settings.json` if it exists.
///
/// `.vscode/settings.json` is read by VS Code and GitHub Copilot CLI when
/// running locally without the MCP server.  The pattern is inserted into the
/// top-level `chat.tools.terminal.autoApprove` map (same key, same regex as
/// the other files, but as a map entry rather than an array element).
///
/// Returns `Ok(true)` if the file was found and updated, `Ok(false)` if the
/// file does not exist (not an error), or an error if the file exists but
/// cannot be parsed or written.
///
/// # Errors
///
/// Returns `crate::AppError` on I/O or JSON parse/serialisation failures.
pub fn write_pattern_to_vscode_settings(
    workspace_root: &Path,
    command: &str,
) -> crate::Result<bool> {
    let vscode_path = workspace_root.join(".vscode").join("settings.json");
    if !vscode_path.exists() {
        return Ok(false);
    }

    let raw = std::fs::read_to_string(&vscode_path)
        .map_err(|e| crate::AppError::Config(format!("read .vscode/settings.json: {e}")))?;
    let stripped = raw
        .lines()
        .map(|line| strip_jsonc_line_comment(line).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    let mut root: serde_json::Value = serde_json::from_str(&stripped)
        .map_err(|e| crate::AppError::Config(format!("parse .vscode/settings.json: {e}")))?;

    let pattern = generate_pattern(command);

    // The autoApprove map is at the top level in .vscode/settings.json
    // (unlike *.code-workspace where it is nested under "settings").
    let obj = root.as_object_mut().ok_or_else(|| {
        crate::AppError::Config(".vscode/settings.json root is not an object".into())
    })?;
    let auto_approve = obj
        .entry("chat.tools.terminal.autoApprove")
        .or_insert_with(|| serde_json::json!({}));
    let map = auto_approve.as_object_mut().ok_or_else(|| {
        crate::AppError::Config("chat.tools.terminal.autoApprove is not an object".into())
    })?;

    // Early return if pattern already present — skips the full file rewrite
    // and preserves any user-maintained JSONC comments in the settings file.
    if map.contains_key(&pattern) {
        return Ok(true);
    }

    // ── Comment-preserving insertion (RI-05) ───────────────
    if let Some(modified) = try_insert_pattern_preserving(&raw, &pattern) {
        let parent = vscode_path.parent().unwrap_or(std::path::Path::new("."));
        let tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| crate::AppError::Config(format!("create temp file: {e}")))?;
        std::io::Write::write_all(&mut tmp.as_file(), modified.as_bytes())
            .map_err(|e| crate::AppError::Config(format!("write .vscode/settings.json: {e}")))?;
        tmp.persist(&vscode_path)
            .map_err(|e| crate::AppError::Config(format!("persist .vscode/settings.json: {e}")))?;
        return Ok(true);
    }

    // Fallback: the autoApprove section does not exist yet.
    warn!("autoApprove section not found in .vscode/settings.json; falling back to full rewrite");

    map.insert(
        pattern,
        serde_json::json!({ "approve": true, "matchCommandLine": true }),
    );

    let parent = vscode_path.parent().unwrap_or(std::path::Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| crate::AppError::Config(format!("create temp file: {e}")))?;
    serde_json::to_writer_pretty(tmp.as_file(), &root)
        .map_err(|e| crate::AppError::Config(format!("serialise .vscode/settings.json: {e}")))?;
    tmp.persist(&vscode_path)
        .map_err(|e| crate::AppError::Config(format!("persist .vscode/settings.json: {e}")))?;

    Ok(true)
}

/// Handle an `auto_approve_add` or `auto_approve_dismiss` button click.
///
/// * `auto_approve_add` — writes the pattern to `.intercom/settings.json`
///   inside the configured workspace root and replaces the suggestion message
///   with a confirmation.
/// * `auto_approve_dismiss` — replaces the message with a dismissed status
///   without modifying the policy file.
///
/// # Errors
///
/// Returns a descriptive error string if the settings write or message
/// update fails.
pub async fn handle_auto_approve_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let action_id = action.action_id.to_string();
    let command = action
        .value
        .as_deref()
        .ok_or_else(|| "auto_approve action missing command value".to_owned())?;

    let status_text = if action_id == "auto_approve_add" {
        // Build path to .intercom/settings.json inside the workspace root.
        let settings_path = state
            .config
            .default_workspace_root()
            .join(".intercom")
            .join("settings.json");

        // Write .intercom/settings.json via spawn_blocking — sync I/O must not
        // block the Tokio async executor (consistent with keyring credential lookups).
        let settings_path_owned = settings_path.clone();
        let command_for_settings = command.to_owned();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = settings_path_owned.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create .intercom dir: {e}"))?;
            }
            write_pattern_to_settings(&settings_path_owned, &command_for_settings)
                .map_err(|e| format!("failed to write auto-approve pattern: {e}"))
        })
        .await
        .map_err(|e| format!("spawn_blocking join error (settings write): {e}"))??;

        // Best-effort: also write to the VS Code *.code-workspace file so the
        // rule is active for local Copilot Chat sessions without manual editing.
        let workspace_root_buf = state.config.default_workspace_root().to_path_buf();
        let command_for_ws = command.to_owned();
        match tokio::task::spawn_blocking(move || {
            write_pattern_to_workspace_file(&workspace_root_buf, &command_for_ws)
        })
        .await
        {
            Err(e) => {
                warn!(%e, user_id, command, "spawn_blocking join error for workspace file (non-fatal)");
            }
            Ok(Ok(true)) => info!(
                user_id,
                command, "auto-approve pattern added to workspace file"
            ),
            Ok(Ok(false)) => info!(
                user_id,
                command, "no .code-workspace file found; skipping VS Code update"
            ),
            Ok(Err(err)) => {
                warn!(%err, user_id, command, "failed to update .code-workspace file (non-fatal)");
            }
        }

        // Best-effort: also propagate to .vscode/settings.json for Copilot CLI.
        let workspace_root_buf2 = state.config.default_workspace_root().to_path_buf();
        let command_for_vscode = command.to_owned();
        match tokio::task::spawn_blocking(move || {
            write_pattern_to_vscode_settings(&workspace_root_buf2, &command_for_vscode)
        })
        .await
        {
            Err(e) => {
                warn!(%e, user_id, command, "spawn_blocking join error for .vscode/settings.json (non-fatal)");
            }
            Ok(Ok(true)) => info!(
                user_id,
                command, "auto-approve pattern added to .vscode/settings.json"
            ),
            Ok(Ok(false)) => info!(user_id, command, "no .vscode/settings.json found; skipping"),
            Ok(Err(err)) => {
                warn!(%err, user_id, command, "failed to update .vscode/settings.json (non-fatal)");
            }
        }

        info!(
            user_id,
            command, "auto-approve pattern added to .intercom/settings.json"
        );
        format!("\u{2705} *Added to auto-approve policy* by <@{user_id}> \u{2014} `{command}`")
    } else if action_id == "auto_approve_dismiss" {
        info!(user_id, command, "auto-approve suggestion dismissed");
        format!("\u{1f6ab} *Auto-approve suggestion dismissed* by <@{user_id}>")
    } else {
        return Err(format!("unknown auto_approve action_id: {action_id}"));
    };

    // Replace the suggestion buttons with a static status line (FR-022).
    if let Some(ref slack) = state.slack {
        let msg_ts = message.map(|m| m.origin.ts.clone());
        let chan_id = channel.map(|c| c.id.clone());

        if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
            let replacement = vec![blocks::text_section(&status_text)];
            if let Err(err) = slack.update_message(ch, ts, replacement).await {
                warn!(%err, user_id, action_id, "failed to replace auto-approve suggestion buttons");
            }
        }
    }

    Ok(())
}
