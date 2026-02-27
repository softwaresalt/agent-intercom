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

/// Generate a safe regex pattern that anchors to the given command.
///
/// The pattern matches the command (with optional trailing whitespace and
/// arguments) but never matches unrelated commands.  Special regex
/// characters in the command are escaped before embedding.
#[must_use]
pub fn generate_pattern(command: &str) -> String {
    let escaped = regex::escape(command);
    // Anchor the pattern: allow optional trailing args (no shell metacharacters).
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

    // Write to `chat.tools.terminal.autoApprove` — the unified key shared by
    // both .intercom/settings.json (MCP evaluator) and *.code-workspace (VS Code).
    let obj = root
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("settings root is not an object".into()))?;
    let commands_val = obj
        .entry("chat.tools.terminal.autoApprove")
        .or_insert_with(|| serde_json::json!([]));
    let arr = commands_val
        .as_array_mut()
        .ok_or_else(|| crate::AppError::Config("chat.tools.terminal.autoApprove is not an array".into()))?;
    // Avoid duplicates.
    let pattern_val = serde_json::Value::String(pattern);
    if !arr.contains(&pattern_val) {
        arr.push(pattern_val);
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

    // Insert only if not already present.
    if !map.contains_key(&pattern) {
        map.insert(
            pattern,
            serde_json::json!({ "approve": true, "matchCommandLine": true }),
        );
    }

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

        // Ensure the parent directory exists.
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create .intercom dir: {e}"))?;
        }

        write_pattern_to_settings(&settings_path, command)
            .map_err(|e| format!("failed to write auto-approve pattern: {e}"))?;

        // Best-effort: also write to the VS Code *.code-workspace file so the
        // rule is active for local Copilot Chat sessions without manual editing.
        let workspace_root = state.config.default_workspace_root();
        match write_pattern_to_workspace_file(workspace_root, command) {
            Ok(true) => info!(user_id, command, "auto-approve pattern added to workspace file"),
            Ok(false) => info!(user_id, command, "no .code-workspace file found; skipping VS Code update"),
            Err(err) => warn!(%err, user_id, command, "failed to update .code-workspace file (non-fatal)"),
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
