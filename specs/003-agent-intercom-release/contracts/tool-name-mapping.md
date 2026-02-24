# Tool Name Mapping Contract: 003-agent-intercom-release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23

## Contract Summary

This document defines the authoritative mapping between old MCP tool names and new intercom-themed tool names. The rename affects ONLY the tool `name` field in the MCP protocol. Input schemas, output schemas, and behavior are unchanged.

## Tool Name Contracts

### check_clearance (was: ask_approval)

**Name**: `check_clearance`  
**Blocking**: Yes  
**Input schema**: Unchanged — `title`, `diff`, `file_path`, `description`, `risk_level`  
**Output schema**: Unchanged — `status` (approved/rejected/timeout), `request_id`, `reason`

### check_diff (was: accept_diff)

**Name**: `check_diff`  
**Blocking**: No  
**Input schema**: Unchanged — `request_id`, `force`  
**Output schema**: Unchanged — `status` (applied), `files_written`

### auto_check (was: check_auto_approve)

**Name**: `auto_check`  
**Blocking**: No  
**Input schema**: Unchanged — `tool_name`, `context`  
**Output schema**: Unchanged — `auto_approved`, `reason`

### transmit (was: forward_prompt)

**Name**: `transmit`  
**Blocking**: Yes  
**Input schema**: Unchanged — `prompt`, `options`  
**Output schema**: Unchanged — `response`, `action`

### standby (was: wait_for_instruction)

**Name**: `standby`  
**Blocking**: Yes  
**Input schema**: Unchanged — `timeout_seconds`  
**Output schema**: Unchanged — `instruction`, `source`

### signal (was: heartbeat)

**Name**: `signal`  
**Blocking**: No  
**Input schema**: Unchanged — `session_id` (optional), `progress`  
**Output schema**: Unchanged — `session_id`, `status`, `stall_warning`

### broadcast (was: remote_log)

**Name**: `broadcast`  
**Blocking**: No  
**Input schema**: Unchanged — `message`, `level`  
**Output schema**: Unchanged — `delivered`

### reboot (was: recover_state)

**Name**: `reboot`  
**Blocking**: No  
**Input schema**: Unchanged — (no required params)  
**Output schema**: Unchanged — `has_interrupted_session`, `session`, `pending_approvals`

### switch_freq (was: set_operational_mode)

**Name**: `switch_freq`  
**Blocking**: No  
**Input schema**: Unchanged — `mode` (remote/local/hybrid)  
**Output schema**: Unchanged — `mode`, `previous_mode`

## Notification Contracts (New)

### accept_diff Success Notification

**Trigger**: `check_diff` (accept_diff) applies patch successfully  
**Channel**: Session's configured Slack channel  
**Content**: File path, bytes written, approval request_id  
**Block Kit**: Section block with success emoji + file details

### accept_diff Conflict Notification

**Trigger**: `check_diff` (accept_diff) encounters hash mismatch without `force: true`  
**Channel**: Session's configured Slack channel  
**Content**: File path, expected hash, actual hash, guidance to re-propose  
**Block Kit**: Section block with warning emoji + conflict details

### accept_diff Force-Apply Warning

**Trigger**: `check_diff` (accept_diff) called with `force: true` and hash mismatch  
**Channel**: Session's configured Slack channel  
**Content**: File path, warning that file conflict was overridden  
**Block Kit**: Section block with alert emoji + force-apply warning

### No Channel Error Response

**Trigger**: `check_clearance` (ask_approval) called without configured Slack channel  
**Channel**: N/A — returns error to agent  
**Content**: Descriptive error explaining no Slack channel is configured for this session  
**Response**: `CallToolResult` with `is_error: true`

### Rejection Delivery Confirmation

**Trigger**: Operator rejects a proposal via Slack buttons  
**Channel**: Session's configured Slack channel  
**Content**: Confirmation that rejection was delivered to the agent  
**Block Kit**: Context block appended to original approval message
