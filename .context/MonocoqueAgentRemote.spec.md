# **Monocoque Agent Remote (monocoque-agent-rem)**

## **Technical Specification & Architecture**

**Version:** 2.0.0

**Status:** Draft / Request for Comments

**License:** Apache-2.0

**Language:** Rust

**Protocol:** Model Context Protocol (MCP) v1.0

## **1\. Executive Synopsis**

The software entity herein designated as **Monocoque Agent Remote** (monocoque-agent-rem) is constituted as a standalone server implementation of the Model Context Protocol (MCP). The primary objective of this apparatus is the provision of "Remote Input/Output" capabilities to local Artificial Intelligence agents, encompassing but not limited to Claude Code, the GitHub Copilot Command Line Interface (CLI), Cursor, and Visual Studio Code.

Through the local execution of this server, an AI agent is endowed with the capacity to establish an asynchronous, bi-directional communication channel with a remote operator via the Slack platform. In standard operational paradigms, an agentic workflow is tethered to the physical console, requiring synchronous human intervention for the approval of file modifications or the execution of terminal commands. This system decouples that dependency. Rather than the terminal remaining in a blocked state whilst awaiting local input, the agent is enabled to transmit diffs, requests, and inquiries to a designated Slack channel, subsequently awaiting approval in an asynchronous manner. This architecture effectively permits the "Shadow Agent" workflow, wherein the computational heavy lifting occurs on a secure, local machine, while the orchestration and oversight are conducted remotely.

**Version 1.5 Update:** This iteration significantly refines the **Proposal Review Workflow**. It explicitly delineates the mechanism by which code differentials are rendered within the Slack interface—specifically, the utilization of "Smart Snippets" to handle variable content lengths. Furthermore, it rigorously maps the "Accept" action of the remote operator to the file write execution of the agent, thereby mirroring the "View Diff \-\> Accept" experience characteristic of the GitHub Copilot graphical interface.
**Version 1.6 Update:** This iteration introduces two significant capabilities. First, the **Programmatic Diff Acceptance** mechanism via the `accept_diff` tool, which enables the MCP server to apply approved code changes directly to disk on behalf of the agent, eliminating the requirement for manual UI-based "Keep" or "Accept" interactions in the host IDE. Second, the **Workspace Auto-Approve Policy** via a `.monocoque/settings.json` file, which permits per-workspace declaration of commands and tool invocations that bypass the remote approval gate entirely. This is analogous to the `.vscode/settings.json` `chat.tools.terminal.autoApprove` pattern but operates at the MCP server layer, rendering it IDE-agnostic across VS Code, GitHub Copilot CLI, OpenAI Codex, Claude Code CLI, Cursor, and any other MCP-compatible host.

**Version 1.7 Update:** This iteration introduces the **Remote Session Orchestration** capability. The remote operator is no longer constrained to responding to an active agent session; they may now initiate new agent sessions, clear running sessions, create checkpoints, and restore prior session states entirely from the Slack interface. A comprehensive **Command Discovery** mechanism is provided via an enhanced `/monocoque help` command that returns a richly formatted, categorized listing of all available slash commands. This transforms the remote operator from a passive reviewer into an active orchestrator who can drive the local workstation proactively.

**Version 1.8 Update:** This iteration introduces the **Agent Continuation Prompt Forwarding** mechanism. AI agents such as GitHub Copilot, Claude Code, and Codex periodically emit meta-level continuation prompts when they have been operating for an extended duration (e.g., "Copilot has been working on this problem for a while. It can continue to iterate, or you can send a new message to refine your prompt."). These prompts block the agent until the local user responds, which defeats the purpose of remote orchestration. The `forward_prompt` tool intercepts these continuation prompts and relays them to the remote Slack interface with actionable buttons, enabling the remote operator to keep the agent running, stop it, or refine the instruction — all without physical access to the local terminal.

**Version 1.9 Update:** This iteration is a **Spec Hardening** release focused on implementability and completeness. All eight MCP tools now have fully specified parameters, return values, and timeout behaviors. Critical architectural gaps have been resolved: spawned agent sessions connect via SSE/HTTP transport (not stdio inheritance), Slack interactions are restricted to authorized user IDs, and a comprehensive error handling and failover strategy is defined. New additions include the Refine modal dialog JSON specification, the `session-checkpoints` listing command, the Slack resource schema, graceful shutdown behavior, missing crate dependencies (`toml`, `tracing`, `sha2`, `axum`), server transport configuration, command output size limits, and the rejection workflow path. The architectural diagram and session state machine have been corrected to reflect all v1.6–1.8 additions.
## **2\. Architectural Framework**

The system is predicated upon the standard MCP Client/Host architecture, significantly augmented by a **Stateful Persistence Layer**, a **Local Inter-Process Communication (IPC) Interface**, and a **Registry-Based Command Engine**. This tripartite structure ensures resilience against network latency, process termination, and concurrent access attempts.

graph TD  
    subgraph "Local Workstation"  
        IDE\[Agentic IDE / CLI\] \--\>|Stdio / SSE| MCP\[monocoque-agent-rem (Rust)\]  
        IDE \-- "Calls Tool: ask\_approval(diff)" \--\> MCP  
        IDE \-- "Calls Tool: forward\_prompt(text)" \--\> MCP  
          
        MCP \-- "Polls/Listens" \--\> IPC\[Local IPC Socket / File\]  
        CLI\[monocoque-ctl\] \-.-\>|Approves| IPC  
          
        DB\[(Local State DB \- SurrealDB)\] \<--\> MCP  
        CMD\[Command Dispatcher\] \-- "Look up & Exec" \--\> Registry\[Config.toml Allowlist\]  
        MCP \-- "Routes /cmd" \--\> CMD  
        DIFF\[Diff Applicator\] \<-- "accept\_diff" \--\> MCP  
        POLICY\[Policy Evaluator\] \<-- "check\_auto\_approve" \--\> MCP  
        POLICY \-- "Watches" \--\> SETTINGS\[.monocoque/settings.json\]  
        SESSION\[Session Orchestrator\] \<--\> MCP  
        SESSION \-- "Spawns via SSE" \--\> SPAWNED\[Spawned Agent Processes\]  
    end

    subgraph "External Network"  
        MCP \-- "WSS (Socket Mode)" \--\> SlackAPI\[Slack API\]  
    end

    subgraph "Remote User"  
        SlackApp\[Slack Mobile App\] \-- "Action: Accept/Reject/Continue/Refine/Stop" \--\> SlackAPI  
    end

### **2.1 Functional Component Designation**

* **The Host (Agent):** The Integrated Development Environment (e.g., Claude Code, Cursor) or CLI tool. This component is responsible for driving the logical processes, generating code proposals, and initiating tool calls. It functions as the "brain" of the operation but possesses no inherent capability to communicate outside the local shell.  
* **The Server (Monocoque):** A stateful bridge and protocol translator. This component exposes the MCP tools and maintains a **Session Mailbox**. It is responsible for formatting MCP JSON-RPC requests into Slack Block Kit payloads, managing the WebSocket lifecycle, and persisting state to the local database to survive system restarts.  
* **The Local Controller (CLI):** A lightweight binary executable intended for local overrides. This component provides a mechanism for the operator, should they be physically present at the workstation, to intervene in the agent's operation via a secondary terminal window, bypassing the network round-trip to Slack.  
* **The Dispatcher:** A security-critical module tasked with the validation and routing of incoming slash commands. It performs a lookup within the local registry and executes the corresponding safe shell command only if strict validation criteria are met.
* **The Auto-Approve Engine:** A policy evaluation module that reads the workspace-level `.monocoque/settings.json` file and determines whether a given operation may bypass the remote approval gate. This component enforces a strict hierarchy: the global `config.toml` defines the absolute security boundary, and the workspace policy can only reduce friction within that boundary, never expand it.
* **The Session Orchestrator:** A module that manages the lifecycle of agent sessions from the remote interface. It maintains a registry of active, paused, and checkpointed sessions in the persistence layer. It accepts session-management slash commands from Slack and translates them into local IPC signals or direct database operations. This component enables the remote operator to proactively drive the local workstation rather than merely reacting to agent-initiated requests.

## **3\. Implementation Specifications regarding the Rust Programming Language**

### **3.1 Dependency Inventory (Crates)**

The project shall leverage the Rust ecosystem to ensure memory safety, type safety, and high-performance concurrency. The selection of specific crates is justified as follows:

| Crate | Purpose and Justification |
| :---- | :---- |
| mcp\_rust\_sdk | The core implementation of the Model Context Protocol (Server traits). This library abstracts the complexities of the JSON-RPC message framing and transport layers. |
| slack-mrh or slack-rust | The handling of Slack Socket Mode and the construction of Block Kit JSON. These libraries facilitate the maintenance of a persistent WebSocket connection, obviating the need for inbound firewall ports or public IP addresses. |
| tokio | The asynchronous runtime selected for the simultaneous handling of WebSocket heartbeats, MCP request loops, and IPC task polling. Its "work-stealing" scheduler ensures minimal latency. |
| interprocess or tokio-uds | Local socket/pipe communication (Local Override). This enables the implementation of the monocoque-ctl side-channel without resorting to file-based locking mechanisms. |
| surrealdb | The embedded multi-model database for the persistence of session state and configuration. SurrealDB is selected for its actively maintained ecosystem, native async/await Rust SDK, built-in query language (SurrealQL), and ability to run in embedded mode (`surreal::engine::local::RocksDb` or `surreal::engine::local::Mem`) without requiring an external server process. Its document-graph hybrid model simplifies session-to-checkpoint relationship queries. This aligns with the SurrealDB usage in the existing monocoque agent project. **Licensing note:** The SurrealDB Rust SDK is Apache-2.0 licensed; the embedded engine is Business Source License 1.1 (BSL). This is acceptable for this project because monocoque-agent-rem is distributed as standalone developer tooling (an MCP server extension), not as a library embedded into downstream products. End users install and run the binary locally for individual productivity — no user redistributes the engine or offers it as a database service. The BSL restriction (prohibiting use as a competing database product) is categorically inapplicable to this use case. |
| serde / serde\_json | The de facto standard for serialization and deserialization in Rust, essential for parsing MCP payloads and Slack API responses. |
| anyhow | Error handling. Provides a robust mechanism for propagating context-rich error messages up the stack. |
| walkdir / glob | Utilized for efficient file system traversal in list-files. walkdir is preferred for its recursive capabilities and iterator-based interface. |
| shlex | The safe parsing of command line arguments. This is critical for ensuring that strings passed to the shell are properly escaped, mitigating injection vulnerabilities. |
| notify | File system event watcher for hot-reloading the `.monocoque/settings.json` workspace policy file. Provides cross-platform support for inotify (Linux), FSEvents (macOS), and ReadDirectoryChangesW (Windows). |
| diffy | Unified diff parsing and patch application. This crate provides the core algorithm for the Diff Applicator module, enabling the `accept_diff` tool to apply standard unified diffs to existing files with conflict detection. |
| jsonschema | JSON Schema validation for the `.monocoque/settings.json` workspace policy file. Ensures the policy file conforms to the expected schema before the server loads it into the in-memory cache. |
| toml | TOML configuration file parsing for `config.toml`. Provides deserialization into strongly-typed Rust structs via serde integration. |
| tracing / tracing-subscriber | Structured, async-aware logging framework. Preferred over `log` for its support of span-based diagnostics, which align with the concurrent request lifecycle of the MCP server. |
| sha2 | SHA-256 hashing for checkpoint file integrity verification. Used by the Session Orchestrator to compute and compare workspace file hashes during checkpoint creation and restoration. |
| axum or warp | Lightweight HTTP server for the SSE transport endpoint used by spawned agent sessions (see Section 4.5, Host CLI Integration). Only one of these crates is required; `axum` is preferred for its tokio-native integration. |

### **3.2 Core Modules Description**

#### **A. The MCP Server Layer**

This module implements the mcp\_rust\_sdk::Server trait. It functions as the primary interface for the AI Agent, defining the contract of available Tools and Resources. It is responsible for decoding incoming JSON-RPC messages and routing them to the appropriate internal handler.

#### **B. The Slack Bridge Layer (Async Actor)**

This module manages the WebSocket connection to Slack using the Actor model. Because the Slack WebSocket client must run continuously to process heartbeats, while MCP tool calls are sporadic and blocking, this layer runs in a detached Tokio task. It routes incoming Slash Commands to Module E and outgoing notifications to the Slack API.

#### **C. The Session Manager (Persistence)**

This module handles Checkpoints and State Recovery. It utilizes the SurrealDB embedded database to atomically store the state of every active "Approval Request." If the server process is terminated (e.g., via a system reboot), this module reconstructs the pending request queue upon restart, preventing data loss.

#### **D. The Local Control Layer**

This module listens on a Unix Domain Socket (or Named Pipe on Windows) for local overrides. It creates a secondary control plane, allowing monocoque-ctl to inject "Approve" or "Reject" signals directly into the pending request map.

#### **E. Registry Command Dispatcher**

This module parses incoming Slack text and maps said text to safe, pre-defined shell commands. It enforces a strict "deny-by-default" policy, ensuring that only commands explicitly whitelisted in the configuration file can be executed.

#### **F. The Diff Applicator**

This module is responsible for the safe, atomic application of approved diffs to the local file system. It implements two strategies: full-file write (for new files or complete replacements) and unified diff patch application (for incremental modifications). The module validates path boundaries, applies patches with conflict detection, and maintains idempotency by tracking consumed approval records. It also performs pre-write file integrity checks to detect local modifications that occurred after the proposal was created.

#### **G. The Workspace Policy Evaluator**

This module discovers and parses the `.monocoque/settings.json` file within the workspace root. On server startup and upon file system change notification, it loads the auto-approve rules into an in-memory policy cache. It exposes a query interface consumed by the MCP Server Layer and the Dispatcher to short-circuit approval gates for matching operations. The evaluator enforces the precedence hierarchy: global `config.toml` boundaries supersede workspace policy, and runtime mode overrides supersede workspace policy.

#### **H. The Session Orchestrator**

This module manages the full lifecycle of agent sessions from the remote Slack interface. It maintains a session registry in the SurrealDB embedded database, tracking each session's state (active, paused, checkpointed, terminated), associated prompt/instruction, creation timestamp, and checkpoint history. The module exposes session-management slash commands to the Dispatcher and translates them into actionable local operations: spawning new agent processes via the configured host CLI, sending pause/resume signals through the IPC layer, serializing session state for checkpoints, and restoring prior states. It also provides the enhanced `/monocoque help` command, which introspects all registered commands (built-in, custom registry, and session management) and renders a categorized Block Kit response.

## **4\. Exposed Capabilities within the Model Context Protocol**

### **4.1 Functional Instrumentation (Tools)**

#### **ask\_approval (The "Diff & Accept" Engine)**

* **Description:** The central mechanism for the "Shadow Agent" workflow. It suspends the execution of the Agent and presents a **Code Proposal** to the operator. This tool is designed to emulate the "review" phase of a code contribution workflow.  
* **Operational Logic:**  
  1. The Agent generates a code modification (e.g., creating a new auth.rs file or refactoring a function).  
  2. The Agent invokes the ask\_approval tool, passing the generated diff content as a parameter.  
  3. The Monocoque server analyzes the size of the diff.  
     * **Small Diffs (\< 20 lines):** These are rendered directly into the chat interface as an inline Slack Markdown code block. This prioritizes immediate visibility for trivial changes.  
     * **Large Diffs (\> 20 lines):** These are uploaded to Slack as a **Slack Snippet** (File object) with diff syntax highlighting enabled. This facilitates the expansion/collapse and scrolling of extensive changes on mobile devices without cluttering the chat history or hitting message character limits.  
  4. Monocoque constructs a Block Kit message containing "✅ Accept Changes" and "❌ Reject" buttons and posts it to the channel.  
  5. The system enters a wait state.  
  6. The Operator reviews the proposal and selects "Accept Changes".  
  7. Monocoque receives the interaction event, resolves the wait state, and returns "Approved" to the Agent.  
  8. The Agent proceeds to write the file to the disk.  
* **Parameters:**  
  * title (string): A concise summary, e.g., "Create Auth Middleware".
  * description (string): Contextual details, e.g., "Implements JWT validation logic using the jsonwebtoken crate."
  * diff (string): The standard unified diff or raw file content proposed by the agent.
  * file\_path (string, required): The target file path relative to the `workspace_root` where the changes should be applied. For multi-file diffs, this is the primary file; additional paths are extracted from the unified diff headers.
  * risk\_level (string): low | high | critical. High risk levels may trigger additional alerting mechanisms, such as @channel mentions.
* **Return Value:**
  * `{ "status": "approved", "request_id": "<unique_id>" }` — The operator approved the proposal. The `request_id` is a unique identifier that must be passed to `accept_diff` to apply the changes.
  * `{ "status": "rejected", "request_id": "<unique_id>", "reason": "<optional rejection note>" }` — The operator rejected the proposal. The agent should not proceed with the file write and should adjust its approach or report the rejection via `remote_log`.
  * `{ "status": "timeout", "request_id": "<unique_id>" }` — The approval request expired without a response (see timeout behavior below).
* **Timeout Behavior:** If no response is received within `approval_timeout_seconds` (configured in `config.toml`, default: 3600 seconds / 1 hour), the server marks the request as `expired`, posts a notification to Slack, and returns the `timeout` status to the Agent. The Agent should treat a timeout as a soft rejection and invoke `wait_for_instruction` or retry.
#### **set\_operational\_mode**

Switches the server between Remote, Local, and Hybrid modes. This allows the operator to dynamically adjust the behavior of the approval gates based on their physical location (e.g., disabling Slack notifications when sitting at the desk).

* **Parameters:**
  * mode (string, required): The target operational mode. Supported values:
    * `remote` — All approval requests are forwarded to Slack. Local IPC overrides are disabled. Auto-approve policies in `.monocoque/settings.json` are disabled.
    * `local` — Approval requests are routed exclusively to the local IPC channel (monocoque-ctl). Slack notifications are suppressed. The server operates silently from the remote operator's perspective.
    * `hybrid` — Approval requests are forwarded to both Slack and the local IPC channel. The first response (from either channel) is accepted. Auto-approve policies are active.
* **Return Value:** `{ "previous_mode": "<mode>", "current_mode": "<mode>" }`
* **Side Effects:** A mode change is logged to the Slack channel (unless switching to `local`) and persisted to the SurrealDB embedded database so it survives restarts.

#### **wait\_for\_instruction**

Places the system in a standby loop. The agent will pause execution and poll for a "Resume" signal or a new command payload from the Slack interface. This is essential for session continuity across long pauses.

* **Parameters:**
  * message (string, optional): A status message to display in Slack while waiting, e.g., "Awaiting next task." Defaults to "Agent is idle and awaiting instructions."
  * timeout\_seconds (number, optional, default: 0): Maximum time to wait before returning a timeout status. A value of 0 means wait indefinitely.
* **Return Value:**
  * `{ "status": "resumed", "instruction": "<new prompt text>" }` — The operator provided a new instruction via Slack.
  * `{ "status": "resumed", "instruction": null }` — The operator sent a bare "Resume" signal without a new instruction (the agent should continue its previous task).
  * `{ "status": "timeout" }` — The wait period expired without operator input.
* **Slack Rendering:** The server posts a message: "💤 **Agent idle** — *<message>*. Reply in this channel or use `/monocoque session-resume` to continue." The message is updated to "▶️ **Resumed**" when the operator responds.

#### **recover\_state**

Retrieves the last known checkpoint from the persistent database. This tool is called by the Agent upon startup to check if there was a pending approval request that was interrupted by a crash or timeout.

* **Parameters:**
  * session\_id (string, optional): The specific session to recover. When omitted, the server returns the most recently active session's state.
* **Return Value:**
  * `{ "status": "recovered", "session_id": "<id>", "pending_requests": [ { "request_id": "<id>", "type": "approval|prompt", "title": "<title>", "created_at": "<ISO 8601>" } ], "last_checkpoint": { "checkpoint_id": "<id>", "label": "<label>", "created_at": "<ISO 8601>" } | null }` — One or more pending items were recovered.
  * `{ "status": "clean", "session_id": null }` — No pending state was found. The server is starting fresh.

#### **remote\_log**

Transmits status updates and logging information to the Slack channel without blocking execution. This is utilized to keep the remote operator informed of progress (e.g., "Running tests...", "Build completed").

* **Parameters:**
  * message (string, required): The log message to post.
  * level (string, optional, default: "info"): The log level, which controls the visual presentation in Slack. Supported values:
    * `info` — Standard informational message. Rendered as plain text.
    * `success` — Positive outcome. Prefixed with ✅.
    * `warning` — Non-blocking issue. Prefixed with ⚠️. Rendered in a yellow-tinted context block.
    * `error` — Error condition. Prefixed with ❌. Rendered in a red-tinted section block.
  * thread\_ts (string, optional): The Slack thread timestamp to post the log as a reply. When omitted, the log is posted as a top-level message.
* **Return Value:** `{ "posted": true, "ts": "<message timestamp>" }` — Confirmation that the message was posted, with its timestamp for threading.

#### **accept\_diff (The "Keep Changes" Engine)**

* **Description:** The programmatic complement to `ask_approval`. Whereas `ask_approval` presents a proposal and blocks until a human decision is received, `accept_diff` is invoked *after* approval has been granted and instructs the Monocoque server to apply the approved changes directly to the file system. This eliminates the manual "Keep" or "Accept" step that is otherwise required in the host IDE's diff viewer UI. The tool enables full end-to-end remote orchestration: the operator approves via Slack, and the file is written without any local UI interaction.
* **Operational Logic:**
  1. A prior `ask_approval` call has completed with a status of `approved`, and the approval record (including its `request_id` and the associated diff content) is stored in the Session Manager.
  2. The Agent invokes `accept_diff`, providing the `request_id` of the approved proposal.
  3. Monocoque retrieves the original diff and target file path(s) from the Session Manager.
  4. **Validation Gate:** The server verifies that (a) the `request_id` references a valid, approved proposal; (b) the approval has not already been consumed (idempotency guard); and (c) all target paths resolve within the configured `workspace_root` (path traversal protection).
  5. **Diff Application:** The server applies the changes to disk. Two modes are supported:
     * **Full-file write:** When the diff content represents a complete new file, the server writes the content directly to the target path, creating intermediate directories as required.
     * **Patch application:** When the diff content is a standard unified diff, the server applies the patch to the existing file using an embedded patch algorithm. If the patch fails to apply cleanly (e.g., due to a conflicting local edit), the tool returns an error with the specific hunk failures.
  6. The server marks the approval record as `consumed` in the database to prevent duplicate application.
  7. The server posts a confirmation message to the Slack thread: "✅ Changes applied to `<file_path>`."
  8. The tool returns a success payload to the Agent, including the list of files written and their byte sizes.
* **Parameters:**
  * request\_id (string, required): The unique identifier of the approved proposal returned by `ask_approval`.
  * force (boolean, optional, default: false): When true, overwrites the target file even if the local content has diverged since the proposal was created. This is intended for recovery scenarios and logs a warning to Slack.
* **Error Conditions:**
  * `request_not_found`: The `request_id` does not correspond to any known proposal.
  * `not_approved`: The referenced proposal has not yet been approved or was rejected.
  * `already_consumed`: The approved diff has already been applied.
  * `path_violation`: A target path resolves outside the `workspace_root`.
  * `patch_conflict`: The unified diff could not be applied cleanly and `force` is false.

#### **check\_auto\_approve**

* **Description:** Queries the Workspace Auto-Approve Policy to determine whether a given tool invocation or command should bypass the approval gate. The Agent or server internals may call this tool prior to `ask_approval` to short-circuit the remote approval round-trip when the workspace configuration permits it.
* **Parameters:**
  * tool\_name (string, required): The name of the tool or command to check (e.g., "write_file", "cargo test").
  * context (object, optional): Additional metadata such as the target file path or risk level, enabling fine-grained policy evaluation.
* **Return Value:** `{ "auto_approved": true | false, "matched_rule": "<rule_key>" | null }`.

#### **forward\_prompt (The "Continuation Gate" Engine)**

* **Description:** Intercepts and relays agent-generated meta-prompts to the remote Slack interface. AI agents periodically emit continuation prompts when they have been executing for an extended period, when they encounter ambiguity, or when they require user input to proceed. These prompts are not code proposals (handled by `ask_approval`) but operational control questions that would otherwise block the local terminal indefinitely. Common examples include:
  * GitHub Copilot: "Copilot has been working on this problem for a while. It can continue to iterate, or you can send a new message to refine your prompt."
  * Claude Code: "I've been working on this for a while. Would you like me to continue, or would you like to give me additional guidance?"
  * General agents: "The task is taking longer than expected. Continue?"

  The tool forwards these prompts to Slack with a standardized set of response actions, then blocks until the remote operator responds.

* **Operational Logic:**
  1. The Agent encounters a continuation checkpoint in its execution loop. Rather than presenting it to the local terminal, the Agent invokes `forward_prompt` with the prompt text.
  2. Monocoque generates a unique `prompt_id` and persists the pending prompt in the Session Manager database, associated with the current session.
  3. Monocoque constructs a Block Kit message with the prompt text, the session context (ID, elapsed time, actions taken so far), and a set of response buttons.
  4. The message is posted to the Slack channel. If the session's `risk_level` is `high` or `critical`, an `@channel` mention is prepended to ensure visibility.
  5. The system enters a wait state, identical in mechanism to `ask_approval`.
  6. The Operator selects one of the response actions:
     * **▶️ Continue:** The agent resumes execution with no changes to its instruction.
     * **✏️ Refine:** The operator provides a revised or supplementary instruction via a Slack modal dialog. The new instruction text is returned to the agent as the response payload.
     * **🛑 Stop:** The agent is instructed to terminate its current task gracefully.
  7. Monocoque receives the interaction event, resolves the wait state, and returns the operator's decision to the Agent.
  8. The Agent acts on the response: continuing, adjusting its approach based on the refined prompt, or halting.

* **Parameters:**
  * prompt\_text (string, required): The raw text of the continuation prompt emitted by the agent.
  * prompt\_type (string, optional, default: "continuation"): Categorizes the prompt to enable tailored rendering. Supported values:
    * `continuation` — The agent has been working for a while and is asking whether to proceed.
    * `clarification` — The agent has encountered ambiguity and needs user guidance.
    * `error_recovery` — The agent has encountered a non-fatal error and is asking how to proceed.
    * `resource_warning` — The agent is warning about resource consumption (tokens, time, API calls).
  * elapsed\_seconds (number, optional): The number of seconds the agent has been executing since the last user interaction. Rendered in the Slack message for context.
  * actions\_taken (number, optional): The count of actions (tool calls, file writes, commands) the agent has performed in this iteration. Provides the operator with a sense of progress.

* **Timeout Behavior:** If no response is received within `prompt_timeout_seconds` (configured in `config.toml`, default: 1800 seconds / 30 minutes), the server auto-responds with `{"decision": "continue"}`, posts a notification to Slack ("⏰ Continuation prompt auto-continued after timeout"), and logs the event. This prevents indefinite blocking when the operator is unavailable. The timeout duration is configurable per `prompt_type`.

* **Return Value:**
  * `{ "decision": "continue" }` — The operator chose to let the agent keep running.
  * `{ "decision": "refine", "instruction": "<new prompt text>" }` — The operator provided a revised instruction.
  * `{ "decision": "stop" }` — The operator chose to halt the agent.

* **Auto-Approve Behavior:** The `.monocoque/settings.json` workspace policy may include `forward_prompt` in the `autoApprove.tools` array. When auto-approved, continuation prompts are automatically answered with `"continue"` and a log entry is posted to Slack. This is useful for long-running automated pipelines where the operator has pre-authorized extended execution. Prompts of type `error_recovery` are never auto-approved regardless of policy.

### **4.2 Resource Access (Context Reading)**

* **slack://channel/{id}/recent:** Reads the recent chat history from the configured channel. This allows the Agent to "read" instructions or feedback provided by the user in the chat thread, effectively treating the chat log as a dynamic context source.
  * **Parameters:**
    * id (string, required): The Slack channel ID (e.g., `C0123456789`). Must match the `channel_id` configured in `config.toml`.
    * limit (number, optional, default: 20): Maximum number of messages to retrieve (1–100).
  * **Return Schema:**

    ```json
    {
      "messages": [
        {
          "ts": "1706284800.000100",
          "user": "U0123456789",
          "text": "Focus on the API layer next.",
          "thread_ts": null
        }
      ],
      "has_more": false
    }
    ```

  * **Security:** Only messages from the configured `channel_id` are returned. Messages from other channels are not accessible. Bot messages are included; file uploads are represented by their text summary only.

### **4.3 Direct Remote Commands (Registry Only)**

(Utilized by the Human via Slack.)

To mitigate security risks, this feature is **Strictly Registry-Based**. It does not allow arbitrary shell execution.

#### **Built-in Commands (Always Available)**

**File Operations**

| Command | Arguments | Description |
| :---- | :---- | :---- |
| list-files | \[path\] \[--depth N\] | Lists files and directories in the workspace. Strictly restricted to the workspace\_root. Returns a formatted directory tree using Unicode box-drawing characters (├── , └── , │). Default depth is 3 levels. The optional `--depth` flag controls recursion depth (1–10). **Rendering:** Small trees (≤ 40 lines) are rendered inline as a Slack Markdown code block. Large trees (> 40 lines) are uploaded as a `.txt` Slack Snippet for collapsible viewing. The response header includes the resolved path and total file/directory count. |
| show-file | \<path\> \[--lines START:END\] | Reads a file and returns its contents with syntax highlighting. Strictly restricted to the workspace\_root to prevent exfiltration of system files. The optional `--lines` flag restricts output to a line range (e.g., `--lines 10:50`). **Rendering:** Small files (≤ 30 lines or ≤ 2 KB) are rendered inline as a Slack Markdown code block with the language identifier inferred from the file extension (e.g., ` ```rust ` for `.rs` files). Large files (> 30 lines or > 2 KB) are uploaded as a Slack Snippet (File object) with the appropriate `filetype` parameter set for syntax highlighting. Binary files are rejected with an error message. The response header includes the file path, size, last modified timestamp, and line count. |

**Command Discovery**

| Command | Arguments | Description |
| :---- | :---- | :---- |
| help | \[category\] | Returns a richly formatted, categorized listing of all available slash commands. When invoked without arguments, it returns the complete command catalog grouped into categories (File Operations, Session Management, Custom Commands). When invoked with a category name, it returns only commands in that category with extended descriptions and usage examples. The output is rendered as a Slack Block Kit message with section headers and code blocks for syntax reference. |

**Session Management**

| Command | Arguments | Description |
| :---- | :---- | :---- |
| sessions | None | Lists all tracked sessions with their current state (active, paused, checkpointed, terminated), creation timestamp, and last activity. Output is formatted as a table with session IDs truncated for readability. |
| session-start | \<prompt\> | Initiates a new agent session on the local workstation. The server spawns the configured host CLI (e.g., Claude Code, GitHub Copilot CLI) with the provided prompt as the initial instruction. The session is registered in the persistence layer and assigned a unique session ID, which is returned to the operator. |
| session-clear | \[session\_id\] | Terminates and cleans up a session. When invoked without a session ID, it targets the currently active session. The server sends a termination signal via IPC, removes pending approval requests associated with the session, and marks the session as terminated in the database. Does not delete checkpoint history. |
| session-pause | \[session\_id\] | Pauses a running session. The server sends a suspend signal via IPC, causing the agent to enter a wait state. The session state is persisted so that it can be resumed later. When invoked without a session ID, it targets the currently active session. |
| session-resume | \[session\_id\] | Resumes a previously paused session. The server sends a resume signal via IPC, causing the agent to continue from where it was suspended. When invoked without a session ID, it targets the most recently paused session. |
| session-checkpoint | \[session\_id\] \[label\] | Creates a named checkpoint of the current session state, including pending approval requests, workspace file hashes, and agent context. The checkpoint is stored in the SurrealDB embedded database and can be restored later. The optional label provides a human-readable name (e.g., "before-refactor"). Returns the checkpoint ID. |
| session-restore | \\<checkpoint\\_id\\> | Restores a previously checkpointed session. The server terminates any currently active session, reconstructs the session state from the checkpoint, and signals the agent to resume from the checkpointed context. The operator is warned if workspace files have diverged since the checkpoint was created. |\n| session-checkpoints | \\[session\\_id\\] | Lists all checkpoints. When invoked with a session ID, returns only checkpoints for that session. Output includes checkpoint ID, label, creation timestamp, and a summary of file hashes stored. Without a session ID, returns all checkpoints across all sessions, sorted by creation time (most recent first). |

#### **Custom Registry Commands**

Commands are triggered via the slash command syntax /monocoque \<alias\>.

* **Example:** The Operator types /monocoque status.  
* **Server Action:** The system parses the command, looks up the key status in the config.toml file, locates the corresponding value git status, and executes git status in the shell. The standard output is captured and posted back to Slack.

### **4.4 Workspace Auto-Approve Policy**

The Workspace Auto-Approve Policy is a per-workspace configuration mechanism that permits the declaration of tool invocations and commands exempt from the remote approval gate. This eliminates unnecessary round-trips to Slack for operations the workspace owner has predetermined to be safe.

#### **Design Rationale**

In existing IDE workflows, VS Code provides the `chat.tools.terminal.autoApprove` setting within `.vscode/settings.json` to allow certain terminal commands to execute without user confirmation. However, this mechanism is tightly coupled to VS Code and is invisible to other MCP hosts such as GitHub Copilot CLI, OpenAI Codex, Claude Code CLI, or Cursor. The Workspace Auto-Approve Policy elevates this concept to the MCP server layer, making it IDE-agnostic. Any MCP-compatible host benefits from the same auto-approve rules without configuration duplication.

#### **Configuration File Location**

The policy is defined in a `.monocoque/settings.json` file located at the root of the workspace (adjacent to `.vscode/`, `.git/`, etc.). This mirrors the established convention of dot-directory configuration files.

#### **Schema Definition**

```json
{
  "$schema": "https://monocoque.dev/schemas/settings.v1.json",
  "autoApprove": {
    "enabled": true,
    "commands": [
      "git status",
      "git diff",
      "git log *",
      "cargo test *",
      "cargo clippy *",
      "npm run lint",
      "npm run test"
    ],
    "tools": [
      "read_file",
      "list_files",
      "remote_log",
      "check_auto_approve"
    ],
    "filePatterns": {
      "write": [
        "src/tests/**",
        "tests/**",
        "*.test.ts",
        "*.spec.rs"
      ],
      "read": ["**"]
    },
    "riskLevelThreshold": "low"
  },
  "notifications": {
    "logAutoApproved": true,
    "summaryIntervalSeconds": 300
  }
}
```

#### **Field Definitions**

| Field | Type | Description |
| :---- | :---- | :---- |
| autoApprove.enabled | boolean | Master switch. When false, all operations require explicit approval regardless of other settings. |
| autoApprove.commands | string\[\] | Shell commands that bypass approval. Glob wildcards (`*`) are permitted for argument matching. The command must still exist in the global `config.toml` allowlist or be a built-in command; this field cannot introduce new commands. |
| autoApprove.tools | string\[\] | MCP tool names that bypass the `ask_approval` gate. Read-only tools are typical candidates. |
| autoApprove.filePatterns.write | string\[\] | Glob patterns for file paths where `accept_diff` may auto-apply without remote approval. Restricted to paths within the `workspace_root`. |
| autoApprove.filePatterns.read | string\[\] | Glob patterns for file paths where `show-file` may auto-serve content. |
| autoApprove.riskLevelThreshold | string | The maximum risk level (low, high, critical) at which auto-approval applies. Operations tagged above this threshold always require explicit approval. |
| notifications.logAutoApproved | boolean | When true, auto-approved actions are logged to the Slack channel as informational messages (non-blocking). Provides an audit trail. |
| notifications.summaryIntervalSeconds | number | Interval in seconds at which the server batches and posts a summary of auto-approved actions to Slack. Set to 0 to disable batching (post each action individually). |

#### **Precedence and Override Rules**

1. The global `config.toml` defines the absolute security boundary. A command not present in `[remote_commands]` or the built-in list cannot be auto-approved regardless of the workspace settings.
2. When `autoApprove.enabled` is false, all workspace auto-approve rules are ignored.
3. Operations with a `risk_level` exceeding `riskLevelThreshold` always require explicit approval.
4. The operator may override the workspace policy at runtime via the `set_operational_mode` tool. Setting the mode to `Remote` disables auto-approve and forces all operations through Slack.

### **4.5 Remote Session Orchestration**

The Remote Session Orchestration capability transforms the remote operator from a passive reviewer into an active driver of the local workstation. In prior versions, the remote operator could only respond to requests initiated by a locally running agent. This section specifies the mechanism by which the operator may proactively initiate, manage, and navigate between agent sessions entirely from the Slack interface.

#### **Design Rationale**

Agentic workflows are typically linear: an agent starts, performs work, requests approvals, and terminates. However, real-world usage patterns demand non-linear control flow. An operator may need to:

* Start a new task while a current task is paused (context switching).
* Roll back to a known-good state after a failed experiment (checkpoint restoration).
* Terminate a runaway agent that is consuming resources (session clearing).
* Inspect what sessions are active before issuing further instructions (session discovery).

Without this capability, the operator must physically access the local workstation to perform any of these actions, defeating the purpose of the remote workflow.

#### **Session State Machine**

Each session progresses through a well-defined set of states:

```
[Created] --> [Active] --> [Paused] --> [Active]  (resume)
                |              |
                |              v
                |        [Terminated]
                |
                v
          [Terminated]

[Active]  -- checkpoint --> [Active]   (snapshot, session continues)
[Paused]  -- checkpoint --> [Paused]   (snapshot, session remains paused)
[Terminated] .............. [Checkpointed data retained for restore]
```

* **Created:** The session has been registered but the agent process has not yet started.
* **Active:** The agent process is running and the session is accepting tool calls. Checkpoints can be created from this state without interrupting execution.
* **Paused:** The agent process has been suspended. No tool calls are processed. The session retains its position and can be resumed. Checkpoints can also be created from this state.
* **Checkpointed:** A snapshot of the session state has been persisted. This is not a distinct state but rather a property: a session in any state (active, paused, or terminated) may have one or more associated checkpoints. Checkpoints are stored independently and survive session termination.
* **Terminated:** The session has been cleaned up. The agent process has been stopped. Checkpoint history is retained for potential restoration.
* **Error:** An abnormal termination caused by an agent crash or unrecoverable error. The session transitions to `Terminated` and the error details are logged. Pending approval requests are marked as `orphaned`.

#### **Host CLI Integration**

The `session-start` command requires the server to spawn a new agent process. The host CLI binary and its arguments are configured in the global `config.toml`:

```toml
[session]
host_cli = "claude"                    # The binary used to spawn agent sessions.
host_args = ["--mcp", "monocoque"]     # Default arguments appended to every session start.
max_concurrent_sessions = 3            # Maximum number of simultaneously active sessions.
checkpoint_retention_days = 30         # Number of days to retain checkpoint data before pruning.
```

When the operator types `/monocoque session-start Refactor the auth module to use OAuth2`, the server executes the equivalent of:

```sh
claude --mcp monocoque "Refactor the auth module to use OAuth2"
```

The spawned process connects to the already-running Monocoque server via **Server-Sent Events (SSE) over HTTP** on the configured `server.sse_port` (default: `3100`). The primary host agent connects via stdio (the default MCP transport), but additional spawned sessions cannot share the same stdio pipe. Therefore, Monocoque exposes a secondary SSE transport endpoint alongside the stdio transport. The `host_args` configuration should include the SSE endpoint URL so that the spawned agent discovers the server automatically.

#### **Command Discovery Response Format**

The `/monocoque help` command returns a Block Kit message with the following structure:

```json
[
  {
    "type": "header",
    "text": { "type": "plain_text", "text": "📖 Monocoque Command Reference" }
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "*📁 File Operations*\n`/monocoque list-files [path]` — List workspace files\n`/monocoque show-file <path>` — View file contents"
    }
  },
  {
    "type": "divider"
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "*🔄 Session Management*\n`/monocoque sessions` — List all sessions\n`/monocoque session-start <prompt>` — Start new agent session\n`/monocoque session-clear [id]` — Terminate a session\n`/monocoque session-pause [id]` — Pause a session\n`/monocoque session-resume [id]` — Resume a session\n`/monocoque session-checkpoint [id] [label]` — Save checkpoint\n`/monocoque session-checkpoints [id]` — List checkpoints\n`/monocoque session-restore <checkpoint_id>` — Restore checkpoint"
    }
  },
  {
    "type": "divider"
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "*⚙️ Custom Commands*\n`/monocoque status` — git status\n`/monocoque last-commit` — git log -1 --oneline\n`/monocoque test` — cargo test -- --nocapture\n`/monocoque deploy` — ./scripts/deploy_staging.sh"
    }
  },
  {
    "type": "context",
    "elements": [
      {
        "type": "mrkdwn",
        "text": "💡 Type `/monocoque help <category>` for detailed usage. Categories: `files`, `sessions`, `custom`"
      }
    ]
  }
]
```

The Custom Commands section is dynamically generated from the `[remote_commands]` table in `config.toml`. The listing renders each alias alongside its underlying shell command for transparency.

## **5\. Protocols of Interaction**

### **5.1 The "Code Acceptance" Workflow**

The following sequence describes the data flow for a successful code generation and acceptance cycle:

1. **Agent (Local):** The Agent determines a course of action: "I have generated the user login component. It is required that it be written to src/components/Login.tsx."  
2. **Agent (Local):** The Agent initiates a tool call: ask\_approval(diff="... \+export const Login \= ...", title="Create Login Component").  
3. **Monocoque:** The server intercepts the call. It evaluates the length of the diff string. It is determined to be 50 lines, exceeding the inline threshold.  
4. **Slack:**  
   * **Bot:** The server posts a header message: "⚠️ **Proposal: Create Login Component**".  
   * **Attachment:** The server uploads the code as a diff.patch snippet, ensuring it is collapsible.  
   * **Buttons:** The server appends an interactive block with \[ ✅ Accept Changes \] and \[ ❌ Reject \].  
5. **User (Remote):** The operator receives a notification. They tap the snippet and review the code on a mobile device. The code is deemed acceptable.  
6. **User (Remote):** The operator taps the \[ ✅ Accept Changes \] button.  
7. **Monocoque:** The server receives the WebSocket event, correlates it with the pending request ID, and sends {"status": "approved", "request_id": "req-abc-123"} to the Agent.  
8. **Agent (Local):** The Agent receives the approval and executes the file write operation to disk.  
9. **Agent (Local):** The Agent calls remote\_log("File created successfully.") to close the loop.

#### **5.1.1 The "Rejection" Path**

If the operator selects ❌ Reject in step 6:

6b. **User (Remote):** The operator taps the \[ ❌ Reject \] button. Optionally, a Slack modal dialog opens requesting a rejection reason.
7b. **Monocoque:** The server receives the rejection event and sends `{"status": "rejected", "request_id": "req-abc-123", "reason": "<optional note>"}` to the Agent.
8b. **Slack:** The original message buttons are replaced with "❌ **Rejected** — *<reason>*" to prevent double-submission.
9b. **Agent (Local):** The Agent receives the rejection and should either adjust its approach based on the reason, invoke `wait_for_instruction` to ask for guidance, or proceed to the next task. The Agent must not write the rejected changes to disk.

### **5.2 The "Programmatic Keep" Workflow**

The following sequence describes the data flow when the server applies approved changes to disk on behalf of the agent, bypassing the host IDE's manual "Keep" interaction:

1. **Agent (Local):** Steps 1–2 of the Code Acceptance Workflow (Section 5.1) have completed. The Agent has received `{"status": "approved", "request_id": "req-abc-123"}` from `ask_approval`.
2. **Agent (Local):** The Agent invokes `accept_diff(request_id="req-abc-123")`.
3. **Monocoque:** The server retrieves the proposal record from the Session Manager database. It confirms the proposal status is `approved` and has not been previously consumed.
4. **Monocoque:** The server resolves the canonical target path and verifies it resides within `workspace_root`.
5. **Monocoque (Patch Mode):** If the diff is a unified diff, the server reads the current file from disk, applies the patch hunks, and writes the result. If the diff is a full file, the server writes the content directly.
6. **Monocoque:** The server atomically marks the proposal as `consumed` in the database.
7. **Slack:** The server posts: "✅ Changes applied to `src/components/Login.tsx` (1,247 bytes written)."
8. **Monocoque:** The tool returns `{"status": "applied", "files": [{"path": "src/components/Login.tsx", "bytes": 1247}]}` to the Agent.
9. **Agent (Local):** The Agent logs success and proceeds to the next task without requiring any manual UI interaction.

### **5.3 The "Auto-Approve" Workflow**

The following sequence describes the data flow when a workspace auto-approve policy permits an operation to bypass the remote approval gate:

1. **Agent (Local):** The Agent intends to run `cargo test -- --nocapture`.
2. **Agent (Local):** The Agent invokes `check_auto_approve(tool_name="cargo test", context={"args": "-- --nocapture"})`.
3. **Monocoque:** The server loads `.monocoque/settings.json` from the workspace root. It evaluates the `autoApprove.commands` array and finds the pattern `cargo test *` matches the requested command.
4. **Monocoque:** The server verifies that `autoApprove.enabled` is true and that the operation's risk level does not exceed the `riskLevelThreshold`.
5. **Monocoque:** The tool returns `{"auto_approved": true, "matched_rule": "cargo test *"}` to the Agent.
6. **Agent (Local):** The Agent skips the `ask_approval` call and executes the command directly.
7. **Monocoque (Audit):** If `notifications.logAutoApproved` is true, the server posts an informational message to Slack: "\[Auto-Approved\] `cargo test -- --nocapture` (matched rule: `cargo test *`)."

### **5.4 The "Remote Command Discovery & Execution" Workflow**

The following sequence describes the data flow when the remote operator discovers available commands and initiates a new agent session from Slack:

1. **User (Remote):** The operator types `/monocoque help` in the Slack channel.
2. **Slack:** The Slack API routes the slash command payload to the Monocoque server via the WebSocket connection.
3. **Monocoque (Dispatcher):** The server parses the command token `help` and routes it to the Session Orchestrator module.
4. **Monocoque (Session Orchestrator):** The module introspects the command registry: it enumerates the built-in file commands, the session management commands, and the custom commands from `config.toml`. It constructs a categorized Block Kit payload (as specified in Section 4.5).
5. **Slack:** The server posts the formatted command reference as an ephemeral message visible only to the requesting operator.
6. **User (Remote):** The operator reviews the listing and decides to start a new session. They type `/monocoque session-start Implement rate limiting middleware for the API gateway`.
7. **Slack:** The Slack API routes the command payload to Monocoque.
8. **Monocoque (Dispatcher):** The server parses the command token `session-start` and extracts the prompt string.
9. **Monocoque (Session Orchestrator):** The module verifies that the `max_concurrent_sessions` limit has not been reached. It creates a new session record in the SurrealDB embedded database with state `Created`. It spawns the configured host CLI process with the prompt as the initial instruction. The session state transitions to `Active`.
10. **Slack:** The server posts a confirmation: "🚀 **Session started** (`session-id: ses-xyz-789`). Agent is processing: *Implement rate limiting middleware for the API gateway*." The message includes a \[ 🛑 Stop \] button for quick termination.
11. **Agent (Local):** The newly spawned agent begins executing and uses the MCP tools (e.g., `ask_approval`, `remote_log`) as normal. The remote operator can interact with the new session's approval requests immediately.

### **5.5 The "Session Checkpoint & Restore" Workflow**

The following sequence describes the data flow for checkpointing a session and restoring it later:

1. **User (Remote):** The operator types `/monocoque session-checkpoint ses-xyz-789 before-refactor`.
2. **Monocoque (Session Orchestrator):** The module locates session `ses-xyz-789` in the database. It serializes the session state: pending approval requests, agent context metadata, and a manifest of workspace file hashes (SHA-256) for divergence detection. The snapshot is written to the SurrealDB embedded database with a unique checkpoint ID and the label "before-refactor".
3. **Slack:** The server posts: "📌 **Checkpoint created** (`chk-abc-456`: *before-refactor*) for session `ses-xyz-789`." The session continues running uninterrupted.
4. *(Time passes. The agent performs further work that the operator wants to undo.)*
5. **User (Remote):** The operator types `/monocoque session-restore chk-abc-456`.
6. **Monocoque (Session Orchestrator):** The module terminates the currently active session (if any). It retrieves the checkpoint data from the database. It compares the stored file hashes against the current workspace files.
7. **Slack:** If files have diverged, the server posts a warning: "⚠️ The following files have changed since checkpoint *before-refactor*: `src/middleware/rate_limit.rs`, `Cargo.toml`. Proceeding will override local state." The message includes \[ ✅ Proceed \] and \[ ❌ Cancel \] buttons.
8. **User (Remote):** The operator taps \[ ✅ Proceed \].
9. **Monocoque (Session Orchestrator):** The module reconstructs the session state, re-creates the pending approval queue, and spawns a new agent process initialized with the checkpointed context.
10. **Slack:** The server posts: "♻️ **Session restored** from checkpoint *before-refactor*. Agent is resuming."

### **5.6 The "Continuation Prompt Forwarding" Workflow**

The following sequence describes the data flow when an agent emits a continuation prompt and the remote operator responds:

1. **Agent (Local):** The agent has been executing for 12 minutes and has performed 47 tool calls. The host IDE/CLI emits a continuation checkpoint: "Copilot has been working on this problem for a while. It can continue to iterate, or you can send a new message to refine your prompt."
2. **Agent (Local):** Rather than blocking the local terminal, the agent invokes `forward_prompt(prompt_text="Copilot has been working on this problem for a while...", prompt_type="continuation", elapsed_seconds=720, actions_taken=47)`.
3. **Monocoque:** The server generates `prompt_id: prm-def-321` and persists the pending prompt in the Session Manager, linked to the active session `ses-xyz-789`.
4. **Slack:** The server posts a Block Kit message (as specified in Section 6) with the prompt text, session context, and three action buttons: \[ ▶️ Continue \], \[ ✏️ Refine \], and \[ 🛑 Stop \].
5. **User (Remote):** The operator receives the notification on their mobile device. They review the elapsed time and action count. They decide the agent should continue but with narrower focus.
6. **User (Remote):** The operator taps \[ ✏️ Refine \]. A Slack modal dialog opens with a text input field pre-populated with the original task description.
7. **User (Remote):** The operator edits the instruction to: "Focus only on the authentication module. Skip the user profile refactor for now." They tap "Submit".
8. **Monocoque:** The server receives the modal submission event, correlates it with `prm-def-321`, and returns `{"decision": "refine", "instruction": "Focus only on the authentication module. Skip the user profile refactor for now."}` to the Agent.
9. **Slack:** The server updates the original message to show: "✏️ **Refined** — Operator provided updated instruction" (replacing the buttons to prevent double-submission).
10. **Agent (Local):** The Agent receives the refined instruction and adjusts its execution accordingly, continuing from its current state with the narrowed scope.

## **6\. Interface Layout Specifications for Slack**

The Rust server shall dynamically construct JSON blocks based on content size and context.

**Scenario: Large Diff Proposal**

\[  
  {  
    "type": "header",  
    "text": { "type": "plain\_text", "text": "📝 Code Change Proposal" }  
  },  
  {  
    "type": "section",  
    "text": { "type": "mrkdwn", "text": "\*Context:\* Refactoring user controller\\n\*Risk:\* \`Medium\`" }  
  },  
  {  
    "type": "section",  
    "text": { "type": "mrkdwn", "text": "The proposed changes exceed the inline display limit. Please see the attached snippet for full diff details." }  
  },  
  {  
    "type": "actions",  
    "elements": \[  
      { "type": "button", "style": "primary", "text": { "type": "plain\_text", "text": "✅ Accept Changes" }, "value": "approve" },  
      { "type": "button", "style": "danger", "text": { "type": "plain\_text", "text": "❌ Reject" }, "value": "reject" }  
    \]  
  }  
\]

*(Note: The actual diff content is uploaded as a separate files.upload call in the same thread to ensure proper syntax highlighting and to bypass message length restrictions).*

**Scenario: Directory Listing (`/monocoque list-files src`)**

\[
  {
    "type": "header",
    "text": { "type": "plain\_text", "text": "📁 Directory: src/" }
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "```\nsrc/\n├── main.rs\n├── lib.rs\n├── config/\n│   ├── mod.rs\n│   └── settings.rs\n├── mcp/\n│   ├── mod.rs\n│   ├── server.rs\n│   └── tools.rs\n├── slack/\n│   ├── mod.rs\n│   ├── bridge.rs\n│   └── blocks.rs\n└── session/\n    ├── mod.rs\n    ├── manager.rs\n    └── orchestrator.rs\n```"
    }
  },
  {
    "type": "context",
    "elements": \[
      { "type": "mrkdwn", "text": "📊 4 directories, 12 files | Depth: 3 | 💡 Use `--depth N` to adjust" }
    \]
  }
\]

*(Note: When the tree exceeds 40 lines, the code block is replaced by a files.upload call that posts a collapsible `.txt` snippet in the same thread.)*

**Scenario: File Contents — Small File (`/monocoque show-file src/config/settings.rs`)**

\[
  {
    "type": "header",
    "text": { "type": "plain\_text", "text": "📄 src/config/settings.rs" }
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "```rust\nuse serde::Deserialize;\n\n#[derive(Debug, Deserialize)]\npub struct Settings {\n    pub workspace_root: String,\n    pub host_cli: String,\n}\n\nimpl Settings {\n    pub fn load() -> anyhow::Result<Self> {\n        // ...\n    }\n}\n```"
    }
  },
  {
    "type": "context",
    "elements": \[
      { "type": "mrkdwn", "text": "📊 14 lines | 298 bytes | Last modified: 2026-02-07 09:32 UTC" }
    \]
  }
\]

*(Note: When the file exceeds 30 lines or 2 KB, the inline code block is replaced by a files.upload call with `filetype` set to the appropriate language identifier, e.g., `rust`, `typescript`, `python`. The header and context blocks are still posted as a regular message in the same thread.)*

**Scenario: File Contents — Large File (`/monocoque show-file src/mcp/server.rs`)**

\[
  {
    "type": "header",
    "text": { "type": "plain\_text", "text": "📄 src/mcp/server.rs" }
  },
  {
    "type": "section",
    "text": { "type": "mrkdwn", "text": "File exceeds the inline display limit (247 lines, 8.4 KB). See the attached snippet for full contents." }
  },
  {
    "type": "context",
    "elements": \[
      { "type": "mrkdwn", "text": "📊 247 lines | 8,412 bytes | Last modified: 2026-02-06 14:18 UTC | 💡 Use `--lines 10:50` to view a range" }
    \]
  }
\]

*(The file contents are uploaded as a separate files.upload call with `filetype: rust` and `title: src/mcp/server.rs` in the same thread.)*

**Scenario: Command Discovery (`/monocoque help`)**

The response uses categorized sections with dividers to provide a scannable, mobile-friendly reference. Each command is rendered as an inline code block with its arguments and a brief description. A `context` block at the bottom provides a hint for sub-category filtering. The full Block Kit JSON is specified in Section 4.5.

**Scenario: Session Start Confirmation**

\[
  {
    "type": "section",
    "text": { "type": "mrkdwn", "text": "🚀 \*Session started\* (`ses-xyz-789`)\n\*Prompt:\* _Implement rate limiting middleware for the API gateway_" }
  },
  {
    "type": "actions",
    "elements": \[
      { "type": "button", "style": "danger", "text": { "type": "plain\_text", "text": "🛑 Stop" }, "value": "session-clear:ses-xyz-789" }
    \]
  }
\]

**Scenario: Session Listing (`/monocoque sessions`)**

\[
  {
    "type": "header",
    "text": { "type": "plain\_text", "text": "📋 Active Sessions" }
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "| ID | State | Started | Last Activity |\n|:---|:---|:---|:---|\n| `ses-xyz-789` | 🟢 Active | 2h ago | 5m ago |\n| `ses-abc-123` | ⏸️ Paused | 1d ago | 6h ago |\n| `ses-def-456` | 🔴 Terminated | 3d ago | 3d ago |"
    }
  },
  {
    "type": "context",
    "elements": \[
      { "type": "mrkdwn", "text": "💡 Use `/monocoque session-resume <id>` to resume a paused session." }
    \]
  }
\]

**Scenario: Continuation Prompt (`forward_prompt`)**

\[
  {
    "type": "header",
    "text": { "type": "plain\_text", "text": "⏳ Agent Awaiting Direction" }
  },
  {
    "type": "section",
    "text": {
      "type": "mrkdwn",
      "text": "\*Session:\* `ses-xyz-789`\n\*Elapsed:\* 12m 00s | \*Actions taken:\* 47\n\n> Copilot has been working on this problem for a while. It can continue to iterate, or you can send a new message to refine your prompt."
    }
  },
  {
    "type": "actions",
    "block\_id": "continuation\_prm-def-321",
    "elements": \[
      { "type": "button", "style": "primary", "text": { "type": "plain\_text", "text": "▶️ Continue" }, "value": "continue:prm-def-321" },
      { "type": "button", "text": { "type": "plain\_text", "text": "✏️ Refine" }, "value": "refine:prm-def-321" },
      { "type": "button", "style": "danger", "text": { "type": "plain\_text", "text": "🛑 Stop" }, "value": "stop:prm-def-321" }
    \]
  },
  {
    "type": "context",
    "elements": \[
      { "type": "mrkdwn", "text": "💡 \*Refine\* opens a dialog to update the agent's instruction. \*Stop\* terminates the current task." }
    \]
  }
\]

*(Note: When the operator selects "Refine", a `views.open` modal is triggered with a `plain_text_input` block pre-populated with the session's original prompt. Upon submission, the revised instruction is relayed back to the agent. After any action is taken, the buttons in the original message are replaced with a static text block indicating the chosen action to prevent duplicate responses.)*

## **7\. Security Protocols and Configuration Parameters**

### **7.1 Configuration File**

The system configuration is managed via a TOML file located at \~/.config/monocoque/config.toml. This file serves as the single source of truth for security policies.

\[server\]  
workspace\_root \= "/Users/dev/projects/my-app" \# The jail for file operations. All file access is relative to this path.
transport \= "stdio"                    \# Primary transport for the initial host connection. Options: "stdio", "sse".
sse\_port \= 3100                        \# Port for the SSE/HTTP transport used by spawned sessions (session-start).
approval\_timeout\_seconds \= 3600        \# Default timeout for ask\_approval requests (1 hour).
prompt\_timeout\_seconds \= 1800          \# Default timeout for forward\_prompt requests (30 minutes).
command\_output\_limit\_bytes \= 65536     \# Maximum output size captured from custom command execution (64 KB). Output exceeding this limit is truncated with a warning.
command\_timeout\_seconds \= 60           \# Maximum execution time for custom commands before forced termination.
log\_level \= "info"                      \# Server logging verbosity. Options: "trace", "debug", "info", "warn", "error".
log\_file \= "\~/.local/share/monocoque/monocoque.log" \# Optional log file path. When omitted, logs are written to stderr only.

\[slack\]  
app\_token \= "xapp-..." \# The App-level token for Socket Mode.  
bot\_token \= "xoxb-..." \# The Bot User OAuth Token for API calls.
channel\_id \= "C0123456789"             \# The Slack channel ID where the bot operates.
authorized\_user\_ids \= \["U0123456789"\]  \# Slack user IDs permitted to approve proposals, issue commands, and interact with session controls. Interactions from unauthorized users are silently ignored and logged as security events.
reconnect\_backoff\_max\_seconds \= 300    \# Maximum backoff interval for WebSocket reconnection attempts (exponential backoff with jitter).

\# STRICT ALLOWLIST: Only commands listed here can be triggered remotely.  
\# The key represents the alias used in Slack; the value is the command executed locally.  
\[remote\_commands\]  
status \= "git status"  
last-commit \= "git log \-1 \--oneline"  
test \= "cargo test \-- \--nocapture"  
deploy \= "./scripts/deploy\_staging.sh"

\[session\]  
host\_cli \= "claude" \# The binary used to spawn agent sessions.  
host\_args \= \["\--mcp", "monocoque"\] \# Default arguments appended to every session start.  
max\_concurrent\_sessions \= 3 \# Maximum number of simultaneously active sessions.  
checkpoint\_retention\_days \= 30 \# Days to retain checkpoint data before pruning.
### **7.2 Workspace Configuration File**
**Discovery Mechanism:** On startup, the server resolves the `workspace_root` from `config.toml` and probes for `<workspace_root>/.monocoque/settings.json`. If the file exists, it is parsed and cached. A file system watcher (using `notify` crate) monitors the file for changes and hot-reloads the policy without requiring a server restart.

**Validation:** The server validates the file against the JSON Schema on load. Malformed or invalid files are rejected with an error logged to both the local console and the Slack channel. The server falls back to the "require approval for everything" default rather than operating with a partially parsed policy.

### **7.3 Safety Rails**

* **Registry Allowlist:** The server shall strictly reject any command that does not correspond to a key in the \[remote\_commands\] table or the built-in command list. This effectively neutralizes the risk of arbitrary code execution from the chat interface.  
* **No Argument Injection:** Custom commands do *not* accept arbitrary arguments from Slack by default. The command string defined in the TOML file is executed exactly as written. This prevents parameter injection attacks (e.g., trying to append ; rm \-rf /).  
* **Path Traversal Protection:** For the show-file and list-files commands, it is mandated that the resolved canonical path must commence with the configured workspace\_root. Any attempt to access parent directories (e.g., ../) will result in an immediate permission denied error.
* **Auto-Approve Boundary Enforcement:** The `.monocoque/settings.json` workspace policy can only auto-approve operations that are already permitted by the global `config.toml`. The workspace file cannot introduce new commands, elevate risk thresholds beyond the server default, or reference paths outside the `workspace_root`. This is enforced by intersecting the workspace policy with the global allowlist at load time.
* **Idempotency of Diff Application:** The `accept_diff` tool marks each approval record as `consumed` upon successful application. Repeated invocations with the same `request_id` return an `already_consumed` error without modifying the file system. This prevents accidental double-writes.
* **Audit Trail for Auto-Approved Actions:** All auto-approved operations are logged to the embedded database with timestamps. When `notifications.logAutoApproved` is enabled, they are also posted to the Slack channel to maintain operator visibility.
* **Session Spawn Boundary:** The `session-start` command only spawns the binary specified in `[session].host_cli`. The prompt text is passed as a single quoted argument via `shlex` escaping. The operator cannot specify an arbitrary binary or inject shell metacharacters into the spawn command.
* **Concurrent Session Limit:** The `max_concurrent_sessions` configuration parameter enforces a hard cap on simultaneously active sessions. Attempts to start a session beyond this limit return an error to Slack. This prevents resource exhaustion on the local workstation.
* **Checkpoint Integrity:** Checkpoint restoration compares stored file hashes against the current workspace. Divergences are surfaced to the operator with an explicit confirmation gate. Automatic restoration without confirmation is never permitted.
* **Slack User Authorization:** Only Slack user IDs listed in `config.toml` `[slack].authorized_user_ids` may interact with approval buttons, slash commands, and session controls. Interaction payloads from unauthorized users are silently discarded. Each unauthorized attempt is logged as a security event with the user ID, action attempted, and timestamp.
* **Slack Rate Limiting:** The server respects the Slack Web API rate limits (Tier 1–4). Outbound messages are queued and dispatched with exponential backoff when `429 Too Many Requests` responses are received. Burst-heavy operations (e.g., `remote_log` in a tight loop) are coalesced into batched updates when the rate limit is approached.
* **Token Storage:** The `config.toml` file containing Slack tokens should be protected with file system permissions (e.g., `chmod 600`). The server validates file permissions on startup and emits a warning if the file is world-readable. Future versions may integrate with OS-level secret managers (e.g., macOS Keychain, Windows Credential Manager).

### **7.4 Error Handling and Failover Strategy**

The server adopts a "degrade gracefully, never silently" philosophy. Every error condition is surfaced to at least one operator interface (Slack, local console, or IPC).

| Error Condition | Behavior |
| :---- | :---- |
| Slack WebSocket disconnection | The server enters a reconnection loop with exponential backoff (initial: 1s, max: `reconnect_backoff_max_seconds`). Pending approval requests remain in the SurrealDB embedded database and are re-posted upon reconnection. The local console logs each reconnection attempt. |
| Slack API rate limit (429) | Outbound messages are queued in-memory and retried after the `Retry-After` header interval. The queue is bounded (default: 500 messages) to prevent memory exhaustion. |
| SurrealDB database corruption | The server logs the error and attempts to re-open the database with a fresh namespace. If the underlying RocksDB storage is corrupted beyond recovery, the server starts with a fresh database directory and logs a critical warning to stderr. Checkpoint data is lost; pending requests must be re-created by the agent via `ask_approval`. |
| Config file missing or invalid | The server refuses to start and prints a descriptive error to stderr with the expected file path and schema. A non-zero exit code is returned. |
| `.monocoque/settings.json` invalid | The workspace policy is rejected. The server falls back to "require approval for everything" and logs a warning to both stderr and Slack. The server continues running. |
| Agent process crash (spawned session) | The Session Orchestrator detects the process exit via `tokio::process::Child::wait`. The session state is moved to `Terminated`. A notification is posted to Slack: "⚠️ Session `<id>` terminated unexpectedly (exit code: `<code>`)." Pending approval requests for the session are marked as `orphaned`. |
| Custom command timeout | Custom commands are executed with a configurable timeout (default: 60 seconds). Commands exceeding the timeout are killed via `SIGKILL` (Unix) or `TerminateProcess` (Windows). A timeout error is posted to Slack. |
| Output size exceeded | Custom command output exceeding `command_output_limit_bytes` is truncated. The Slack message includes a warning: "⚠️ Output truncated at 64 KB." |

### **7.5 Graceful Shutdown**

Upon receiving `SIGINT` or `SIGTERM` (Unix) or `CTRL_C_EVENT` (Windows):

1. The server stops accepting new MCP tool calls and slash commands.
2. Active approval requests are marked as `interrupted` in the SurrealDB embedded database with a timestamp.
3. A final Slack message is posted: "🔴 **Monocoque server shutting down.** Pending requests have been saved and will be restored on next startup."
4. The Slack WebSocket connection is closed cleanly.
5. The SurrealDB embedded database is flushed to disk.
6. Spawned agent processes (if any) are sent `SIGTERM` and given 5 seconds to exit before `SIGKILL`.
7. The process exits with code 0.

### **7.6 Refine Modal Dialog Specification**

When the operator selects "✏️ Refine" on a continuation prompt (Section 5.6, step 6), the server triggers a Slack `views.open` API call with the following modal view:

```json
{
  "type": "modal",
  "callback_id": "refine_prompt_prm-def-321",
  "title": { "type": "plain_text", "text": "Refine Instruction" },
  "submit": { "type": "plain_text", "text": "Submit" },
  "close": { "type": "plain_text", "text": "Cancel" },
  "blocks": [
    {
      "type": "section",
      "text": {
        "type": "mrkdwn",
        "text": "*Current session:* `ses-xyz-789`\n*Original prompt:* _Implement rate limiting middleware for the API gateway_"
      }
    },
    {
      "type": "input",
      "block_id": "refined_instruction",
      "label": { "type": "plain_text", "text": "Updated Instruction" },
      "element": {
        "type": "plain_text_input",
        "action_id": "instruction_text",
        "multiline": true,
        "initial_value": "Implement rate limiting middleware for the API gateway",
        "placeholder": { "type": "plain_text", "text": "Provide a revised or supplementary instruction for the agent..." }
      }
    }
  ]
}
```

Upon submission, the `view_submission` event payload contains the updated instruction text at `view.state.values.refined_instruction.instruction_text.value`. The server extracts this value and returns it to the agent via the pending `forward_prompt` response.

## **8\. Developmental Trajectory**

1. **Phase 1-4:** Implementation of the Core MCP Server traits, Slack Socket Mode integration, and the Persistence Layer using SurrealDB (embedded mode).  
2. **Phase 5:** Implementation of the IPC Listener to support local overrides via monocoque-ctl.  
3. **Phase 6:** Compilation and release of the ctl subcommand functionality.  
4. **Phase 7:** Implementation of the Command Dispatcher module, incorporating the strict Registry Lookup logic and configuration file parsing.  
5. **Phase 8:** **(New)** Implementation of the "Smart Diff" logic. This involves building the heuristics to distinguish between short diffs (Block Kit) and long diffs (Snippets) and managing the file upload API calls.
6. **Phase 9:** **(New)** Implementation of the Diff Applicator module (`accept_diff` tool). This encompasses the unified diff patch engine, the full-file write mode, the idempotency guard, and the pre-write integrity check. Integration with the Session Manager to track consumed approvals.
7. **Phase 10:** **(New)** Implementation of the Workspace Policy Evaluator. This involves the `.monocoque/settings.json` discovery, JSON Schema validation, in-memory policy cache, file system watcher for hot-reload, and the `check_auto_approve` tool. Integration with the MCP Server Layer and Dispatcher to short-circuit approval gates.
8. **Phase 11:** **(New)** Implementation of the Session Orchestrator module. This encompasses the session state machine, the session registry in SurrealDB, the enhanced `/monocoque help` command with Block Kit rendering, the `session-start` spawn mechanism (including `shlex` escaping and host CLI integration), the `session-pause`/`session-resume` IPC signaling, and the `session-checkpoint`/`session-restore` serialization engine with file hash divergence detection.
9. **Phase 12:** **(New)** Implementation of the Continuation Prompt Forwarding module (`forward_prompt` tool). This encompasses the prompt interception mechanism, the `prompt_type` classification, the Block Kit rendering with session context (elapsed time, actions taken), the Slack modal dialog for the "Refine" action, the message update logic to prevent double-submission, and the auto-approve integration for `continuation` type prompts. Integration with the Session Manager for prompt persistence and session correlation.