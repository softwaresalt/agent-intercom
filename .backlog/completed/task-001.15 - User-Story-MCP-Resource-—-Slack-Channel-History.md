---
id: TASK-001.15
title: "001 - User Story MCP Resource — Slack Channel History"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001
dependencies: []
ordinal: 1150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Expose Slack channel history as an MCP resource for agent context

### Tests (Constitution Principle III)

- [X] T126 Write contract tests for `slack://channel/{id}/recent` resource in `tests/contract/resource_tests.rs`: validate output schema per mcp-resources.json; test channel ID validation against config

### Implementation

- [X] T091 Implement `slack://channel/{id}/recent` MCP resource handler in `src/mcp/resources/slack_channel.rs`: read recent messages from configured Slack channel using `conversations.history` API; return `{messages, has_more}` per mcp-resources.json contract; validate `id` matches `config.slack.channel_id` (FR-018)
- [X] T092 Wire resource handler into `AgentRcServer::read_resource` in `src/mcp/handler.rs`

**Checkpoint**: Agent can read operator instructions from Slack channel

---

<!-- SECTION:DESCRIPTION:END -->
