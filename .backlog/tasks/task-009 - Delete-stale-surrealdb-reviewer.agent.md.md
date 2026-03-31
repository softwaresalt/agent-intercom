---
id: TASK-009
title: Delete stale surrealdb-reviewer.agent.md
status: To Do
assignee: []
created_date: '2026-03-31 07:17'
labels:
  - tech-debt
  - agents
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The file `.github/agents/review/surrealdb-reviewer.agent.md` is architecturally stale — it reviews SurrealDB patterns but the project migrated to SQLite (sqlx 0.8). It has been replaced by `.github/agents/review/sqlite-reviewer.agent.md`. Delete the old file to prevent agents from receiving incorrect database review guidance. Command: `git rm .github/agents/review/surrealdb-reviewer.agent.md`
<!-- SECTION:DESCRIPTION:END -->
