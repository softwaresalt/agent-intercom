---
id: TASK-001.10.08
title: "001-002 - Parallel Example: Phase 2 (US6)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-001.10
dependencies: []
ordinal: 1180
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

```text
# All policy watcher tests can be written in one pass:
T004: register_loads_initial_policy
T005: policy_file_modification_detected
T006: policy_file_deletion_falls_back_to_deny_all
T007: malformed_policy_file_uses_deny_all
T008: unregister_stops_watching
T009: multiple_workspaces_independent_policies
```

---

<!-- SECTION:DESCRIPTION:END -->
