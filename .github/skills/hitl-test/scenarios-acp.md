---
description: "HITL test scenarios for ACP mode — the Agent Communication Protocol session lifecycle, threading, and workspace routing."
mode: acp
---

# HITL Test Scenarios — ACP Mode

Each scenario tests a specific aspect of the ACP session lifecycle, Slack
session threading, workspace-to-channel routing, and the Slack-mediated
approval workflow when the server is running in **ACP mode** (`--mode acp`,
slash prefix `/arc`). The operator's expected action is stated in **bold**.

> **Pre-flight:** Confirm the server was started with `--mode acp`. The
> slash command prefix is `/arc`. A `[[workspace]]` mapping must exist in
> `config.toml` that maps the test Slack channel to this workspace. The
> `host_cli` and `host_cli_args` config fields must point to a valid ACP
> agent binary (e.g., GitHub Copilot CLI with `--acp` flag).

> **Session IDs:** Most session commands accept an optional `<short-id>` —
> the first 8 characters of the session UUID (e.g., `49621dd2`). Use
> `/arc sessions` to list active sessions and their short IDs. When only
> one session is active in the channel the ID may be omitted, but these
> test scenarios always specify it explicitly for reliability.

---

## Scenario 1: Ping Baseline (ACP)

**Purpose:** Confirm MCP connectivity in ACP mode before testing session lifecycle.

**Steps:**
1. Call `ping` with `status_message: "HITL test (ACP): connectivity check"`
2. Verify response contains `acknowledged: true`
3. Verify the response contains a `pending_steering` field (array — may be empty)

**Expected:** `acknowledged: true` returned, status message posted to Slack channel. This confirms the MCP tool surface is available in ACP mode.

**Known failure modes:**
- Connection refused / timeout — Server is not running or not in ACP mode. Record as FAIL and continue.

---

## Scenario 2: Remote Log Delivery (ACP)

**Purpose:** Verify that `broadcast` messages appear in the Slack channel in ACP mode.

**Steps:**
1. Call `broadcast` with `message: "HITL test (ACP): info level message"`, `level: "info"`
2. Call `broadcast` with `message: "HITL test (ACP): warning level message"`, `level: "warning"`
3. Call `broadcast` with `message: "HITL test (ACP): success level message"`, `level: "success"`
4. Verify each call returns successfully (no error)

**Expected:** Three messages appear in Slack with distinct visual formatting per level.

**Operator validates:**
- [ ] Info message appeared with neutral formatting
- [ ] Warning message appeared with warning indicator
- [ ] Success message appeared with success indicator

---

## Scenario 3: ACP Session Start via Slash Command

**Purpose:** Verify the ACP session lifecycle — starting a new session via the `/arc session-start` slash command (S024, T058). The server spawns the agent process, performs the ACP handshake, and posts a "session started" message as the thread root. This is the core ACP differentiator from MCP mode.

**Steps:**
1. Call `broadcast` with `message: "[TEST] About to test ACP session start. Operator: please run '/arc session-start Implement unit tests for the config module' in Slack within 30 seconds."`, `level: "info"`
2. **Operator action: In Slack, type `/arc session-start Implement unit tests for the config module` and send**
3. Wait approximately 10 seconds for the session to spawn and handshake to complete
4. **Operator action: Observe Slack. Verify the following:**
   - A "session started" Block Kit message appeared
   - The message shows a session ID (short hash)
   - The workspace name is displayed
   - The prompt text is shown

**Expected:** `/arc session-start` accepted. Agent process launched, ACP handshake completed, session record created in DB, "session started" message posted to Slack.

**Known failure modes:**
- `"max concurrent ACP sessions reached"` — Too many active sessions. The operator should stop an existing session first.
- `"host_cli is not configured"` — The `host_cli` / `host_cli_args` fields in `config.toml` are not set. Record as FAIL and skip remaining ACP session scenarios.
- Handshake timeout — The agent process failed to respond within `startup_timeout_seconds`. Record as FAIL and provide the timeout duration.

**Operator validates:**
- [ ] `/arc session-start` command was accepted with an immediate acknowledgement
- [ ] After a few seconds, a "session started" Block Kit message appeared in the channel
- [ ] Message includes session ID, workspace name, and prompt text
- [ ] No error messages appeared

---

## Scenario 4: Session Threading Verification

**Purpose:** Verify that all messages for an active ACP session are threaded under the "session started" root message (T058, S036). This ensures the operator can follow per-session conversation threads.

**Prerequisites:** Scenario 3 must have passed (an active ACP session exists).

**Steps:**
1. Call `broadcast` with `message: "[TEST] This message should appear as a top-level channel message, NOT in the session thread."`, `level: "info"`
2. **Operator action: Verify the broadcast appeared as a top-level message in the channel (not threaded)**
3. Call `broadcast` with `message: "[TEST] About to test session threading. Operator: please run '/arc steer Check if this steering message appears in the session thread' in Slack."`, `level: "info"`
4. **Operator action: In Slack, type `/arc steer Check threading` and send**
5. Wait 5 seconds for processing
6. **Operator action: Verify the steering confirmation message appears as a reply in the session's thread (not top-level)**

**Expected:** Broadcast messages are top-level channel messages. Session-specific interactions (steering, approval requests, status updates) are threaded under the session's root message.

**Operator validates:**
- [ ] Broadcast messages appeared as top-level channel messages
- [ ] Steering confirmation appeared as a threaded reply under the session-started message
- [ ] Thread structure is clear and navigable

---

## Scenario 5: Session List

**Purpose:** Verify the `/arc sessions` command lists all tracked sessions with correct status.

**Prerequisites:** Scenario 3 must have passed (at least one session exists).

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: please run '/arc sessions' in Slack."`, `level: "info"`
2. **Operator action: In Slack, type `/arc sessions` and send**
3. **Operator action: Verify the response includes a list of sessions with:**
   - Session ID
   - Status (e.g., `active`, `paused`, `terminated`)
   - Owner
   - Created timestamp
   - Protocol mode showing `acp`

**Expected:** Sessions list returned. At least one session from Scenario 3 is shown with `active` status and `acp` protocol.

**Operator validates:**
- [ ] `/arc sessions` returned a formatted list
- [ ] Active ACP session from Scenario 3 is visible
- [ ] Session shows protocol mode as `acp`

---

## Scenario 6: Session Pause and Resume

**Purpose:** Verify that an active ACP session can be paused and resumed via slash commands.

**Prerequisites:** Scenario 3 must have passed (an active ACP session exists).

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: first run '/arc sessions' to note the session short ID (first 8 characters of the UUID), then run '/arc session-pause <short-id>' to pause it."`, `level: "info"`
2. **Operator action: Run `/arc sessions`, note the 8-character short ID (e.g., `49621dd2`), then type `/arc session-pause <short-id>` and send**
3. **Operator action: Verify the response confirms the session was paused**
4. Call `broadcast` with `message: "[TEST] Operator: now run '/arc sessions' to confirm the session shows as paused."`, `level: "info"`
5. **Operator action: Run `/arc sessions` and verify the session status is `paused`**
6. Call `broadcast` with `message: "[TEST] Operator: now run '/arc session-resume <short-id>' to resume the session (use the same short ID from step 2)."`, `level: "info"`
7. **Operator action: In Slack, type `/arc session-resume <short-id>` and send**
8. **Operator action: Verify the response confirms the session was resumed**

**Expected:** Session transitions: active → paused → active. State changes are reflected in `/arc sessions`.

**Operator validates:**
- [ ] `/arc session-pause` succeeded with confirmation message
- [ ] Sessions list showed `paused` status
- [ ] `/arc session-resume` succeeded with confirmation message
- [ ] Sessions list shows `active` status again after resume

---

## Scenario 7: Approval in ACP Session Context (@-mention, US17)

**Purpose:** Test the full approval round-trip in ACP mode using the US17 text-only thread reply flow. Because ACP sessions always have a `session_thread_ts`, `check_clearance` posts a **plain-text** proposal to the session thread — no block-kit buttons. The operator replies with `@agent-intercom approve` in the thread.

**Steps:**
1. Call `check_clearance` with:
   - `title`: `"HITL Test (ACP): Create test fixture file"`
   - `diff`: `"# HITL ACP Test Fixture\nGenerated by HITL test suite in ACP mode.\n"`
   - `file_path`: `"tests/fixtures/hitl-acp-test-file.txt"`
   - `description`: `"HITL TEST (ACP): Reply '@agent-intercom approve' in the session thread to approve."`
   - `risk_level`: `"high"`
2. **Operator action:** Observe the session thread. Verify a **plain-text** proposal message appeared with NO Approve/Reject buttons.
3. **Operator action: Reply in the session thread with `@agent-intercom approve`**
4. Verify response `status` is `"approved"` and `request_id` is a non-empty string
5. Call `check_diff` with the returned `request_id`
6. Verify response `status` is `"applied"`
7. Verify the file `tests/fixtures/hitl-acp-test-file.txt` exists on disk (use terminal: `Test-Path tests/fixtures/hitl-acp-test-file.txt`)

**Expected:** Text-only proposal posted to session thread. `@agent-intercom approve` resolves it. File written.

**Operator validates:**
- [ ] Plain-text approval proposal appeared in the session thread (NOT a block-kit card with buttons)
- [ ] NO Approve / Reject buttons were present
- [ ] Message included `@agent-intercom` usage instructions
- [ ] `@agent-intercom approve` reply resolved the approval
- [ ] File was created on disk

---

## Scenario 8: Rejection in ACP Session Context (@-mention, US17)

**Purpose:** Verify the text-only rejection flow in ACP mode. The operator replies `@agent-intercom reject <reason>` in the session thread.

**Steps:**
1. Call `check_clearance` with:
   - `title`: `"HITL Test (ACP): Intentionally rejected change"`
   - `diff`: `"+this line should never be written (ACP)\n"`
   - `file_path`: `"tests/fixtures/hitl-acp-rejected-file.txt"`
   - `description`: `"HITL TEST (ACP): Reply '@agent-intercom reject testing rejection flow' in the session thread."`
   - `risk_level`: `"low"`
2. **Operator action:** Observe the session thread. Verify the proposal is plain text with NO buttons.
3. **Operator action: Reply in the session thread with `@agent-intercom reject testing rejection flow`**
4. Verify response `status` is `"rejected"`
5. Verify response `reason` field contains `"testing rejection flow"`
6. Do NOT call `check_diff`
7. Verify `tests/fixtures/hitl-acp-rejected-file.txt` does NOT exist on disk

**Expected:** Text-only proposal posted. `@agent-intercom reject <reason>` rejects it with the supplied reason. No file written.

**Operator validates:**
- [ ] Plain-text rejection proposal appeared in the session thread (no buttons)
- [ ] `@agent-intercom reject <reason>` reply was accepted
- [ ] Rejection reason was preserved in the tool response
- [ ] No file was created on disk

---

## Scenario 9: Forward Prompt in ACP Mode (@-mention, US17)

**Purpose:** Test bidirectional communication in ACP mode using the US17 text-only thread reply flow. Because the ACP session has a `session_thread_ts`, `transmit` posts a **plain-text** prompt to the session thread — no reply button, no modal. The operator responds by @-mentioning the bot directly in the thread.

**Steps:**
1. Call `broadcast` with `message: "[TEST] About to call transmit in ACP mode (US17 text-only). Operator: a plain-text message will appear in the session thread — reply with '@agent-intercom continue' (no buttons to click)."`, `level: "info"`
2. Call `transmit` with:
   - `prompt_text`: `"HITL TEST (ACP, US17): Reply in this thread with '@agent-intercom continue' to proceed, or '@agent-intercom refine <instructions>' to provide guidance."`
3. **Operator action:** Observe the session thread. Verify:
   - A **plain-text** message appeared in the thread (NOT a block-kit card)
   - There is NO reply button and NO modal
   - The message instructs how to respond via @-mention
4. **Operator action: Reply in the thread with `@agent-intercom continue`**
5. Verify the response contains `decision: "continue"` (or equivalent continue decision)

**Expected:** Text-only prompt posted to thread. No block-kit, no modal. `@agent-intercom continue` resolves the call.

**Error resilience:** `transmit` is a blocking call. If no response is received within a reasonable time, record as FAIL/TIMEOUT and continue.

**Operator validates:**
- [ ] Plain-text prompt appeared in the session thread (no block-kit card, no buttons)
- [ ] Message contained @-mention instructions for responding
- [ ] There was NO "Continue / Refine / Stop" button and NO modal dialog
- [ ] `@agent-intercom continue` reply resolved the `transmit` call

---

## Scenario 10: Wait for Instruction in ACP Mode (@-mention, US17)

**Purpose:** Test the passive wait mode in ACP mode using the US17 text-only thread reply flow. `standby` posts a **plain-text** standby message to the session thread — no "Resume with Instructions" button, no modal. The operator replies with `@agent-intercom resume <instructions>` in the thread.

**Steps:**
1. Call `broadcast` with `message: "[TEST] About to call standby in ACP mode (US17 text-only). When the standby message appears in the session thread, reply '@agent-intercom resume focus on error handling' — there is no button to click."`, `level: "info"`
2. Call `standby` with `message: "HITL TEST (ACP, US17): Agent paused. Reply in this thread with '@agent-intercom resume <your instruction>' to continue."`, `timeout_seconds: 120`
3. **Operator action:** Observe the session thread. Verify:
   - A **plain-text** standby message appeared in the thread
   - There is NO "Resume with Instructions" button
   - The message instructs how to respond via @-mention
4. **Operator action: Reply in the session thread with `@agent-intercom resume focus on error handling`**
5. Verify the response:
   - `status` is `"resumed"`
   - `instruction` (or equivalent field) contains `"focus on error handling"`

**Expected:** Text-only standby message posted to thread. No button, no modal. `@agent-intercom resume <text>` resolves it with instruction text preserved.

**Error resilience:** `standby` is a blocking call. If no response within `timeout_seconds`, it returns a timeout response. Record as FAIL/TIMEOUT if it hangs beyond the configured timeout.

**Operator validates:**
- [ ] Plain-text standby message appeared in the session thread (no block-kit, no button)
- [ ] There was NO "Resume with Instructions" button
- [ ] Message contained @-mention instructions
- [ ] `@agent-intercom resume <text>` reply resolved the standby call
- [ ] Instruction text was preserved in the tool response

---

## Scenario 11: Operator Steering Queue (ACP)

**Purpose:** Validate the steering queue works with the `/arc` prefix.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: please run '/arc steer HITL-ACP-STEER-MSG' in Slack within 30 seconds."`, `level: "info"`
2. **Operator action: In Slack, type `/arc steer HITL-ACP-STEER-MSG` and send**
3. Wait approximately 5 seconds for the message to be stored
4. Call `ping` with `status_message: "HITL test (ACP): checking steering queue"`
5. Verify the response contains `pending_steering` as a non-empty array
6. Verify at least one entry in `pending_steering` contains the text `"HITL-ACP-STEER-MSG"`
7. Call `ping` again with `status_message: "HITL test (ACP): verifying consumed"`
8. Verify the second ping's `pending_steering` is empty

**Expected:** Steering message delivered and consumed on first ping (same behavior as MCP mode).

**Operator validates:**
- [ ] `/arc steer` command was accepted without error
- [ ] Steering confirmation appeared in Slack (threaded under session if applicable)

---

## Scenario 12: Task Inbox Delivery (ACP)

**Purpose:** Validate the task inbox works with the `/arc` prefix.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: please run '/arc task HITL-ACP-TASK-ITEM' in Slack within 30 seconds."`, `level: "info"`
2. **Operator action: In Slack, type `/arc task HITL-ACP-TASK-ITEM` and send**
3. Wait approximately 5 seconds for the item to be stored
4. Call `reboot` with no arguments
5. Verify the response contains a `pending_tasks` field with a non-empty array
6. Verify at least one entry in `pending_tasks` contains the text `"HITL-ACP-TASK-ITEM"`

**Expected:** Task inbox item queued and delivered in `reboot` response.

**Operator validates:**
- [ ] `/arc task` command was accepted without error
- [ ] Task confirmation appeared in Slack

---

## Scenario 13: Session Checkpoint and Restore

**Purpose:** Verify the checkpoint/restore cycle for ACP sessions.

**Prerequisites:** An active ACP session from Scenario 3 must exist.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: run '/arc sessions' to get the session short ID (first 8 chars of UUID), then run '/arc session-checkpoint <short-id> hitl-test-checkpoint' to create a checkpoint. The first argument is the session short ID, the second is the label."`, `level: "info"`
2. **Operator action: Run `/arc sessions` to get the session short ID (e.g., `49621dd2`), then type `/arc session-checkpoint <short-id> hitl-test-checkpoint` and send**
3. **Operator action: Verify the response includes a checkpoint ID**
4. Call `broadcast` with `message: "[TEST] Operator: now run '/arc session-checkpoints <short-id>' to list checkpoints (use the same session short ID)."`, `level: "info"`
5. **Operator action: Run `/arc session-checkpoints <short-id>` and verify the list includes the checkpoint labeled `hitl-test-checkpoint`**

**Expected:** Checkpoint created and listed. The checkpoint label matches what was provided.

**Operator validates:**
- [ ] `/arc session-checkpoint` created a checkpoint with a returned ID
- [ ] `/arc session-checkpoints` shows the checkpoint with correct label
- [ ] Checkpoint includes a timestamp

---

## Scenario 14: Session Stop (Graceful)

**Purpose:** Verify `/arc session-stop` sends an interrupt to the agent process, terminates it, and posts a "session stopped" notification to the session thread.

**Prerequisites:** An active ACP session from Scenario 3 must exist.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Operator: run '/arc sessions' to get the session short ID (first 8 chars of UUID), then run '/arc session-stop <short-id>' to gracefully stop it."`, `level: "info"`
2. **Operator action: Run `/arc sessions` to get the session short ID, then type `/arc session-stop <short-id>` and send**
3. **Operator action: Verify the response confirms the session was stopped**
4. **Operator action: Check the session's thread — a "session stopped" notification should appear as a threaded reply**
5. Call `broadcast` with `message: "[TEST] Operator: run '/arc sessions' to verify the session shows as terminated."`, `level: "info"`
6. **Operator action: Run `/arc sessions` and verify the session status is `terminated`**

**Expected:** Session stopped gracefully. Thread receives a "stopped by operator" notification. Session status transitions to terminated.

**Operator validates:**
- [ ] `/arc session-stop` succeeded with confirmation
- [ ] "Session stopped" notification appeared in the session thread
- [ ] Sessions list shows `terminated` status

---

## Scenario 15: Session Restart

**Purpose:** Verify `/arc session-restart` terminates the current session and spawns a new one with the same original prompt.

**Steps:**
1. First start a fresh session:
   Call `broadcast` with `message: "[TEST] Operator: please start a session with '/arc session-start Test restart feature for HITL'."`, `level: "info"`
2. **Operator action: Run `/arc session-start Test restart feature for HITL`**
3. Wait 10 seconds for the session to fully start
4. Call `broadcast` with `message: "[TEST] Operator: run '/arc sessions' to get the session short ID (first 8 chars of UUID), then restart with '/arc session-restart <short-id>'."`, `level: "info"`
5. **Operator action: Run `/arc sessions` to get the session short ID (e.g., `49621dd2`), then type `/arc session-restart <short-id>` and send**
6. **Operator action: Verify:**
   - The old session thread receives a "restarting" notification
   - A new "session started" message appears (possibly in a new thread)
   - The new session's prompt matches the original prompt
7. Wait 10 seconds for the new session to stabilize

**Expected:** Old session terminated. New session started with the same prompt. Restart notification posted to old session thread.

**Operator validates:**
- [ ] Restart notification appeared in the old session's thread
- [ ] New "session started" message appeared with the original prompt
- [ ] New session is active per `/arc sessions`

---

## Scenario 16: Max Concurrent Sessions Enforcement

**Purpose:** Verify the server enforces the `acp.max_sessions` limit (S024). Attempting to start more sessions than configured should return an error.

**Prerequisites:** Check the current `max_sessions` value in `config.toml` (default: 5).

**Steps:**
1. Call `broadcast` with `message: "[TEST] Testing max session enforcement. Operator: start sessions until the limit is reached."`, `level: "info"`
2. **Operator action: Note how many active sessions exist via `/arc sessions`**
3. **If the number of active sessions equals `max_sessions`:**
   - **Operator action: Try `/arc session-start This should fail` and verify it returns an error about max sessions reached**
4. **If there is room:**
   - Record current count and mark as PASS (configuration validated, limit not yet hit)

**Expected:** When the limit is reached, `/arc session-start` returns `"max concurrent ACP sessions reached"`.

**Note:** If `max_sessions` is set to a high value, this scenario may be validated observationally. Record as PASS with the configured limit noted.

**Operator validates:**
- [ ] When limit is reached, error message clearly states the max session count
- [ ] No session was created when the limit was enforced

---

## Scenario 17: ACP-Only Commands Rejected in Wrong Mode

**Purpose:** Observational/documentation check — verify that ACP-only commands (`session-start`, `session-stop`, `session-restart`) are properly gated behind ACP mode. Since we are in ACP mode for this suite, we cannot directly test MCP-mode rejection, but we can verify the commands work here.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Verifying ACP-only commands are available. Operator: run '/arc help session' to see ACP-specific commands."`, `level: "info"`
2. **Operator action: Run `/arc help session`**
3. **Operator action: Verify the help output includes `session-start`, `session-stop`, and `session-restart`**

**Expected:** Help text includes ACP-specific session lifecycle commands.

**Note:** To test that these commands are rejected in MCP mode, the server would need to be restarted with `--mode mcp`. This is outside the scope of this ACP test suite. Record as PASS if the commands are documented and functional in ACP mode.

**Operator validates:**
- [ ] `/arc help session` shows `session-start`, `session-stop`, `session-restart`
- [ ] Help text is well-formatted and informative

---

## Scenario 18: Workspace-to-Channel Mapping Observation

**Purpose:** Verify that the workspace-to-channel mapping from `config.toml` is active. Messages are routed to the correct channel based on the `[[workspace]]` config.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Workspace mapping check — this message arrives in the channel mapped to this workspace. The mapping is defined in config.toml [[workspace]] entries."`, `level: "info"`
2. Verify the call returned successfully
3. Use terminal to read the workspace mapping: `Get-Content config.toml | Select-String -Pattern 'workspace_id|channel_id|label' -Context 0,0`
4. Record the workspace-to-channel mapping for the current test channel

**Expected:** Message appears in the correct Slack channel per the workspace mapping. The config reports a `[[workspace]]` entry for this channel.

**Note:** If no workspace mapping exists for the test channel, the server falls back to `default_workspace_root`. Record this distinction.

**Operator validates:**
- [ ] Message appeared in the expected channel
- [ ] Channel matches the `channel_id` in the `[[workspace]]` config

---

## Scenario 19: Stall Detection Observation (ACP)

**Purpose:** Confirm stall detection is active for ACP sessions. In ACP mode, the stall detector monitors the session's heartbeat and notifies the operator if the agent stops communicating.

**Steps:**
1. Call `ping` with `status_message: "HITL test (ACP): stall detector active check"`
2. Verify the ping successfully resets the heartbeat timer
3. Call `broadcast` with `message: "[TEST] Stall detector check passed in ACP mode. The ACP stall detector monitors session activity and would alert if the agent process stops communicating."`, `level: "info"`

**Expected:** Ping succeeds and no stall notification fires.

---

## Scenario 20: Session Clear (Force Terminate)

**Purpose:** Verify `/arc session-clear` force-terminates a session regardless of state. Unlike `session-stop` (which sends an interrupt first), `session-clear` is immediate.

**Steps:**
1. If no active ACP session exists, start one:
   Call `broadcast` with `message: "[TEST] Operator: start a session with '/arc session-start Disposable session for clear test'."`, `level: "info"`
2. **Operator action: Start a session if needed**
3. Wait 10 seconds
4. Call `broadcast` with `message: "[TEST] Operator: run '/arc sessions' to get the session short ID (first 8 chars of UUID), then run '/arc session-clear <short-id>' to force-terminate it."`, `level: "info"`
5. **Operator action: Run `/arc sessions` to get the session short ID, then type `/arc session-clear <short-id>` and send**
6. **Operator action: Verify the response confirms the session was terminated**
7. **Operator action: Verify the session thread shows a "terminated by operator" notification**

**Expected:** Session force-terminated immediately. Thread notification posted.

**Operator validates:**
- [ ] `/arc session-clear` succeeded with confirmation
- [ ] Session thread shows termination notification
- [ ] Session appears as `terminated` in `/arc sessions`

---

## Scenario 21: Audit Log Verification (ACP)

**Purpose:** Verify that ACP session lifecycle events are recorded in the audit log.

**Steps:**
1. Use terminal to check audit log: `Test-Path .intercom/logs`
2. If the directory exists, list today's audit file: `Get-ChildItem .intercom/logs/audit-*.jsonl -ErrorAction SilentlyContinue`
3. If an audit file exists, read the last 15 lines: `Get-Content .intercom/logs/audit-{today}.jsonl -Tail 15`
4. Verify entries include ACP-specific events (session creation, session termination, tool calls)
5. Look for entries with session IDs matching the sessions created in this suite

**Expected:** Audit log contains structured entries for ACP session lifecycle events. Each entry is valid JSON with `timestamp`, `session_id`, and `event_type` fields.

**Note:** If audit logging is disabled, record as SKIP.

---

## Scenario 22: Cleanup

**Purpose:** Remove test artifacts and stop any remaining ACP sessions created during the suite.

**Steps:**
1. Call `broadcast` with `message: "[TEST] Cleanup phase. Stopping remaining test sessions."`, `level: "info"`
2. **Operator action: Run `/arc sessions` to list active sessions**
3. For each active session created during this test suite:
   - **Operator action: Run `/arc session-clear {session_id}` to force-terminate**
4. Use terminal to list test fixture files: `Get-ChildItem tests/fixtures/hitl-acp-* -ErrorAction SilentlyContinue`
5. For each file found, call `check_clearance` to propose deletion:
   - `title`: `"HITL Test (ACP): Cleanup — delete {filename}"`
   - `diff`: Unified diff showing file deletion
   - `file_path`: The file path
   - `description`: `"HITL TEST (ACP): Please APPROVE to clean up test artifacts."`
   - `risk_level`: `"low"`
6. **Operator action: APPROVE each cleanup proposal**
7. Call `check_diff` for each approved cleanup
8. Verify all `hitl-acp-*` files have been removed from `tests/fixtures/`

**Expected:** All test sessions terminated. All test fixture files cleaned up.

**Note:** If no sessions or files remain (prior scenarios failed or were skipped), mark as PASS.

---

## Scenario 23: Text-Only Prompt — Refine with Instructions (@-mention, US17)

**Purpose:** Verify that `@agent-intercom refine <instructions>` in an ACP session thread carries the full instruction text back to the calling tool, demonstrating the refined decision path.

**Prerequisites:** An active ACP session must exist (see Scenario 3).

**Steps:**
1. Call `broadcast` with `message: "[TEST] Testing @-mention refine flow (US17). Operator: when the prompt appears in the session thread, reply '@agent-intercom refine focus on error handling'."`, `level: "info"`
2. Call `transmit` with:
   - `prompt_text`: `"HITL TEST (ACP, US17): Reply in this thread with '@agent-intercom refine focus on error handling' to test the refinement flow."`
3. **Operator action:** Observe the session thread — verify plain-text message with no buttons.
4. **Operator action: Reply in the session thread with `@agent-intercom refine focus on error handling`**
5. Verify the `transmit` response:
   - `decision` is `"refine"` (or contains the keyword)
   - The instruction text `"focus on error handling"` is present in the response

**Expected:** `@agent-intercom refine <text>` carries decision keyword and instruction text to the agent.

**Operator validates:**
- [ ] Plain-text prompt appeared in session thread (no buttons)
- [ ] `@agent-intercom refine <text>` reply was accepted
- [ ] Full instruction text was preserved in the tool response

---

## Scenario 24: Text-Only Approval — Thread @-mention vs. Main Channel Block-Kit (Contrast Test, US17)

**Purpose:** Verify the US17 behavior boundary: messages IN a session thread use text-only @-mention; messages in the main channel (no thread) continue to use block-kit buttons. Run both cases back-to-back for comparison.

**Prerequisites:** An active ACP session exists.

**Steps — Part A: Threaded text-only (ACP session thread)**
1. Call `broadcast` with `message: "[TEST] US17 contrast test — Part A. The next check_clearance will post to the session THREAD as text-only."`, `level: "info"`
2. Call `check_clearance` with:
   - `title`: `"HITL Test (US17 contrast): Threaded text-only approval"`
   - `diff`: `"+# US17 thread test\n"`
   - `file_path`: `"tests/fixtures/hitl-acp-us17-contrast.txt"`
   - `description`: `"HITL TEST: Reply '@agent-intercom approve' in the SESSION THREAD — no button to click."`
   - `risk_level`: `"low"`
3. **Operator action:** Verify the proposal appeared in the **session thread** as plain text with **no buttons**.
4. **Operator action: Reply `@agent-intercom approve` in the session thread**
5. Verify `status` is `"approved"`. Call `check_diff` and verify `status` is `"applied"`.

**Steps — Part B: Main channel block-kit (MCP context, requires switching to MCP mode)**
> **Note:** Part B requires the server to be in MCP mode with no session_thread_ts active. If the test is running in ACP mode, skip Part B and mark as PARTIAL PASS. The equivalent test is MCP Scenario 28.

6. Record the contrast observation: threaded ACP sessions use text-only @-mention; non-threaded messages use block-kit buttons.

**Expected (Part A):** Text-only in thread. Approved via @-mention. File written.

**Operator validates:**
- [ ] Part A: Proposal appeared in the session thread as plain text (no buttons)
- [ ] Part A: `@agent-intercom approve` resolved the approval
- [ ] File `tests/fixtures/hitl-acp-us17-contrast.txt` created on disk
