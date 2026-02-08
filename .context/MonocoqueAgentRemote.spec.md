# **Monocoque Agent Remote (monocoque-agent-rem)**

## **Technical Specification & Architecture**

**Version:** 1.5.0

**Status:** Draft / Request for Comments

**Language:** Rust

**Protocol:** Model Context Protocol (MCP) v1.0

## **1\. Executive Synopsis**

The software entity herein designated as **Monocoque Agent Remote** (monocoque-agent-rem) is constituted as a standalone server implementation of the Model Context Protocol (MCP). The primary objective of this apparatus is the provision of "Remote Input/Output" capabilities to local Artificial Intelligence agents, encompassing but not limited to Claude Code, the GitHub Copilot Command Line Interface (CLI), Cursor, and Visual Studio Code.

Through the local execution of this server, an AI agent is endowed with the capacity to establish an asynchronous, bi-directional communication channel with a remote operator via the Slack platform. In standard operational paradigms, an agentic workflow is tethered to the physical console, requiring synchronous human intervention for the approval of file modifications or the execution of terminal commands. This system decouples that dependency. Rather than the terminal remaining in a blocked state whilst awaiting local input, the agent is enabled to transmit diffs, requests, and inquiries to a designated Slack channel, subsequently awaiting approval in an asynchronous manner. This architecture effectively permits the "Shadow Agent" workflow, wherein the computational heavy lifting occurs on a secure, local machine, while the orchestration and oversight are conducted remotely.

**Version 1.5 Update:** This iteration significantly refines the **Proposal Review Workflow**. It explicitly delineates the mechanism by which code differentials are rendered within the Slack interface—specifically, the utilization of "Smart Snippets" to handle variable content lengths. Furthermore, it rigorously maps the "Accept" action of the remote operator to the file write execution of the agent, thereby mirroring the "View Diff \-\> Accept" experience characteristic of the GitHub Copilot graphical interface.

## **2\. Architectural Framework**

The system is predicated upon the standard MCP Client/Host architecture, significantly augmented by a **Stateful Persistence Layer**, a **Local Inter-Process Communication (IPC) Interface**, and a **Registry-Based Command Engine**. This tripartite structure ensures resilience against network latency, process termination, and concurrent access attempts.

graph TD  
    subgraph "Local Workstation"  
        IDE\[Agentic IDE / CLI\] \--\>|Stdio / SSE| MCP\[monocoque-agent-rem (Rust)\]  
        IDE \-- "Calls Tool: ask\_approval(diff)" \--\> MCP  
          
        MCP \-- "Polls/Listens" \--\> IPC\[Local IPC Socket / File\]  
        CLI\[monocoque-ctl\] \-.-\>|Approves| IPC  
          
        DB\[(Local State DB)\] \<--\> MCP  
        CMD\[Command Dispatcher\] \-- "Look up & Exec" \--\> Registry\[Config.toml Allowlist\]  
        MCP \-- "Routes /cmd" \--\> CMD  
    end

    subgraph "External Network"  
        MCP \-- "WSS (Socket Mode)" \--\> SlackAPI\[Slack API\]  
    end

    subgraph "Remote User"  
        SlackApp\[Slack Mobile App\] \-- "Action: Accept Changes" \--\> SlackAPI  
    end

### **2.1 Functional Component Designation**

* **The Host (Agent):** The Integrated Development Environment (e.g., Claude Code, Cursor) or CLI tool. This component is responsible for driving the logical processes, generating code proposals, and initiating tool calls. It functions as the "brain" of the operation but possesses no inherent capability to communicate outside the local shell.  
* **The Server (Monocoque):** A stateful bridge and protocol translator. This component exposes the MCP tools and maintains a **Session Mailbox**. It is responsible for formatting MCP JSON-RPC requests into Slack Block Kit payloads, managing the WebSocket lifecycle, and persisting state to the local database to survive system restarts.  
* **The Local Controller (CLI):** A lightweight binary executable intended for local overrides. This component provides a mechanism for the operator, should they be physically present at the workstation, to intervene in the agent's operation via a secondary terminal window, bypassing the network round-trip to Slack.  
* **The Dispatcher:** A security-critical module tasked with the validation and routing of incoming slash commands. It performs a lookup within the local registry and executes the corresponding safe shell command only if strict validation criteria are met.

## **3\. Implementation Specifications regarding the Rust Programming Language**

### **3.1 Dependency Inventory (Crates)**

The project shall leverage the Rust ecosystem to ensure memory safety, type safety, and high-performance concurrency. The selection of specific crates is justified as follows:

| Crate | Purpose and Justification |
| :---- | :---- |
| mcp\_rust\_sdk | The core implementation of the Model Context Protocol (Server traits). This library abstracts the complexities of the JSON-RPC message framing and transport layers. |
| slack-mrh or slack-rust | The handling of Slack Socket Mode and the construction of Block Kit JSON. These libraries facilitate the maintenance of a persistent WebSocket connection, obviating the need for inbound firewall ports or public IP addresses. |
| tokio | The asynchronous runtime selected for the simultaneous handling of WebSocket heartbeats, MCP request loops, and IPC task polling. Its "work-stealing" scheduler ensures minimal latency. |
| interprocess or tokio-uds | Local socket/pipe communication (Local Override). This enables the implementation of the monocoque-ctl side-channel without resorting to file-based locking mechanisms. |
| sled | The embedded database for the persistence of session state and configuration. Chosen for its lock-free architecture and ability to embed directly into the binary without requiring an external database server (like PostgreSQL). |
| serde / serde\_json | The de facto standard for serialization and deserialization in Rust, essential for parsing MCP payloads and Slack API responses. |
| anyhow | Error handling. Provides a robust mechanism for propagating context-rich error messages up the stack. |
| walkdir / glob | Utilized for efficient file system traversal in list-files. walkdir is preferred for its recursive capabilities and iterator-based interface. |
| shlex | The safe parsing of command line arguments. This is critical for ensuring that strings passed to the shell are properly escaped, mitigating injection vulnerabilities. |

### **3.2 Core Modules Description**

#### **A. The MCP Server Layer**

This module implements the mcp\_rust\_sdk::Server trait. It functions as the primary interface for the AI Agent, defining the contract of available Tools and Resources. It is responsible for decoding incoming JSON-RPC messages and routing them to the appropriate internal handler.

#### **B. The Slack Bridge Layer (Async Actor)**

This module manages the WebSocket connection to Slack using the Actor model. Because the Slack WebSocket client must run continuously to process heartbeats, while MCP tool calls are sporadic and blocking, this layer runs in a detached Tokio task. It routes incoming Slash Commands to Module E and outgoing notifications to the Slack API.

#### **C. The Session Manager (Persistence)**

This module handles Checkpoints and State Recovery. It utilizes the sled database to atomically store the state of every active "Approval Request." If the server process is terminated (e.g., via a system reboot), this module reconstructs the pending request queue upon restart, preventing data loss.

#### **D. The Local Control Layer**

This module listens on a Unix Domain Socket (or Named Pipe on Windows) for local overrides. It creates a secondary control plane, allowing monocoque-ctl to inject "Approve" or "Reject" signals directly into the pending request map.

#### **E. Registry Command Dispatcher**

This module parses incoming Slack text and maps said text to safe, pre-defined shell commands. It enforces a strict "deny-by-default" policy, ensuring that only commands explicitly whitelisted in the configuration file can be executed.

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
  * risk\_level (string): low | high | critical. High risk levels may trigger additional alerting mechanisms, such as @channel mentions.

#### **set\_operational\_mode**

Switches the server between Remote, Local, and Hybrid modes. This allows the operator to dynamically adjust the behavior of the approval gates based on their physical location (e.g., disabling Slack notifications when sitting at the desk).

#### **wait\_for\_instruction**

Places the system in a standby loop. The agent will pause execution and poll for a "Resume" signal or a new command payload from the Slack interface. This is essential for session continuity across long pauses.

#### **recover\_state**

Retrieves the last known checkpoint from the persistent database. This tool is called by the Agent upon startup to check if there was a pending approval request that was interrupted by a crash or timeout.

#### **remote\_log**

Transmits status updates and logging information to the Slack channel without blocking execution. This is utilized to keep the remote operator informed of progress (e.g., "Running tests...", "Build completed").

### **4.2 Resource Access (Context Reading)**

* slack://channel/{id}/recent: Reads the recent chat history from the configured channel. This allows the Agent to "read" instructions or feedback provided by the user in the chat thread, effectively treating the chat log as a dynamic context source.

### **4.3 Direct Remote Commands (Registry Only)**

(Utilized by the Human via Slack.)

To mitigate security risks, this feature is **Strictly Registry-Based**. It does not allow arbitrary shell execution.

#### **Built-in Commands (Always Available)**

| Command | Arguments | Description |
| :---- | :---- | :---- |
| list-files | \[path\] | Lists files in the allowed workspace. This command is strictly restricted to the workspace\_root. It performs a traversal of the directory structure and returns a formatted tree view. |
| show-file | \<path\> | Reads a file and returns it as a Slack snippet. This is strictly restricted to the workspace\_root to prevent exfiltration of system files. |
| help | None | Lists the allowed commands currently defined in config.toml, providing the user with a menu of available actions. |

#### **Custom Registry Commands**

Commands are triggered via the slash command syntax /monocoque \<alias\>.

* **Example:** The Operator types /monocoque status.  
* **Server Action:** The system parses the command, looks up the key status in the config.toml file, locates the corresponding value git status, and executes git status in the shell. The standard output is captured and posted back to Slack.

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
7. **Monocoque:** The server receives the WebSocket event, correlates it with the pending request ID, and sends {"status": "approved"} to the Agent.  
8. **Agent (Local):** The Agent receives the approval and executes the file write operation to disk.  
9. **Agent (Local):** The Agent calls remote\_log("File created successfully.") to close the loop.

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

## **7\. Security Protocols and Configuration Parameters**

### **7.1 Configuration File**

The system configuration is managed via a TOML file located at \~/.config/monocoque/config.toml. This file serves as the single source of truth for security policies.

\[server\]  
workspace\_root \= "/Users/dev/projects/my-app" \# The jail for file operations. All file access is relative to this path.

\[slack\]  
app\_token \= "xapp-..." \# The App-level token for Socket Mode.  
bot\_token \= "xoxb-..." \# The Bot User OAuth Token for API calls.

\# STRICT ALLOWLIST: Only commands listed here can be triggered remotely.  
\# The key represents the alias used in Slack; the value is the command executed locally.  
\[remote\_commands\]  
status \= "git status"  
last-commit \= "git log \-1 \--oneline"  
test \= "cargo test \-- \--nocapture"  
deploy \= "./scripts/deploy\_staging.sh"

### **7.2 Safety Rails**

* **Registry Allowlist:** The server shall strictly reject any command that does not correspond to a key in the \[remote\_commands\] table or the built-in command list. This effectively neutralizes the risk of arbitrary code execution from the chat interface.  
* **No Argument Injection:** Custom commands do *not* accept arbitrary arguments from Slack by default. The command string defined in the TOML file is executed exactly as written. This prevents parameter injection attacks (e.g., trying to append ; rm \-rf /).  
* **Path Traversal Protection:** For the show-file and list-files commands, it is mandated that the resolved canonical path must commence with the configured workspace\_root. Any attempt to access parent directories (e.g., ../) will result in an immediate permission denied error.

## **8\. Developmental Trajectory**

1. **Phase 1-4:** Implementation of the Core MCP Server traits, Slack Socket Mode integration, and the Persistence Layer using sled.  
2. **Phase 5:** Implementation of the IPC Listener to support local overrides via monocoque-ctl.  
3. **Phase 6:** Compilation and release of the ctl subcommand functionality.  
4. **Phase 7:** Implementation of the Command Dispatcher module, incorporating the strict Registry Lookup logic and configuration file parsing.  
5. **Phase 8:** **(New)** Implementation of the "Smart Diff" logic. This involves building the heuristics to distinguish between short diffs (Block Kit) and long diffs (Snippets) and managing the file upload API calls.