# Backlog

## Feature Groups

### Feature 005-intercom-acp-server

Implement Agent Client Protocol (ACP) server mode for `agent-intercom` to actively send prompts to agents and receive responses, in addition to the existing Model Context Protocol (MCP) passive server mode.

Introducing a startup flag (like `--mcp` or `--acp`) is the standard way to handle this in Rust ecosystem tooling. It allows `agent-intercom` to act as a "Swiss Army knife" for agent control, depending on how the user wants to integrate it.

Here is how you can architect this dual-mode system without turning your codebase into a tangled mess of `if/else` statements.

### The "Shared Core" Architecture

The beauty of keeping both modes is that about 70% of your application does exactly the same thing regardless of the protocol. You want to isolate the AI protocol logic to the edges of your application.

Here is how the responsibilities break down:

**1. The Shared Foundation (Always Runs)**

* **Slack Socket Mode:** The `slack-morphism` event loop that listens for `/intercom` commands and interactive button clicks (Accept/Reject).
* **Persistence Layer:** Your SQLite `sqlx` database tracking active sessions, checkpoints, and user policies.
* **UI Layer:** The Slack Block Kit formatting logic.

**2. The MCP Interface (`--mcp`)**

* **Role:** Passive Server.
* **Execution:** Spins up the Axum HTTP server (or stdio listener) and waits for the AI agent to initiate a connection.
* **Flow:** The agent decides when to trigger the `check_clearance` tool. Your server intercepts, posts to Slack, holds the HTTP request open, and returns the Slack approval back to the agent.

**3. The ACP Interface (`--acp`)**

* **Role:** Active Controller / Client.
* **Execution:** Initiates a TCP connection to `localhost:8080` (where GitHub Copilot CLI is running in headless mode).
* **Flow:** Your Slack command (`/intercom session-start "build a web server"`) sends the initial JSON-RPC prompt *to* the agent. Your server then listens to the TCP stream for `window/showMessageRequest` payloads to trigger the Slack approval UI.

### Structuring the CLI in Rust

If you are using the `clap` crate for your CLI arguments, the cleanest way to represent this is using subcommands or an enum for the mode.

```rust
use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(name = "agent-intercom")]
struct Cli {
    /// The protocol mode to run the server in
    #[arg(long, value_enum, default_value_t = Mode::Mcp)]
    mode: Mode,
    
    // ... other shared args like config path
}

#[derive(Clone, ValueEnum)]
enum Mode {
    /// Run as a passive Model Context Protocol (MCP) server
    Mcp,
    /// Run as an active Agent Client Protocol (ACP) bridge
    Acp,
}

```

### The State Machine Challenge

The biggest architectural hurdle you will face when merging these two paradigms is **Session State Ownership**:

| Feature | MCP Mode (`--mcp`) | ACP Mode (`--acp`) |
| --- | --- | --- |
| **Who starts the session?** | The IDE / Agent (e.g., user types in Cursor). | `agent-intercom` (via Slack command). |
| **Who owns the context?** | The Agent. Your server only sees what the agent passes into the tool call. | Your server. You dictate the prompt and control the working directory. |
| **Stall Detection** | Handled via your existing `ping` tool and timeouts. | Handled by monitoring the TCP stream for inactivity. |

To handle this cleanly, you should define a Rust `Trait` (e.g., `AgentDriver`) that abstracts the protocol.

Your Slack event loop shouldn't care if it's running in MCP or ACP mode. When a user clicks "Accept" in Slack, the Slack handler just calls `agent_driver.approve(request_id)`.

* If running in MCP mode, that trait implementation fulfills the pending Axum HTTP future.
* If running in ACP mode, that trait implementation serializes a JSON-RPC approval and writes it to the TCP stream.

Abstracting the protocol behind a trait is the perfect way to keep your Slack event loop clean. It also gives you a massive bonus: **testability**. You can easily write a `MockDriver` to simulate an agent requesting file changes without having to actually spin up Copilot or Cursor.

To make this work asynchronously in Rust, we need to handle two-way communication.

1. **Inbound (Slack $\rightarrow$ Agent):** Handled by calling methods on the `AgentDriver` trait.
2. **Outbound (Agent $\rightarrow$ Slack):** Handled by the driver pushing events into a `tokio::sync::mpsc` channel that your core application listens to.

Here is a clean, decoupled design specification for that trait and its associated events.

### 1. The Event Enum (Agent $\rightarrow$ Core)

First, define the events your drivers will emit. Your core application (the Slack loop and SQLite state manager) will listen to a stream of these events and react accordingly.

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Emitted when the agent needs permission to modify a file or run a command
    ClearanceRequested {
        request_id: String,
        session_id: String,
        action_type: ActionType, // e.g., FileWrite, TerminalCommand
        description: String,
        diff: Option<String>,
    },
    /// Emitted when the agent sends a status update or reasoning text
    StatusUpdated {
        session_id: String,
        message: String,
    },
    /// Emitted when the agent finishes its task or crashes
    SessionTerminated {
        session_id: String,
        exit_code: i32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    FileWrite(PathBuf),
    FileDelete(PathBuf),
    TerminalCommand(String),
}

```

### 2. The Driver Trait (Core $\rightarrow$ Agent)

Next, define the trait. Since this will be heavily asynchronous and shared across threads, using `async_trait` is usually the cleanest approach here.

```rust
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DriverError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

#[async_trait]
pub trait AgentDriver: Send + Sync {
    /// Responds to a pending `ClearanceRequested` event.
    /// In MCP: Completes the pending HTTP tool call.
    /// In ACP: Sends a window/showMessageRequest response over TCP.
    async fn resolve_clearance(
        &self, 
        request_id: &str, 
        approved: bool, 
        feedback: Option<String>
    ) -> Result<(), DriverError>;

    /// Sends a new prompt to the agent.
    /// In MCP: This might just broadcast a message or be a no-op if the IDE owns the prompt.
    /// In ACP: Sends a session/prompt JSON-RPC payload.
    async fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), DriverError>;

    /// Halts the current agent execution.
    /// In MCP: Returns an error/abort signal to the tool call.
    /// In ACP: Sends a cancellation request over TCP.
    async fn interrupt(&self, session_id: &str) -> Result<(), DriverError>;
}

```

### 3. Wiring It Together in Main

When your application starts, it will check the command-line flag, initialize the appropriate driver, and hand the event receiver off to your Slack loop.

```rust
use tokio::sync::mpsc;
use std::sync::Arc;

// Assume we have McpDriver and AcpDriver structs that implement AgentDriver

#[tokio::main]
async fn main() {
    let cli = Cli::parse(); // From our previous clap setup
    
    // Create the channel for the driver to send events back to the app
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(100);

    // Initialize the specific driver based on the CLI flag
    let driver: Arc<dyn AgentDriver> = match cli.mode {
        Mode::Mcp => {
            Arc::new(McpDriver::start(event_tx.clone(), config).await.unwrap())
        },
        Mode::Acp => {
            Arc::new(AcpDriver::start(event_tx.clone(), config).await.unwrap())
        }
    };

    // Spawn the core event loop that listens to the driver
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AgentEvent::ClearanceRequested { request_id, description, .. } => {
                    // 1. Save pending request to SQLite
                    // 2. Format a Block Kit message
                    // 3. Post to Slack
                    println!("Posting approval to Slack for: {}", description);
                }
                // ... handle other events
            }
        }
    });

    // ... Run your Slack Socket Mode client loop here
    // When Slack gets an "Approve" button click, it calls:
    // driver.resolve_clearance(&request_id, true, None).await;
}

```

### The Beauty of this Setup

By structuring `agent-intercom` this way, your complex Slack Block Kit code and SQLite checkpointing logic never have to parse a raw JSON-RPC payload or deal with HTTP headers. The `McpDriver` and `AcpDriver` act as simple translators, wrapping the messy wire protocols into clean `AgentEvent` structs.

Look at how to implement the `tokio::sync::oneshot` channels inside the `McpDriver`? That is the standard pattern for holding a `check_clearance` HTTP request open while waiting for the Slack approval to come back asynchronously.

Implementing `tokio::sync::oneshot` is the textbook way to bridge the gap between a synchronous-looking HTTP request and an unpredictable, asynchronous human action (like waiting for a Slack button click).

By using oneshot channels, the MCP tool call cleanly suspends its execution without blocking your server's underlying threads. When the approval finally comes in, the channel wakes the exact HTTP task back up so it can return the response to the agent.

Here is the blueprint for wiring this up inside your `McpDriver`.

### 1. The Shared State Map

Because the Slack approval will come in on a completely different HTTP/WebSocket request than the one the AI agent is holding open, your driver needs a thread-safe place to store the "waker" (the sender side of the oneshot channel).

We will use a standard `HashMap` wrapped in an `Arc<Mutex<...>>` for this.

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use async_trait::async_trait;
use uuid::Uuid; // For generating request IDs

// The payload sent back to the waiting MCP task
#[derive(Debug)]
pub struct ClearanceResponse {
    pub approved: bool,
    pub feedback: Option<String>,
}

// A thread-safe map holding the Sender half of the oneshot channels
type PendingRequestsMap = Arc<Mutex<HashMap<String, oneshot::Sender<ClearanceResponse>>>>;

pub struct McpDriver {
    /// Holds pending requests waiting for a Slack response
    pending_requests: PendingRequestsMap,
    /// Channel to send events out to the core Slack loop
    event_tx: mpsc::Sender<AgentEvent>,
}

```

### 2. The MCP Tool Handler (The Suspending Task)

When the AI agent (e.g., Claude or Cursor) makes an HTTP POST to your MCP server invoking the `check_clearance` tool, it triggers a handler function. This is where the oneshot channel is created and `.await`-ed.

```rust
impl McpDriver {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// This simulates the Axum handler or `rmcp` tool execution logic.
    pub async fn handle_check_clearance(&self, session_id: String, diff: String) -> String {
        let request_id = Uuid::new_v4().to_string();
        
        // 1. Create the oneshot channel
        let (tx, rx) = oneshot::channel::<ClearanceResponse>();

        // 2. Store the transmitter (tx) in our shared map using the request_id
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.insert(request_id.clone(), tx);
        }

        // 3. Fire the event out to the core loop (which will post the Slack message)
        let event = AgentEvent::ClearanceRequested {
            request_id: request_id.clone(),
            session_id,
            action_type: ActionType::FileWrite(PathBuf::from("unknown")), // Map accordingly
            description: "Agent wants to apply a diff".to_string(),
            diff: Some(diff),
        };
        
        if self.event_tx.send(event).await.is_err() {
            return "System error: Failed to route request to Slack.".to_string();
        }

        // 4. SUSPEND execution here until Slack replies. 
        // This does not block the thread, just this specific Tokio task.
        match rx.await {
            Ok(response) => {
                if response.approved {
                    "Approval granted. You may proceed.".to_string()
                } else {
                    let reason = response.feedback.unwrap_or_else(|| "No reason provided".to_string());
                    format!("Request rejected by operator. Reason: {}", reason)
                }
            }
            Err(_) => {
                // This happens if the sender (tx) is dropped before sending a message.
                "Request was aborted or timed out.".to_string()
            }
        }
    }
}

```

### 3. The Driver Trait Implementation (The Waking Task)

Now, we implement the `AgentDriver` trait we defined earlier. When you click "Accept" in Slack, your core app loop receives the Socket Mode event, parses the `request_id` from the button's payload, and calls `resolve_clearance`.

This method simply pops the `oneshot::Sender` out of the map and fires the result down the tube.

```rust
#[async_trait]
impl AgentDriver for McpDriver {
    async fn resolve_clearance(
        &self, 
        request_id: &str, 
        approved: bool, 
        feedback: Option<String>
    ) -> Result<(), DriverError> {
        
        // 1. Safely remove the sender from the map
        let sender_opt = {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.remove(request_id)
        };

        // 2. If it exists, send the response
        if let Some(tx) = sender_opt {
            let response = ClearanceResponse { approved, feedback };
            
            // Send the response. If the receiver was already dropped 
            // (e.g., the agent disconnected), ignore the error.
            let _ = tx.send(response);
            Ok(())
        } else {
            // The request ID wasn't in the map. It may have timed out or already been answered.
            Err(DriverError::SessionNotFound(request_id.to_string()))
        }
    }

    // ... other trait methods (send_prompt, interrupt)
}

```

### Why This is Bulletproof

* **No Deadlocks:** Using a standard `std::sync::Mutex` here is perfectly safe and fast because you are only holding the lock long enough to `insert` or `remove` a value from the `HashMap`. You are never holding the lock across an `.await` point.
* **Memory Safe:** By using `.remove(request_id)` inside `resolve_clearance`, you guarantee that the channel is destroyed and memory is freed immediately after the response is handled.
* **Crash Resilient:** If the Slack user never responds, the `rx.await` in the tool handler could wait forever. In a production environment, you would wrap `rx.await` in a `tokio::time::timeout` to automatically reject the request if no one responds within, say, 10 minutes.

Look at how to map this incoming oneshot pattern over to the new ACP mode, where instead of holding an HTTP request open, you are responding to a `window/showMessageRequest` over a JSON-RPC TCP stream. The same `resolve_clearance` method can be implemented to serialize a JSON response and write it to the TCP stream instead of sending a message down a channel. This is the beauty of abstracting the protocol behind a trait. Your Slack event loop and core application logic remain blissfully unaware of the underlying communication mechanism.

Mapping this pattern to ACP reveals a fundamental difference between HTTP frameworks and raw TCP sockets. It is actually a major "aha!" moment when building AI agent tooling.

With MCP (via an HTTP framework like Axum), the framework expects your handler function to return a value before it closes the connection. That is why you **must** use a `oneshot` channel to suspend the function until the Slack approval arrives.

With ACP (JSON-RPC over a raw TCP stream), the connection is **full-duplex and continuous**. You do not need to suspend a function at all! JSON-RPC handles request/response correlation natively using the `"id"` field.

Here is how the architecture shifts and how you implement the `AgentDriver` for ACP.

### 1. The Split Stream Architecture

When your Rust server connects to Copilot CLI's TCP port, you will immediately split the stream into two independent Tokio tasks:

* **The Read Task:** Continuously reads incoming bytes, parses JSON-RPC objects, and emits `AgentEvent`s to your core Slack loop.
* **The Write Task:** Listens on an `mpsc` channel for outgoing JSON-RPC objects, serializes them, and writes them to the TCP socket.

### 2. The ACP Driver State

Because the Write Task handles all outbound TCP traffic, your `AcpDriver` struct doesn't need a complex `HashMap` of `oneshot` channels anymore. It just needs a transmitter to send messages to the Write Task.

```rust
use tokio::sync::mpsc;
use serde_json::{json, Value};
use async_trait::async_trait;

pub struct AcpDriver {
    /// Channel to send raw JSON-RPC responses to the TCP Write Task
    tcp_tx: mpsc::Sender<Value>,
    /// Channel to send parsed events to the core Slack loop
    event_tx: mpsc::Sender<AgentEvent>,
}

impl AcpDriver {
    pub fn new(tcp_tx: mpsc::Sender<Value>, event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self { tcp_tx, event_tx }
    }
}

```

### 3. The Read Task (Generating the Event)

When Copilot wants permission to modify a file, it sends a JSON-RPC request over TCP that looks like this:

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "window/showMessageRequest",
  "params": { "message": "Execute npm install?", "actions": [{"title": "Accept"}, {"title": "Reject"}] }
}

```

Your TCP Read Task parses this, grabs the ID (`42`), and fires it off to Slack. Notice that it **does not wait** for a response. It immediately goes back to listening for more TCP traffic (like status updates).

```rust
// Inside your TCP Read Loop Task:
if method == "window/showMessageRequest" {
    let request_id = request["id"].to_string(); // e.g., "42"
    
    let event = AgentEvent::ClearanceRequested {
        request_id, // We pass the JSON-RPC ID straight through
        session_id: current_session.clone(),
        action_type: ActionType::TerminalCommand("npm install".into()),
        description: request["params"]["message"].as_str().unwrap_or("").to_string(),
        diff: None,
    };
    
    // Fire it to the Slack loop and immediately move on
    let _ = event_tx.send(event).await; 
}

```

### 4. The Driver Trait Implementation (Sending the Approval)

When you click "Accept" in Slack, your core app loop calls `driver.resolve_clearance("42", true, None)`.

The `AcpDriver` simply constructs a JSON-RPC response matching that exact ID and shoves it into the `tcp_tx` channel. Copilot CLI receives the ID, matches it to its own internal pending request, and resumes its execution.

```rust
#[async_trait]
impl AgentDriver for AcpDriver {
    async fn resolve_clearance(
        &self, 
        request_id: &str, 
        approved: bool, 
        _feedback: Option<String>
    ) -> Result<(), DriverError> {
        
        // 1. Determine the action based on the Slack button click
        let action_title = if approved { "Accept" } else { "Reject" };

        // 2. Format the exact JSON-RPC response Copilot is waiting for
        let response = json!({
            "jsonrpc": "2.0",
            // We use the exact ID that came from the Read Task
            "id": serde_json::from_str::<Value>(request_id).unwrap_or(json!(request_id)), 
            "result": {
                "title": action_title
            }
        });

        // 3. Send it to the TCP Write Task to be pushed over the wire
        self.tcp_tx
            .send(response)
            .await
            .map_err(|_| DriverError::Network("TCP Write task has died".to_string()))?;

        Ok(())
    }

    // ... send_prompt and interrupt methods work the exact same way, 
    // just formatting different JSON-RPC payloads!
}

```

### Why this is a massive upgrade

By moving to this event-driven, full-duplex model, your server becomes incredibly resilient.

* If you take 10 minutes to click "Accept" on your phone, the TCP connection remains perfectly healthy and continues streaming background logs or pings.
* You don't have to worry about HTTP timeouts.
* You completely eliminate the `HashMap` and `Mutex` overhead used in the MCP oneshot workaround.

Look at how to cleanly frame and parse the incoming JSON stream in your TCP Read Task using `tokio_util::codec::LinesCodec` so you don't have to deal with fragmented byte chunks manually. This is the standard way to handle line-delimited JSON streams in Rust and will save you from a world of pain trying to buffer and parse incomplete messages.

Working with raw TCP sockets is where many developers get tripped up because TCP is a continuous stream of bytes, not a stream of discrete messages.

If you just run `socket.read(&mut buffer)`, you might get half of a JSON-RPC payload, or you might get one and a half payloads glued together. If you try to pass that fragment into `serde_json`, it will panic and crash your Read Task.

To solve this, we use **framing**. Assuming the ACP implementation uses newline-delimited JSON (NDJSON) over the socket, `tokio_util` provides a brilliant abstraction called `FramedRead` paired with `LinesCodec`. It buffers the incoming bytes automatically and only yields a string when it detects a clean `\n` or `\r\n`.

Here is how you wire up that bulletproof TCP Read Task.

### 1. Add the Dependencies

You will need to add `tokio-util` and `futures` to your `Cargo.toml`. The `futures` crate provides the `StreamExt` trait, which allows you to use `.next().await` on the framed stream.

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
futures = "0.3"
serde_json = "1.0"

```

### 2. Splitting and Framing the Stream

When you connect to Copilot's TCP port, you first split the socket into independent read and write halves. Then, you wrap the read half in the `LinesCodec`.

```rust
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec};
use futures::stream::StreamExt;
use serde_json::Value;

pub async fn start_tcp_read_task(
    mut socket: TcpStream, 
    event_tx: mpsc::Sender<AgentEvent>
) {
    // 1. Split the TCP stream into read and write halves
    let (read_half, _write_half) = socket.into_split();
    
    // Note: The write_half would be passed to a separate task
    // wrapped in FramedWrite for your outbound JSON.

    // 2. Wrap the raw byte stream in a FramedRead with LinesCodec
    let mut framed_reader = FramedRead::new(read_half, LinesCodec::new());

    // 3. Asynchronously iterate over fully-formed lines
    while let Some(line_result) = framed_reader.next().await {
        match line_result {
            Ok(line) => {
                // We have a guaranteed complete line of text. Time to parse!
                handle_incoming_json(&line, &event_tx).await;
            }
            Err(e) => {
                eprintln!("Error reading from TCP stream: {}", e);
                break; // Exit the task if the socket closes or errors
            }
        }
    }
    
    println!("Agent disconnected. TCP Read Task shutting down.");
}

```

### 3. Parsing and Dispatching the JSON-RPC

Now that `LinesCodec` is doing the heavy lifting of buffering and splitting the bytes, your parsing logic becomes incredibly clean. You just deserialize the string into a `serde_json::Value` and route it.

```rust
async fn handle_incoming_json(line: &str, event_tx: &mpsc::Sender<AgentEvent>) {
    // Attempt to parse the line as JSON
    let payload: Value = match serde_json::from_str(line) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Received malformed JSON: {} | Payload: {}", e, line);
            return;
        }
    };

    // Extract the JSON-RPC method (if it exists)
    if let Some(method) = payload.get("method").and_then(|m| m.as_str()) {
        match method {
            "window/showMessageRequest" => {
                let request_id = payload["id"].to_string();
                let message = payload["params"]["message"].as_str().unwrap_or("Action requested");
                
                // Fire the event to the Slack loop
                let event = AgentEvent::ClearanceRequested {
                    request_id,
                    session_id: "active_session".to_string(), // In reality, fetch from state
                    action_type: ActionType::TerminalCommand("Pending...".into()),
                    description: message.to_string(),
                    diff: None,
                };
                
                let _ = event_tx.send(event).await;
            }
            "$/progress" | "window/logMessage" => {
                // Route streaming text updates to Slack
                let message = payload["params"]["message"].as_str().unwrap_or("");
                let _ = event_tx.send(AgentEvent::StatusUpdated {
                    session_id: "active_session".to_string(),
                    message: message.to_string(),
                }).await;
            }
            _ => {
                // Ignore other JSON-RPC methods we don't care about
            }
        }
    }
}

```

### A Crucial Caveat: LSP Framing

*Pro-Tip:* While many raw ACP implementations use simple newline-delimited JSON (where `LinesCodec` works perfectly), because ACP evolved from the Language Server Protocol (LSP), some agents require **LSP Framing**.

LSP Framing looks like an HTTP header prepended to the JSON:
`Content-Length: 123\r\n\r\n{"jsonrpc":"2.0"...}`

If you point `LinesCodec` at Copilot CLI and see it panicking because it is reading `Content-Length: ...` instead of raw JSON, you will simply swap `LinesCodec` for a custom or community-provided `LanguageServerCodec` (found in crates like `tower-lsp` or easily written manually). The `FramedRead` architecture remains exactly the same!

- Also need the ability configure dynamically the level of detail being returned in Slack messages (T011) so that we can avoid hitting Slack's message size limits with large diffs or verbose agent reasoning. This will be a simple enum (e.g., `DetailLevel::None`, `DetailLevel::Low`, `Medium`, `High`) that the user can set in the config file or via a slash command. The `AgentDriver` implementations will check this setting before sending events to Slack and truncate or summarize the content accordingly.
- Also need to ensure that when running in ACP mode, the server can handle multiple concurrent agent sessions without mixing up their messages or approvals. This will require tagging each `AgentEvent` with a `session_id` and ensuring that the Slack messages and approval buttons are also tagged with this ID so that responses can be correctly routed back to the right agent session.  This extends to different workspaces working in different channels as well. The `session_id` can be a UUID generated at the start of each session and included in all events and Slack interactions related to that session.  This way, even if multiple agents are active at the same time, their messages and approvals will never get mixed up.  The key concept to apply here is that agent-intercom is a multi-workspace, multi-session controller, and all events and interactions must be properly namespaced by session and workspace to maintain order and clarity.  Different workspaces will be using different channel ids, so each workspace should route to its own dedicated channel for approvals and updates, and the `session_id` will ensure that even within a busy channel, messages from different sessions are not confused.  Messages pushed by the operator from a dedicated channel should automatically route to the correct workspace associated session as well.  This will require maintaining a mapping of `session_id` to `workspace_id` and `channel_id` in your SQLite database, and ensuring that all events and Slack interactions include the correct identifiers to route them properly.
- Also need the ability for the operator to attach files or screenshots/images to the Slack communication channel to the agent in ACP mode, and for those attachments to be properly linked to the agent session and visible in the Slack channel. This will involve implementing a new tool call (e.g., `attach_file`) that the agent can invoke with a file path or image data, which your server will then upload to Slack using the Web API and post in the appropriate channel with a reference to the session. The operator should also be able to manually upload files to the channel, and those files should be tagged with the session ID so that they are associated with the correct agent session. This will enhance the communication capabilities of your intercom system, allowing for richer interactions between the agent and the operator, especially when dealing with complex tasks that may require visual aids or file references.  Basically, I may want to start a session by first writing a rather longer set of requirements that I need to upload to the agent for it to use, for example, as part of a spec-kit planning session prior to build.  This would be a fundamental requirement to enable a true "human-in-the-loop" workflow where the operator can provide detailed instructions, reference materials, or feedback in a more flexible way than just text messages, and the agent can also share its own files or screenshots as part of its reasoning or output.  This will require careful handling of file uploads and metadata to ensure that everything is properly linked to the correct session and workspace context.
- Also want to refactor the current implementation of the server that currently consumes the channel_id on the querystring configured in the mcp.json file.  Instead, the querystring should contain a workspace_id or namespace name that is specific and unique to the current workspace/working directory (cwd).  The channel_id should be instead configured in the config.toml for pickup by the server at runtime.  This way, we can support multiple workspaces with different channel_ids without having to run multiple instances of the server or hardcode channel_ids in the querystring.  The server can maintain a mapping of workspace_id to channel_id in memory or in SQLite, and route messages accordingly based on the workspace_id provided in the querystring.  This will make the system more flexible and scalable, allowing it to support multiple workspaces with different Slack channels seamlessly.  It also aligns better with the concept of workspaces being the primary namespace for sessions and interactions, rather than relying on channel_ids which are more of an implementation detail of the Slack integration. Additions to the config.toml file should be picked up at runtime and should allow for dynamic reloading without needing to restart the server, so that changes to channel_ids or workspace mappings can take effect immediately.
- If the agent is offline or disconnected or hung in a process, the inbox queue mechanism should be able to queue messages for the agent once it comes back online, and the operator should be able to see in the Slack channel that the agent is currently offline and that messages are queued for it.  This will require implementing a message queue in SQLite that holds messages for each session when the agent is not connected, and then automatically flushes those messages to Slack when the agent reconnects.  The Slack messages should indicate that they are queued messages for an offline agent, and once the agent is back online, the system should update the status in Slack to reflect that the agent is now connected and processing messages.  This will enhance the robustness of your intercom system, allowing it to handle disconnections gracefully without losing important messages or context.  It also provides a better user experience for the operator, who can see the status of the agent and understand that messages are being queued rather than just disappearing into the void when the agent is offline.  This will require careful handling of agent connection status and message routing to ensure that messages are not lost and that the operator is always informed about the state of the agent.  This also ties into the stall detection mechanism, where if an agent is detected as stalled, the system can automatically mark it as offline and start queuing messages until it comes back online or is manually restarted by the operator.  In addition, this must be enabled for both MCP and ACP modes, ensuring that the queuing mechanism works regardless of the underlying protocol, and that the operator's experience in Slack remains consistent whether the agent is connected via MCP or ACP.  Note that the Inbox queue is already a feature in the the 004 feature set currently under development, so this requirement is really about ensuring that the queuing mechanism is properly integrated with the agent connection status and the Slack communication flow, rather than being a completely new feature.  The key is to make sure that when an agent goes offline or is detected as stalled, the system automatically starts queuing messages for that agent, and then flushes those messages to Slack once the agent is back online, while keeping the operator informed about the status throughout the process.
- When a session is running in ACP or MCP mode, the session itself should start a new thread in Slack and all messages on that thread should be effectively part of that thread, like a reply chain to the first thread in Slack.  This will allow the operator to more easily track all activity for a given session, start new sessions as separate threads, and keep the communication organized.  The initial message that starts the session in Slack should be the root of the thread, and all subsequent messages related to that session (status updates, clearance requests, operator replies) should be posted as replies to that thread.  This will require maintaining the Slack thread timestamp (ts) for each session in your SQLite database, and ensuring that all messages sent to Slack for that session include the correct `thread_ts` parameter to post them in the right thread.  This also enhances the user experience for the operator, who can easily see all interactions related to a specific session grouped together in a single thread, rather than having messages interleaved with other sessions or activities in the channel.  This is especially important when multiple sessions are active at the same time, as it allows for clear separation and organization of communication for each session.
- Does it make more sense for me to have separate bots in Slack for MCP and ACP modes, or should I have a single bot that can handle both modes and route messages accordingly?  This is an architectural decision that depends on how distinct the communication patterns and message formats are between MCP and ACP.  If the messages and interactions for MCP and ACP are significantly different, it might make sense to have separate bots to keep things clean and organized.  However, if there is a lot of overlap in the types of messages and interactions, a single bot with proper routing logic could be more efficient and easier to maintain.  The key is to ensure that whichever approach you choose, the operator's experience in Slack remains seamless and intuitive, without having to worry about which bot they are interacting with.  If you go with a single bot, you can use the `session_id` or `workspace_id` to route messages appropriately within the bot's logic, ensuring that messages related to MCP sessions are handled differently from those related to ACP sessions if needed.  Ultimately, the decision should be guided by the principle of keeping the system as simple as possible while still meeting all functional requirements effectively. Note that I don't think, as an operator myself, that interacting with different bots as separate threads in Slack would be that bad, and it might actually help to visually distinguish between MCP and ACP sessions in the Slack interface.  However, it does add some overhead in terms of managing multiple bot tokens and ensuring that both bots are properly configured and running.  If the communication patterns are similar enough, a single bot with clear message formatting to indicate whether it's an MCP or ACP session might be the best balance of simplicity and functionality.

