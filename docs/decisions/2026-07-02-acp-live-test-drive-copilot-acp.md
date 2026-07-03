---
type: findings
date: 2026-07-02
linked_work_item: "013.004.002-T"
feeds: "013.004.001-T (ADR-0016)"
tags:
  - acp
  - wire-protocol
  - conformance
  - live-test
---

# F.1-T2 — Live test-drive of a real ACP agent (`copilot --acp`)

Validates the F.1-T1 gap analysis (`2026-07-02-acp-wire-protocol-gap-analysis.md`)
against a **real, conformant** Agent Client Protocol agent. GitHub Copilot CLI
1.0.68/1.0.69 (`C:\Tools\copilot.exe`) exposes `--acp` ("Start as Agent Client
Protocol server"), so it serves as a live reference peer.

## Method

A minimal Node harness spawned `copilot --acp` over stdio and exchanged
newline-delimited JSON-RPC 2.0 messages: one standard `initialize`, then a set
of bespoke agent-intercom methods, capturing every response.

## Results (raw, abridged)

**1. Framing** — responses are newline-delimited JSON (message terminates in
`\n`, no `Content-Length` header). agent-intercom's NDJSON framing (`src/acp/codec.rs`)
is therefore compatible at the transport layer.

**2. Standard `initialize` succeeds** — request:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":false}}}
```

response (JSON-RPC `result`, id-correlated):

```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"mcpCapabilities":{"http":true,"sse":true},"promptCapabilities":{"image":true,"audio":false,"embeddedContext":true},"sessionCapabilities":{"list":{}}},"agentInfo":{"name":"Copilot","version":"1.0.68"},"authMethods":[{"id":"copilot-login", ...}]}}
```

**3. Bespoke methods are rejected** — a real agent returns JSON-RPC
`-32601 Method not found`, id-correlated, for each:

| Sent method | Response |
|---|---|
| `prompt/forward` | `{"id":2,"error":{"code":-32601,"message":"Method not found: prompt/forward"}}` |
| `clearance/response` | `{"id":3,"error":{"code":-32601,"message":"Method not found: clearance/response"}}` |
| `session/interrupt` | `{"id":4,"error":{"code":-32601,"message":"Method not found: session/interrupt"}}` |

**4. `session/new` returned nothing** within the probe window — consistent with
the agent gating session creation behind the advertised `authMethods`
(`authenticate` handshake), which agent-intercom does not perform.

## Confirmation of the gap analysis

| T1 prediction | T2 live evidence | Verdict |
|---|---|---|
| Standard `initialize` uses `protocolVersion` + `clientCapabilities`; bespoke sends `processId`/`workspaceFolders`, omits `clientCapabilities` | Real agent returns `agentCapabilities`/`agentInfo`/`authMethods` from the standard shape | **Confirmed** — bespoke `initialize` is non-conformant |
| Replies must be JSON-RPC `result`/`error`, not method-messages reusing the id | All responses are id-correlated `result`/`error` objects | **Confirmed** — our reply-as-method handling would break |
| `session/interrupt` diverges from standard `session/cancel` | `session/interrupt` → `-32601`; `session/cancel` (notification) accepted silently | **Confirmed** |
| `prompt/forward`, `clearance/*` have no ACP equivalent | Both → `-32601 Method not found` | **Confirmed** — bespoke-only, non-interoperable |
| `authenticate` unimplemented | Agent advertises `authMethods`; `session/new` blocked | **Confirmed** — auth handshake required |

## Conclusion

A real, conformant ACP agent **cannot interoperate** with agent-intercom's
current bespoke wire dialect: the agent's `initialize` shape differs, its replies
are JSON-RPC results (which our reader mishandles), it rejects our bespoke
methods outright, and it requires an auth handshake we do not perform. This is
decisive input to ADR-0016 (conform vs formalize) → **conform**.

## Open items for implementation (not blockers for the decision)

- Perform the `authenticate` handshake (honor advertised `authMethods`) before
  `session/new` — required to drive Copilot/Claude/Gemini agents.
- Consume `agentCapabilities` from the `initialize` result to gate feature use.
- A future live run *with auth completed* should observe a real
  `session/request_permission` and `session/update` stream end-to-end.
