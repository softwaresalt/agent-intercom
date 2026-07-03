# ADR-0017: Stdio-Child Reconnection and Resume-State Contract

**Status**: Accepted
**Date**: 2026-07-03
**Phase**: F.3 (Controller-mode reliability hardening), Task 013.003.004-T

## Context

The child-process controller spawns each agent session as a stdio-attached
host CLI process (`orchestrator::spawner::spawn_session`). When a child agent
crashes mid-task, the child monitor detects the abnormal exit and respawns it
(ADR-adjacent work: `013.003.001-T`). Respawning creates a **new** session
record linked to the crashed one via `Session::restart_of`, because the OS
process — and therefore its process identity — cannot be revived in place.

A new session id breaks the association between the resumed agent and the
durable state that was queued against the crashed session id:

* unconsumed **steering messages** (`steering_message.session_id`)
* pending **clearances** (`approval_request.session_id`, `status = 'pending'`)
* undecided **prompts** (`continuation_prompt.session_id`, `decision IS NULL`)

Without a contract for what is re-bound on respawn, this state is orphaned and
the resumed agent starts blank, losing mid-task context.

## Decision

On respawn, `orchestrator::spawner::respawn_session` re-binds the crashed
session's durable pending state to the resumed session before the replacement
process is activated. The contract is:

1. **Session identity.** The resumed session is a new record with a new `id`.
   It carries forward `owner_user_id`, `workspace_root`, `prompt`, `mode`,
   `protocol_mode`, `channel_id`, `thread_ts`, `title`, and the ACP
   `agent_session_id`, and sets `restart_of = crashed.id`. The
   `agent_session_id` is the logical ACP session that must be re-bound so the
   agent continues the same conversation.

2. **Steering queue (F.3-T2).** Every *unconsumed* steering message for the
   crashed session is reassigned to the resumed session via
   `SteeringRepo::reassign_unconsumed_to_session`. The original owning session
   is recorded in the additive `origin_session_id` column for traceability.

3. **Pending clearances (F.3-T3).** Every *pending* approval request is
   reassigned via `ApprovalRepo::reassign_pending_to_session`. The request
   `id` — the ACP clearance correlation id — is preserved so a later operator
   decision still matches the original request.

4. **Undecided prompts (F.3-T3).** Every prompt with no decision is reassigned
   via `PromptRepo::reassign_pending_to_session`. The prompt `id` — the ACP
   prompt correlation id — is preserved for the same reason.

5. **Best-effort semantics.** Re-binding is best-effort: a failure to move one
   state class is logged (`warn`) but does not abort the respawn. A live
   resumed session with partial state is preferable to no session at all.

6. **Already-terminal state is untouched.** Consumed steering messages, decided
   clearances, and decided prompts remain attached to the crashed session as an
   immutable audit record.

`respawn_session` consumes the persistence primitives added by F.3-T2 and
F.3-T3; it does not re-implement them. Correlation-id *uniqueness* across
restarts is provided upstream by F.2-T3 (fresh UUIDs per
`generate_correlation_id` call); this contract governs correlation-id
*preservation* when a request is carried to a resumed session.

## Consequences

### Positive

* Mid-task pending state (steering, clearances, prompts) survives a crash and
  is available to the resumed agent under its new session id.
* Correlation ids are preserved, so in-flight operator decisions remain valid
  against the resumed session.
* The `restart_of` link plus `origin_session_id` give a durable audit trail of
  the crash → resume transition.

### Negative

* The in-memory `oneshot` response channels (`pending_approvals`,
  `pending_prompts`, `pending_waits`) are not reconstructed by this contract;
  re-posting recovered requests to Slack after a full server restart remains
  the responsibility of the reconnect re-post path (ADR-0011) and the
  `recover_state` tool. This contract governs the DB-durable re-binding only.

### Risks

* If a crashed session is respawned repeatedly (crash loop), pending state is
  carried along each hop. The child monitor bounds this with
  `MAX_RESPAWN_ATTEMPTS = 3` per crash chain, after which the session is left
  `Interrupted` for manual intervention.
