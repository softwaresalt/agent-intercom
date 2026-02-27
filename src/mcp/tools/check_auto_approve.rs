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
use std::time::Duration;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::SlackChannelId;
use tokio::sync::oneshot;
use tracing::{info, info_span, warn, Instrument};
use uuid::Uuid;

use crate::audit::{AuditEntry, AuditEventType};
use crate::mcp::handler::{ApprovalResponse, IntercomServer};
use crate::persistence::session_repo::SessionRepo;
use crate::policy::evaluator::{AutoApproveContext, PolicyEvaluator};
use crate::policy::loader::PolicyLoader;
use crate::slack::{blocks, client::SlackMessage};

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct CheckAutoApproveInput {
    /// Name of the tool or command to check.
    tool_name: String,
    /// Optional additional metadata for fine-grained evaluation.
    context: Option<AutoApproveContext>,
    /// Whether this invocation targets a `"terminal_command"` or `"file_operation"`.
    ///
    /// When `"terminal_command"` and the command is not already covered by the
    /// workspace auto-approve policy, the tool switches to blocking mode: it
    /// posts an approval prompt to Slack and waits for the operator to respond
    /// before returning. On approval it also offers a one-click auto-approve
    /// suggestion so future identical commands bypass the gate automatically.
    ///
    /// Any value other than `"terminal_command"` (including absent) falls back
    /// to the standard non-blocking policy check.
    kind: Option<String>,
}

/// Handle the `check_auto_approve` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
#[allow(clippy::too_many_lines)] // Terminal command gate + policy path are inherently sequential.
pub async fn handle(
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().map(str::to_owned);
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: CheckAutoApproveInput = serde_json::from_value(serde_json::Value::Object(args))
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid check_auto_approve parameters: {err}"),
                None,
            )
        })?;

    let span_has_kind = input.kind.is_some();
    let span = info_span!(
        "check_auto_approve",
        tool_name = %input.tool_name,
        has_context = input.context.is_some(),
        has_kind = span_has_kind,
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

        // ── Terminal command gate ───────────────────────────────
        // When the agent specifies kind = "terminal_command" and the command
        // is not already auto-approved by policy, block and request operator
        // approval via Slack before returning. This is the only path in
        // check_auto_approve that blocks; all other invocations return
        // immediately with the policy result.
        if input.kind.as_deref() == Some("terminal_command") && !result.auto_approved {
            match (&state.slack, &channel_id) {
                (Some(slack), Some(ch)) => {
                    let request_id = Uuid::new_v4().to_string();
                    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
                    {
                        let mut pending = state.pending_approvals.lock().await;
                        pending.insert(request_id.clone(), tx);
                    }
                    {
                        let mut cmd_pending =
                            state.pending_command_approvals.lock().await;
                        cmd_pending.insert(request_id.clone(), input.tool_name.clone());
                    }

                    let approval_blocks =
                        blocks::command_approval_blocks(&input.tool_name, &request_id);
                    let slack_msg = SlackMessage {
                        channel: SlackChannelId(ch.clone()),
                        text: Some(format!(
                            "\u{1f510} Approve terminal command: `{}`",
                            input.tool_name
                        )),
                        blocks: Some(approval_blocks),
                        thread_ts: None,
                    };

                    if let Err(err) = slack.enqueue(slack_msg).await {
                        warn!(
                            %err,
                            tool_name = %input.tool_name,
                            "failed to post command approval to Slack"
                        );
                        state.pending_approvals.lock().await.remove(&request_id);
                        state
                            .pending_command_approvals
                            .lock()
                            .await
                            .remove(&request_id);
                    } else {
                        let timeout =
                            Duration::from_secs(state.config.timeouts.approval_seconds);
                        let approved = if let Ok(Ok(resp)) =
                            tokio::time::timeout(timeout, rx).await
                        {
                            resp.status == "approved"
                        } else {
                            // Timeout or channel closed — clean up and deny.
                            state
                                .pending_approvals
                                .lock()
                                .await
                                .remove(&request_id);
                            state
                                .pending_command_approvals
                                .lock()
                                .await
                                .remove(&request_id);
                            false
                        };
                        let terminal_response = serde_json::json!({
                            "auto_approved": approved,
                            "matched_rule": if approved {
                                serde_json::json!("operator:approved")
                            } else {
                                serde_json::Value::Null
                            },
                        });
                        return Ok(CallToolResult::success(vec![
                            rmcp::model::Content::json(terminal_response)?,
                        ]));
                    }
                }
                _ => {
                    info!(
                        tool_name = %input.tool_name,
                        "terminal_command gate: Slack unavailable, denying without prompt"
                    );
                }
            }
            // Slack unavailable or enqueue failed — return deny.
            return Ok(CallToolResult::success(vec![rmcp::model::Content::json(
                serde_json::json!({ "auto_approved": false, "matched_rule": null }),
            )?]));
        }

        // ── Audit-log the policy decision (FR-026) ───────────
        if let Some(ref logger) = state.audit_logger {
            let event_type = if result.auto_approved {
                AuditEventType::CommandApproval
            } else {
                AuditEventType::CommandRejection
            };
            let entry = AuditEntry::new(event_type)
                .with_session(session.id.clone())
                .with_command(input.tool_name.clone());
            if let Err(err) = logger.log_entry(entry) {
                warn!(%err, "audit log write failed (auto_check)");
            }
        }

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
