# Research: F-15 — Slack Modal Behavior on Mobile and in Threads

**Feature**: 007-acp-correctness-mobile
**Date**: 2026-03-08
**Status**: In Progress — initial findings documented, HITL verification pending

## Findings

### Finding 1: Modals from threaded messages fail on ALL platforms (operator-confirmed)

**Source**: Operator observation during live testing (2026-03-08)
**Severity**: **CRITICAL** — affects all modal interactions across desktop and mobile

**Observation**: Block-kit modals triggered from button actions within a Slack thread
do not render on **either desktop or mobile**. The modal appears to only work when the
triggering message is in the main channel, not when it's a threaded reply.

**Operator confirmation**: "I'm observing this behavior in both mobile and desktop apps."

**Impact**: This affects ALL modal interactions in agent-intercom, not just mobile:
- **Prompt Refine** (`prompt.rs:113`): `slack.open_modal(trigger_id, modal)`
- **Approval Reject** (`approval.rs:202`): `slack.open_modal(trigger_id, modal)`
- **Wait Resume with Instructions** (`wait.rs`): `slack.open_modal(trigger_id, modal)`

All three use `views.open` with a `trigger_id` from a button click in a threaded message
(all agent-intercom messages are posted as thread replies under the session's `thread_ts`).

**Possible root causes**:

1. **`trigger_id` expiry**: Slack `trigger_id` values expire after 3 seconds. If Socket
   Mode message processing introduces latency (Socket Mode reconnections, handler queuing),
   the trigger_id may expire before `views.open` is called. This would affect both desktop
   and mobile, but would be more noticeable on slower connections.

2. **Thread context limitation**: Slack may not support `views.open` when the `trigger_id`
   originates from a threaded message action. The Slack API docs don't explicitly state this
   limitation, but community reports suggest inconsistent behavior.

3. **Socket Mode relay delay**: Socket Mode relays events over WebSocket, adding latency
   compared to direct HTTP-based interactions. The `trigger_id` 3-second window may be
   tighter than expected in Socket Mode.

4. **Mobile client differences**: The Slack iOS/Android clients may handle `views.open`
   responses differently when the triggering context is a thread rather than the main
   channel view.

### Finding 2: Slack API documentation on `views.open`

**Source**: Slack API reference

- `views.open` requires a valid `trigger_id` from a slash command or interactive component
- The `trigger_id` is valid for **3 seconds** after the interaction
- Documentation does not mention thread-specific limitations
- `views.open` is documented to work on mobile (iOS/Android) clients
- `plain_text_input` elements are documented as supported on all surfaces

### Finding 3: Known Slack community reports

**Source**: Slack community forums, Stack Overflow

- Multiple reports of `trigger_id_expired` errors when using Socket Mode
- Socket Mode adds ~100-500ms latency per message relay
- No definitive reports of thread-specific `views.open` failures
- Some reports of modals not appearing on mobile when triggered from threads, but
  these are anecdotal and may be related to trigger_id expiry

## Preliminary Conclusions

The operator has confirmed the issue occurs on **both desktop and mobile**, which rules
out a mobile-only client rendering bug. The most likely root cause is:

**`trigger_id` expiry due to Socket Mode latency.** Socket Mode relays events over
WebSocket, adding 100-500ms per hop. The server then processes the block_action event
through the authorization guard, double-submission prevention (`chat.update`), ownership
check, and handler dispatch — all before calling `views.open`. If total elapsed time
exceeds 3 seconds, the `trigger_id` is expired and the modal silently fails to open.

This is consistent with the observation that main-channel messages (possibly faster to
process) sometimes work while threaded messages (additional thread context lookup) don't.

**Decision**: F-16/F-17 are **unconditionally required** as a reliability fallback for
all platforms. The thread-reply fallback activates whenever `views.open` fails, regardless
of the reason.

## Recommended Investigation Steps (HITL)

1. Add timing instrumentation to measure elapsed time between receiving the block_action
   event and calling `views.open`
2. Test modal opening from a main-channel message (non-threaded) to compare behavior
3. Test on Slack desktop vs. iOS to isolate platform-specific issues
4. Check `views.open` API response for error codes (especially `trigger_id_expired`)

## Impact on F-16/F-17 Decision

Regardless of root cause, this finding **strengthens the case for the thread-reply fallback**
(F-16/F-17). If modals are unreliable in threaded contexts — which is the primary context
for agent-intercom — then a thread-reply alternative is needed for ALL platforms, not just
mobile. The fallback becomes a reliability improvement, not just a mobile accessibility fix.

**Recommendation**: F-16/F-17 are **unconditionally required**. The thread-reply fallback
is a reliability improvement for all platforms, activated when `views.open` fails for any
reason. The conditional gate on F-15 research is removed — proceed directly to implementation.
