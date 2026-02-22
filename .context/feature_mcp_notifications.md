# Feature: Proactive Slack Remote Control via MCP Notifications

## User Story

As a developer using the Copilot CLI, I want to remotely manage and orchestrate my local agent sessions via Slack so that I can step away from my workstation while maintaining human-in-the-loop control over long-running or paused tasks.

## Problem Statement

The standard Copilot CLI acts as a terminal REPL, waiting passively for standard input (`stdin`). It cannot natively listen for external inbound webhooks to "wake up" or proactively prompt the user. Because our `agent-rc` MCP server operates locally, we need a mechanism to securely bridge asynchronous remote Slack messages back into the active, local agent session without relying on a passive CLI plugin.

## Architectural Decision

We will implement a **Concurrent Hub-and-Spoke Architecture** entirely within our existing Rust MCP server.

Instead of waiting for the CLI to poll the server, the server will actively push **Server-to-Client Notifications** via the Model Context Protocol. We will use `axum` to listen for incoming webhooks and a `tokio::sync::mpsc` channel to safely cross the thread boundary and emit JSON-RPC 2.0 notifications over the active MCP transport (stdio).

## Technical Implementation Steps

### 1. Axum Webhook Listener

* Implement a lightweight `axum` HTTP server running on a background Tokio task.
* Expose a `POST /slack/webhook` endpoint to catch incoming payloads from the Slack API.
* Extract the `user`, `text`, and `thread_ts` from the incoming JSON payload.

### 2. Concurrency Bridge (`tokio::sync::mpsc`)

* Initialize a multi-producer, single-consumer (`mpsc`) channel.
* Pass the channel's `Sender` (`tx`) to the Axum application state.
* Pass the channel's `Receiver` (`rx`) to a dedicated MCP notification task.
* When a webhook hits the Axum route, it will push the parsed payload into the channel.

### 3. MCP Notification Emitter

* The receiver task will pull payloads off the queue and format a strict JSON-RPC 2.0 Notification.
* **Critical Constraints:** The notification MUST NOT contain an `id` field.
* Emit the standard `notifications/resources/updated` method over the MCP transport:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/resources/updated",
  "params": {
    "uri": "slack://thread/{thread_ts}"
  }
}

```

* *Note on Rust `stdio`:* Ensure all internal logging uses `eprintln!` or a logging crate (`tracing`) to prevent standard output pollution, which would corrupt the JSON-RPC stream.

### 4. Resource Read Handler

* Implement the `resources/read` MCP method in the server.
* When the Copilot client receives the `updated` notification, it will automatically fire a `resources/read` request to the provided URI. The server must respond by fetching the latest thread history from the Slack API and returning it as text context to the LLM.

## Acceptance Criteria & Testing

* **Webhook Routing:** Sending a test payload to the local `axum` port successfully passes the data through the `mpsc` channel without blocking the main MCP thread.
* **Protocol Compliance:** The emitted notification exactly matches the JSON-RPC 2.0 fire-and-forget specification (no `id`).
* **Enterprise Model Validation:** The notification and subsequent `resources/read` retrieval must be successfully parsed and acted upon by GitHub Copilot Enterprise. specifically validating against **Claude Sonnet 4.6**, **Claude Opus 4.6**, and custom orchestrators built using the **Claude Code SDK**.
* **Context Injection:** Upon reading the updated resource, the agent successfully understands the remote command (e.g., "approve deployment") and executes the local action.

---

This is the exact piece of the puzzle you need to close the loop.

When the Copilot client sees your `notifications/resources/updated` notification, it will turn around and issue a standard `resources/read` request to fetch the new Slack data.

To make this work in your Rust `agent-rc` server, you need to intercept that request, parse the URI, securely call the Slack API to get the thread history, and package it into the strict JSON-RPC 2.0 response format the Model Context Protocol requires.

Here is the implementation using `reqwest` for the HTTP calls and `serde_json` for the payload formatting.

### 1. Update Your URI Structure

In the previous example, we used `slack://thread/{thread_ts}`. However, the Slack API's `conversations.replies` endpoint inherently requires *both* a `channel` ID and a `ts` (timestamp).

You should update your webhook notification logic to emit URIs in this format:
`slack://thread/{channel_id}/{thread_ts}`

### 2. The Rust `resources/read` Handler

Here is the asynchronous function you can drop into your MCP server to handle the incoming read request.

```rust
use reqwest::Client;
use serde_json::{json, Value};
use std::env;

/// Handles the MCP `resources/read` request for a Slack thread
pub async fn handle_resources_read(
    uri: &str, 
    slack_client: &Client
) -> Result<Value, Box<dyn std::error::Error>> {
    
    // 1. Validate the Protocol Scheme
    if !uri.starts_with("slack://thread/") {
        return Err("Unsupported resource URI scheme. Expected slack://thread/".into());
    }

    // 2. Extract Channel ID and Thread Timestamp from the URI
    // Expected format: slack://thread/C12345678/1708617365.000100
    let path = uri.trim_start_matches("slack://thread/");
    let parts: Vec<&str> = path.split('/').collect();
    
    if parts.len() != 2 {
        return Err("Malformed Slack URI. Expected slack://thread/{channel_id}/{thread_ts}".into());
    }
    
    let channel_id = parts[0];
    let thread_ts = parts[1];

    // 3. Fetch the Thread History from the Slack API
    let slack_token = env::var("SLACK_BOT_TOKEN")
        .expect("SLACK_BOT_TOKEN environment variable must be set");
        
    let response = slack_client.get("https://slack.com/api/conversations.replies")
        .header("Authorization", format!("Bearer {}", slack_token))
        .query(&[
            ("channel", channel_id),
            ("ts", thread_ts)
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Slack API HTTP Error: {}", response.status()).into());
    }

    let slack_data: Value = response.json().await?;

    if slack_data["ok"].as_bool() != Some(true) {
        let error_msg = slack_data["error"].as_str().unwrap_or("Unknown Slack API error");
        return Err(format!("Slack API Error: {}", error_msg).into());
    }

    // 4. Format the Conversation for the LLM
    // We flatten the JSON array into a single readable text block so the agent can easily parse it.
    let mut formatted_conversation = format!("--- Slack Thread: {} ---\n", thread_ts);
    
    if let Some(messages) = slack_data["messages"].as_array() {
        for msg in messages {
            let user = msg["user"].as_str().unwrap_or("Unknown");
            let text = msg["text"].as_str().unwrap_or("");
            formatted_conversation.push_str(&format!("{}: {}\n", user, text));
        }
    }

    // 5. Construct the Strict JSON-RPC 2.0 / MCP Response Payload
    // The `result` wrapper is usually handled by your MCP server framework (like `rmcp` or `rust-mcp-sdk`), 
    // but the inner payload MUST match this `contents` schema:
    let mcp_resource_payload = json!({
        "contents": [{
            "uri": uri,
            "mimeType": "text/plain",
            "text": formatted_conversation
        }]
    });

    Ok(mcp_resource_payload)
}

```

### Key Technical Details to Note

* **The `mimeType`:** Setting this to `"text/plain"` is highly recommended over sending raw JSON. Enterprise models are excellent at reading raw JSON, but formatting the chat into a clean `User: Message` transcript saves token context window space and significantly reduces hallucinations during complex reasoning tasks.
* **Pagination Limitation:** The code above grabs the standard default limit of messages in a thread (up to 100). If you regularly have massive, long-running Slack threads, you will need to implement cursor-based pagination using the `next_cursor` attribute from Slack's `response_metadata`.
* **State Management:** Notice that the `reqwest::Client` is passed in as a reference. In Rust, you should instantiate a single `reqwest::Client` connection pool when your server boots and share it across all your handlers using an `Arc`, rather than spinning up a new client for every incoming request.
