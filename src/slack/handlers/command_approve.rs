//! Auto-approve suggestion handler for manual approval suggestions (T066).
//!
//! When an operator manually approves a terminal command, this module can
//! offer a one-click button to persist a regex pattern for that command to
//! the workspace's `.agentrc/settings.json` policy file, enabling future
//! auto-approval.

use std::path::Path;

use slack_morphism::prelude::SlackBlock;

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
/// If `settings_path` does not exist it is created with a minimal JSON
/// skeleton.  If it already exists the pattern is merged into the
/// `"chat.tools.terminal.autoApprove"` map without removing prior entries.
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

    // Ensure the auto-approve map exists.
    let auto_approve = root
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("settings root is not an object".into()))?
        .entry("chat.tools.terminal.autoApprove")
        .or_insert_with(|| serde_json::json!({}));

    let map = auto_approve
        .as_object_mut()
        .ok_or_else(|| crate::AppError::Config("autoApprove is not an object".into()))?;

    // Insert the generated pattern.
    let pattern = generate_pattern(command);
    map.insert(
        pattern,
        serde_json::json!({ "approve": true, "matchCommandLine": true }),
    );

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
