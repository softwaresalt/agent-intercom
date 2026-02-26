//! `check_auto_approve` MCP tool handler (T064, T066, T052).
//!
//! Queries the workspace auto-approve policy to determine whether an
//! operation can bypass the remote approval gate. Returns immediately
//! without blocking the agent.
//!
//! Policy resolution order (T052):
//! 1. Try the shared [`PolicyCache`] in `AppState` (populated by the
//!    [`PolicyWatcher`] on hot-reload events — O(1) read, no disk I/O).
//! 2. On cache miss, load and compile the policy from disk via
//!    [`PolicyLoader`], then back-fill the cache for the next call.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use tracing::{info, info_span, Instrument};

use crate::mcp::handler::IntercomServer;
use crate::persistence::session_repo::SessionRepo;
use crate::policy::evaluator::{AutoApproveContext, PolicyEvaluator};
use crate::policy::loader::PolicyLoader;

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct CheckAutoApproveInput {
    /// Name of the tool or command to check.
    tool_name: String,
    /// Optional additional metadata for fine-grained evaluation.
    context: Option<AutoApproveContext>,
}

/// Handle the `check_auto_approve` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
pub async fn handle(
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: CheckAutoApproveInput = serde_json::from_value(serde_json::Value::Object(args))
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid check_auto_approve parameters: {err}"),
                None,
            )
        })?;

    let span = info_span!(
        "check_auto_approve",
        tool_name = %input.tool_name,
        has_context = input.context.is_some(),
    );

    async move {
        // ── Resolve active session for workspace root ────────
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let sessions = session_repo.list_active().await.map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
        })?;
        let session = sessions
            .into_iter()
            .next()
            .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?;

        let workspace_root = std::path::PathBuf::from(&session.workspace_root);

        // ── Resolve workspace policy (cache-first, T052) ────
        // 1. Try the shared PolicyCache for an O(1) hit.
        let policy = {
            let cache_guard = state.policy_cache.read().await;
            cache_guard.get(&workspace_root).cloned()
        };

        let policy = if let Some(cached) = policy {
            info!("policy cache hit — using pre-compiled policy");
            cached
        } else {
            // 2. Cache miss: load from disk and back-fill the cache.
            let loaded = PolicyLoader::load(&workspace_root).map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to load workspace policy: {err}"),
                    None,
                )
            })?;
            let mut cache_guard = state.policy_cache.write().await;
            cache_guard.insert(workspace_root.clone(), loaded.clone());
            loaded
        };

        // ── Evaluate policy ──────────────────────────────────
        let result = PolicyEvaluator::check(&input.tool_name, &input.context, &policy);

        info!(
            auto_approved = result.auto_approved,
            matched_rule = ?result.matched_rule,
            "policy evaluation complete"
        );

        // ── Build response per contract ──────────────────────
        let response = if result.auto_approved {
            serde_json::json!({
                "auto_approved": true,
                "matched_rule": result.matched_rule,
            })
        } else {
            serde_json::json!({
                "auto_approved": false,
                "matched_rule": null,
            })
        };

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response,
        )?]))
    }
    .instrument(span)
    .await
}
