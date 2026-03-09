# Test Harness Contracts: Slack UI Automated Testing

**Feature**: 008-slack-ui-testing | **Date**: 2026-03-09

## Tier 1 — Block Kit Assertion Contracts

Each Block Kit builder function in `blocks.rs` must produce output matching these contracts.

### `severity_section(level, message)` → SlackBlock::Section

```json
{
  "type": "section",
  "text": {
    "type": "mrkdwn",
    "text": "{emoji} {message}"
  }
}
```

Where `{emoji}` is:
- `level = "success"` → ✅
- `level = "warning"` → ⚠️
- `level = "error"` → ❌
- `level = _` (info/default) → ℹ️

### `approval_buttons(request_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "approval_{request_id}",
  "elements": [
    { "type": "button", "action_id": "approve_accept", "text": { "text": "Accept" }, "value": "{request_id}" },
    { "type": "button", "action_id": "approve_reject", "text": { "text": "Reject" }, "value": "{request_id}" }
  ]
}
```

### `prompt_buttons(prompt_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "prompt_{prompt_id}",
  "elements": [
    { "type": "button", "action_id": "prompt_continue", "text": { "text": "Continue" }, "value": "{prompt_id}" },
    { "type": "button", "action_id": "prompt_refine", "text": { "text": "Refine" }, "value": "{prompt_id}" },
    { "type": "button", "action_id": "prompt_stop", "text": { "text": "Stop" }, "value": "{prompt_id}" }
  ]
}
```

### `nudge_buttons(alert_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "stall_{alert_id}",
  "elements": [
    { "type": "button", "action_id": "stall_nudge", "text": { "text": "Nudge" }, "value": "{alert_id}" },
    { "type": "button", "action_id": "stall_nudge_instruct", "text": { "text": "Nudge with Instructions" }, "value": "{alert_id}" },
    { "type": "button", "action_id": "stall_stop", "text": { "text": "Stop" }, "value": "{alert_id}" }
  ]
}
```

### `wait_buttons(session_id)` → SlackBlock::Actions

```json
{
  "type": "actions",
  "block_id": "wait_{session_id}",
  "elements": [
    { "type": "button", "action_id": "wait_resume", "text": { "text": "Resume" }, "value": "{session_id}" },
    { "type": "button", "action_id": "wait_resume_instruct", "text": { "text": "Resume with Instructions" }, "value": "{session_id}" },
    { "type": "button", "action_id": "wait_stop", "text": { "text": "Stop Session" }, "value": "{session_id}" }
  ]
}
```

### `instruction_modal(callback_id, title, placeholder)` → SlackView::Modal

```json
{
  "type": "modal",
  "callback_id": "{callback_id}",
  "title": { "type": "plain_text", "text": "{title}" },
  "submit": { "type": "plain_text", "text": "Submit" },
  "blocks": [
    {
      "type": "input",
      "block_id": "instruction_block",
      "element": {
        "type": "plain_text_input",
        "action_id": "instruction_text",
        "multiline": true,
        "placeholder": { "type": "plain_text", "text": "{placeholder}" }
      },
      "label": { "type": "plain_text", "text": "Instructions" }
    }
  ]
}
```

### `session_started_blocks(session)` → Vec<SlackBlock>

Must contain a section with:
- Session ID prefix (first 8 chars + "…")
- Protocol mode: "MCP" or "ACP"
- Operational mode: "remote", "local", or "hybrid"
- Workspace root path
- Creation timestamp in "YYYY-MM-DD HH:MM UTC" format

### `stall_alert_blocks(session_id, idle_seconds)` → Vec<SlackBlock>

Must contain:
- Warning severity section with idle duration display
- Nudge/Nudge with Instructions/Stop action buttons

### `command_approval_blocks(command, request_id)` → Vec<SlackBlock>

Must contain:
- Lock emoji (🔐) + "Terminal command approval requested" header
- Command in code fence
- Accept/Reject approval buttons

## Tier 2 — Live Interaction Contracts

### Message Verification (via conversations.history)

After posting a message, the test verifies:
- `messages[0].blocks` matches the expected Block Kit structure
- `messages[0].thread_ts` matches expected threading (None for top-level, parent ts for threaded)
- `messages[0].ts` is a valid Slack timestamp

### Interaction Round-Trip

After dispatching a synthetic interaction payload:
- Database record updated (e.g., `ApprovalRequest.status = "approved"`)
- Oneshot channel resolved (blocking tool call returns)
- Follow-up message posted to correct thread (verified via conversations.replies)

## Tier 3 — Visual Assertion Contracts

### Screenshot Naming Convention

```
{scenario_id}_{step_number}_{description}_{timestamp}.png
```

Example: `modal_in_thread_03_after_click_20260309T064500.png`

### HTML Report Structure

```html
<h1>Tier 3 Visual Test Report — {date}</h1>
<section class="scenario">
  <h2>{scenario_name} — {PASS|FAIL}</h2>
  <div class="step">
    <h3>Step {n}: {description}</h3>
    <img src="screenshots/{filename}" />
    <p class="assertion">{pass|fail}: {what was verified}</p>
  </div>
</section>
```
