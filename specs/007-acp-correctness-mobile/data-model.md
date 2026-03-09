# Data Model: Feature 007 — ACP Correctness Fixes and Mobile Operator Accessibility

**Feature**: 007-acp-correctness-mobile
**Date**: 2026-03-08

## Affected Entities

This feature does not introduce new entities. It modifies the behavior of existing entities
and adds a new repository query method. Below are the entities affected by each fix.

### SteeringMessage (F-06)

**Table**: `steering_message`
**Module**: `src/models/steering.rs`, `src/persistence/steering_repo.rs`

| Field | Type | Description |
|---|---|---|
| id | TEXT (PK) | UUID-based message identifier |
| session_id | TEXT (FK) | Target session |
| message | TEXT | Operator instruction text |
| source | TEXT | Origin: `slack`, `ipc`, `command` |
| consumed | INTEGER | 0 = unconsumed (queued), 1 = consumed (delivered) |
| created_at | TEXT | ISO 8601 timestamp |

**Behavioral change**: The `consumed` flag is now set ONLY after successful delivery via
`send_prompt()`. Previously it was set unconditionally after the delivery attempt,
causing silent message loss on transient errors.

**State transition**:
```
Created (consumed=0) ──send_prompt OK──► Consumed (consumed=1)
Created (consumed=0) ──send_prompt ERR──► Created (consumed=0)  [stays queued for retry]
```

---

### Session (F-07)

**Table**: `session`
**Module**: `src/models/session.rs`, `src/persistence/session_repo.rs`

| Field | Type | Relevant Values |
|---|---|---|
| status | TEXT | `created`, `active`, `paused`, `terminated`, `interrupted` |
| protocol_mode | TEXT | `acp`, `mcp` |

**New query**: `count_active_acp()` — counts sessions where
`(status = 'active' OR status = 'created') AND protocol_mode = 'acp'`.

**Behavioral change**: ACP session capacity check now includes `created` (initializing)
sessions and filters by `protocol_mode = 'acp'`, preventing:
1. Race condition where multiple sessions start concurrently past the limit
2. MCP connections counting against the ACP session quota

---

### PendingParams / MCP Connection (F-10)

**Type**: `Arc<Mutex<(Option<String>, Option<String>, Option<String>)>>`
**Module**: `src/mcp/sse.rs`

**Current**: 3-tuple `(channel_id, session_id, workspace_id)`
**After**: 2-tuple `(session_id, workspace_id)` — `channel_id` slot removed

The factory closure no longer reads or uses `raw_channel`. Channel resolution is
exclusively via workspace mappings from the `workspace_id` parameter.

---

### PromptCorrelationId (F-13)

**Location**: `src/acp/handshake.rs`, `src/driver/acp_driver.rs`

**Current IDs**:
- Handshake: static `"intercom-init-1"`, `"intercom-sess-1"`, `"intercom-prompt-1"`
- Runtime: `PROMPT_COUNTER` (static AtomicU64 starting at 1) → `"intercom-prompt-{N}"`

**After**:
- Handshake: `"intercom-init-{uuid}"`, `"intercom-sess-{uuid}"`, `"intercom-prompt-{uuid}"`
- Runtime: `"intercom-prompt-{uuid}"` (PROMPT_COUNTER removed)

**Uniqueness guarantee**: UUID v4 provides 2^122 bits of randomness. Collision probability
is negligible across any practical number of concurrent sessions and server restarts.

---

### ThreadReplyInput (F-16 — Conditional)

**Not yet defined**. Will be designed during Phase 6 implementation only if F-15 research
confirms that Slack modals are broken on iOS. Preliminary design:

| Field | Type | Description |
|---|---|---|
| channel_id | String | Slack channel where the reply is expected |
| thread_ts | String | Thread timestamp for scoping reply detection |
| entity_type | String | `prompt_refine`, `wait_instruct`, `approval_reject` |
| entity_id | String | The prompt_id, session_id, or request_id |
| oneshot_tx | Sender | Channel to resolve the waiting interaction |

Would be stored in `AppState::pending_thread_replies` (new field, conditional on F-16).
