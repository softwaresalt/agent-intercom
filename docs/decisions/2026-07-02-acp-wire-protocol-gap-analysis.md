---
title: "ACP wire-protocol gap analysis: bespoke NDJSON dialect vs public Agent Client Protocol"
type: findings
date: 2026-07-02
linked_work_item: "013.004.003-T"
tags:
  - acp
  - wire-protocol
  - conformance
---

## Goal

Map every wire element of agent-intercom's bespoke NDJSON ACP dialect to the
public Agent Client Protocol (ACP) standard, so the F.1 "conform-vs-formalize"
ADR (task T3) can decide on a firm evidence base. This document does **not**
recommend a direction; it only records what exists, where, and how it compares.

Public-standard claims are grounded in the ACP v1 schema
(`agentclientprotocol.com/protocol/v1/schema.md`, retrieved 2026-07-02).
Every bespoke claim cites `file:line` in this repository.

## Scope of the current implementation

The bespoke dialect lives entirely under `src/acp/` plus the outbound driver at
`src/driver/acp_driver.rs`. The server acts as the ACP **client** (it spawns and
drives the agent); the spawned process (for example `copilot --acp`) is the ACP
**agent**. Direction below is stated from that frame: client→agent (server sends)
or agent→client (server receives).

## Method gap table

| Bespoke element | File:line | Public ACP equivalent | Match class | Notes |
|---|---|---|---|---|
| NDJSON line framing (`\n`-delimited, 1 MiB/line cap) | `src/acp/codec.rs:24`, `src/acp/codec.rs:58` | ndjson-over-stdio framing (JSON-RPC 2.0) | Exact match | ACP uses newline-delimited JSON over stdio, not LSP `Content-Length` headers. Framing conforms. The `mod.rs:11` doc comment's "LSP-style" label refers to the `initialize`/`initialized` handshake, not the framing. |
| `initialize` request (client→agent) | `src/acp/handshake.rs:91`–`106` | `initialize` | Semantic match, different shape | Method name and `protocolVersion`/`clientInfo` match. Params add non-standard `processId` and `workspaceFolders` (LSP-style) and **omit** `clientCapabilities`, so the agent sees the default `{fs:false,terminal:false}`. |
| `initialized` notification (client→agent) | `src/acp/handshake.rs:156`–`159` | *(none)* | No equivalent | ACP has no `initialized` notification; the `initialize` response alone completes negotiation. This is a bespoke/LSP-carryover step. |
| `session/new` request (client→agent) | `src/acp/handshake.rs:195`–`203` | `session/new` | Exact match | Params `cwd` + `mcpServers` match the required standard fields; response `sessionId` is read at `handshake.rs:216`. Standard optional `additionalDirectories` is unused. |
| `session/prompt` request (client→agent) | `src/acp/handshake.rs:258`–`268`; `src/driver/acp_driver.rs:294`–`304`; `src/driver/acp_driver.rs:427`–`437` | `session/prompt` | Exact match | Params `sessionId` + `prompt: [{type:"text",text}]` match `PromptRequest` with a `ContentBlock::Text`. Reused for initial prompt, operator steering, and standby resume. |
| `session/interrupt` request (client→agent) | `src/driver/acp_driver.rs:331`–`334` | `session/cancel` | Semantic match, different name/shape | Standard `session/cancel` is a **notification** carrying `sessionId`. Bespoke names it `session/interrupt`, sends it as a request-shaped object, **omits `sessionId`** (relies on per-stream identity), and adds a `reason` string. |
| `clearance/request` (agent→client) | `src/acp/reader.rs:232`, `src/acp/reader.rs:542`–`561` | `session/request_permission` | Semantic match, different name/shape | Standard params are `sessionId` + `toolCall` + `options[]`. Bespoke params are `title`, `description`, `diff`, `file_path`, `risk_level` — a purpose-built approval payload with no standard analogue. |
| `clearance/response` (client→agent) | `src/driver/acp_driver.rs:245`–`252` | `session/request_permission` **result** | Semantic match, different shape | Standard answers with a JSON-RPC **response object** (`{outcome}`). Bespoke instead emits a **new method-bearing message** `{"method":"clearance/response","id":<request_id>,"params":{status,reason}}`, reusing the request `id` for correlation. |
| `status/update` (agent→client) | `src/acp/reader.rs:233`, `src/acp/reader.rs:565`–`572` | `session/update` (`agent_message_chunk`) | Semantic match, different name/shape | Bespoke `params.message` is a plain string surfaced to Slack. The standard carries the same intent inside `session/update.update` as a structured `SessionUpdate`. |
| `prompt/forward` (agent→client) | `src/acp/reader.rs:234`, `src/acp/reader.rs:576`–`592` | *(none)* | No equivalent | Agent-initiated question/continuation to the operator (`params.text`, `params.type`). ACP has no agent→client "ask the human a free-form question" method distinct from permission requests. |
| `prompt/response` (client→agent) | `src/driver/acp_driver.rs:378`–`385` | *(none)* | No equivalent | Reply to `prompt/forward` (`params.decision`, `params.instruction`), correlated by reused `id`. No standard counterpart. |
| `heartbeat` (agent→client) | `src/acp/reader.rs:235`, `src/acp/reader.rs:596`–`603` | *(none)* | No equivalent | Liveness/progress ping carrying optional `progress[]`. ACP relies on `session/update` streaming and prompt `stopReason` for progress; there is no heartbeat method. |
| `session/update` notification (agent→client) | `src/acp/reader.rs:238`, `src/acp/reader.rs:615`–`648` | `session/update` | Exact match (partial consumption) | Name and shape (`params.update.sessionUpdate`) match the standard `SessionNotification`. Bespoke only extracts `agent_message_chunk` text; `tool_call`, `tool_call_update`, `agent_thought_chunk`, plans, etc. are parsed then dropped. |
| JSON-RPC result/error handling | `src/acp/handshake.rs:338`–`356`; `src/acp/reader.rs:225`–`229` | JSON-RPC 2.0 responses | Exact match | Handshake matches responses by `id` and treats an `error` object as failure. The reader skips method-less result messages. |
| String correlation IDs `intercom-{purpose}-{uuid}` | `src/acp/handshake.rs:54`–`56`; `src/driver/acp_driver.rs:292`, `src/driver/acp_driver.rs:425` | JSON-RPC `id` (string permitted) | Exact match | Format is bespoke but JSON-RPC 2.0 allows string IDs, so this is compatible. |
| `seq` field stamped on every outbound message | `src/acp/writer.rs:82`–`91` | *(none)* | No equivalent | A monotonic per-session counter injected into every outbound object. Not part of JSON-RPC 2.0 or ACP. |

**Tally:** 6 exact matches, 5 semantic matches (different name or shape),
5 bespoke elements with no standard equivalent — 16 wire elements total.

### Standard methods the bespoke dialect does not implement

These are gaps of omission rather than table rows, but they matter for the ADR:

- **Client methods never handled:** `fs/read_text_file`, `fs/write_text_file`,
  `terminal/create`, `terminal/kill`, `terminal/output`, and the other
  `terminal/*` calls. A conformant agent that emits these would receive no
  response. Bespoke omits `clientCapabilities` in `initialize`
  (`src/acp/handshake.rs:95`–`104`), so the agent defaults these capabilities to
  `false` and *should* avoid calling them — but this is only safe because the
  handshake never advertises them.
- **Agent methods never sent or handled:** `authenticate`, `logout`,
  `session/load`, `session/resume`, `session/set_mode`,
  `session/set_config_option`.
- **Standard permission flow not consumed:** the reader has no branch for
  `session/request_permission` (`src/acp/reader.rs:231`–`250`), so a real
  agent's standard permission request is treated as an unknown method and
  silently dropped at `src/acp/reader.rs:243`–`249`.

## Handshake & framing differences

**Framing conforms.** Both directions use `LinesCodec`-based NDJSON with a
1 MiB per-line limit (`src/acp/codec.rs:24`, `src/acp/codec.rs:57`–`59`;
outbound newline append at `src/acp/writer.rs:99`–`100` and
`src/acp/handshake.rs:393`). ACP's transport is newline-delimited JSON over
stdio, so no `Content-Length` header divergence exists.

**Handshake sequence diverges in one step.** The bespoke startup is:

1. `initialize` request → `src/acp/handshake.rs:81`–`116`.
2. Wait for the `initialize` result by `id` → `src/acp/handshake.rs:138`–`145`,
   `297`–`383`.
3. `initialized` notification → `src/acp/handshake.rs:155`–`169`.
4. `session/new` request → `src/acp/handshake.rs:184`–`232`.
5. `session/prompt` request → `src/acp/handshake.rs:246`–`283`.

Steps 1, 2, 4, and 5 align with the standard `initialize` → `session/new` →
`session/prompt` sequence. Step 3 (`initialized`) has **no standard
counterpart** — ACP considers the connection established once the `initialize`
response arrives. The inbound path also tolerates a stray `initialized` message
as a no-op (`src/acp/reader.rs:239`–`242`).

**`initialize` params differ.** Bespoke sends `protocolVersion: 1`,
`processId`, `clientInfo`, and `workspaceFolders`
(`src/acp/handshake.rs:95`–`105`). The standard `InitializeRequest` defines
`protocolVersion`, `clientCapabilities`, and optional `clientInfo`; it has no
`processId` or `workspaceFolders`. Bespoke therefore sends two extra
LSP-style fields and omits the capabilities object.

## Correlation, streaming & error conventions

- **Correlation model splits into two styles.** Requests the server originates
  (`initialize`, `session/new`, `session/prompt`) use JSON-RPC-style `id` and
  await a proper result object (`src/acp/handshake.rs:338`–`356`). But replies
  the server sends **to agent-originated requests** (`clearance/response`,
  `prompt/response`) are emitted as **new method-bearing messages that reuse the
  original request `id`** (`src/driver/acp_driver.rs:245`–`252`,
  `378`–`385`), not as JSON-RPC response objects with `result`/`error`. This is
  the sharpest divergence from JSON-RPC 2.0 correlation semantics.
- **`jsonrpc` field is inconsistent.** `session/prompt` and the handshake
  messages include `"jsonrpc":"2.0"` (`src/acp/handshake.rs:92`,
  `src/driver/acp_driver.rs:295`), while `clearance/response`
  (`src/driver/acp_driver.rs:245`), `session/interrupt`
  (`src/driver/acp_driver.rs:331`), and `prompt/response`
  (`src/driver/acp_driver.rs:378`) omit it.
- **`seq` is added to everything outbound.** The writer stamps a monotonic
  `seq` on every message before serialisation
  (`src/acp/writer.rs:82`–`91`). This is a non-standard extension field.
- **Inbound envelope is permissive.** `method`, `id`, and `params` are all
  optional, with `params` defaulting to `null`
  (`src/acp/reader.rs:125`–`135`). Method-less messages are treated as
  JSON-RPC results and skipped (`src/acp/reader.rs:226`–`229`).
- **`session/interrupt` cannot target a session by ID.** It omits `sessionId`
  and relies on per-stream identity (`src/driver/acp_driver.rs:331`–`334`),
  whereas standard `session/cancel` requires `sessionId`.
- **Error convention matches on the request side.** A JSON-RPC `error` object
  with a matching `id` is treated as a handshake/method failure
  (`src/acp/handshake.rs:345`–`350`). Inbound rate limiting and codec framing
  errors are handled separately (`src/acp/reader.rs:327`–`373`) and are not
  protocol-level concerns.
- **Prompt turn completion is not consumed.** The standard `session/prompt`
  returns a `PromptResponse` with a `stopReason`; the bespoke reader has no
  handler for the prompt result and relies on `session/update` streaming plus
  process exit for progress and termination
  (`src/acp/reader.rs:231`–`250`, `src/acp/spawner.rs:179`–`223`).

## Implications for conform-vs-formalize

Stated neutrally, per gap, as the cost each direction would carry. This section
takes no position.

- **Permission flow (`clearance/*` vs `session/request_permission`).**
  *Conform:* replace the `clearance/request` reader branch with a
  `session/request_permission` handler, map `toolCall`/`options` to the
  approval UI, and answer with a JSON-RPC **result object** carrying `outcome`
  instead of a `clearance/response` method message. *Formalize:* document
  `clearance/request`/`clearance/response`, the `title`/`diff`/`file_path`/
  `risk_level` payload, and the method-message reply convention as the
  intended contract, and require agents to speak it.
- **Reply correlation (method-message vs JSON-RPC result).**
  *Conform:* change `resolve_clearance` and `resolve_prompt` to emit JSON-RPC
  response objects keyed by the request `id`
  (`src/driver/acp_driver.rs:245`–`252`, `378`–`385`). *Formalize:* specify
  the "reply is a new method message that reuses the request `id`" rule and the
  optional `jsonrpc` field as part of the dialect.
- **Cancellation (`session/interrupt` vs `session/cancel`).**
  *Conform:* rename to `session/cancel`, send as a notification, and include
  `sessionId`. *Formalize:* keep `session/interrupt` with `reason` and document
  the per-stream targeting assumption.
- **Handshake (`initialized`, extra `initialize` params).**
  *Conform:* drop the `initialized` notification, remove `processId`/
  `workspaceFolders`, and add a `clientCapabilities` object. *Formalize:*
  document the LSP-style handshake, including `initialized`, as required.
- **Bespoke-only methods (`prompt/forward`, `prompt/response`, `heartbeat`,
  `status/update`).** *Conform:* re-express operator questions through the
  permission/mode surfaces, replace `status/update` with `session/update`
  consumption, and drop `heartbeat` in favour of streaming/stop-reason liveness.
  *Formalize:* register these as named extensions (potentially under the ACP
  `_meta` extensibility mechanism) and document their payloads.
- **Client methods (`fs/*`, `terminal/*`, `authenticate`, `session/load`).**
  *Conform:* implement handlers and advertise the matching capabilities so real
  agents can read/write files and run terminals through the client. *Formalize:*
  continue advertising no capabilities so agents avoid these calls, and document
  the intentional non-support.
- **`seq` field.** *Conform:* remove it (or move it under `_meta`). *Formalize:*
  document `seq` as a required outbound ordering field.

## Open questions for the live test-drive (F.1-T2)

The following must be verified against a real ACP agent (for example
`copilot --acp` or the Zed reference agent) because they cannot be settled from
code alone:

1. **Does the real agent send `session/request_permission`?** If it does,
   bespoke silently drops it (`src/acp/reader.rs:243`–`249`) and no approval
   ever reaches Slack. Confirm whether the agent uses the standard permission
   flow or something else.
2. **What does the agent expect as a permission reply?** A standard agent waits
   for a JSON-RPC **result** to its `session/request_permission` request. Verify
   whether sending a `clearance/response` method message
   (`src/driver/acp_driver.rs:245`–`252`) leaves the agent blocked.
3. **Does the agent accept the non-standard `initialize` params?** Confirm it
   tolerates extra `processId`/`workspaceFolders` and the missing
   `clientCapabilities`, or whether it rejects/negotiates differently.
4. **Is the `initialized` notification harmless or an error?** ACP defines no
   such message; verify the agent ignores it rather than faulting
   (`src/acp/handshake.rs:155`–`169`).
5. **Does the agent respond to `session/interrupt`?** A standard agent listens
   for `session/cancel` (a notification with `sessionId`). Verify whether the
   bespoke request-shaped, `sessionId`-less `session/interrupt` actually stops
   the turn (`src/driver/acp_driver.rs:331`–`334`).
6. **Does the agent request `fs/*` or `terminal/*`?** If capability negotiation
   fails to suppress these, the agent will stall waiting for a client response
   that never comes. Confirm the default-`false` capabilities hold in practice.
7. **What `session/update` variants does the agent actually stream?** Bespoke
   surfaces only `agent_message_chunk` text (`src/acp/reader.rs:629`–`642`);
   verify how much operator-visible content is lost by dropping
   `tool_call`/`tool_call_update`/plan updates.
8. **How is turn completion signalled?** The agent returns a `PromptResponse`
   with a `stopReason` that bespoke never reads. Verify whether ignoring it
   causes missed end-of-turn detection or duplicated prompts.
9. **Does the agent tolerate the extra `seq` field and inconsistent `jsonrpc`
   presence?** Confirm neither trips strict JSON-RPC validation on the agent
   side (`src/acp/writer.rs:82`–`91`; `src/driver/acp_driver.rs:245`, `331`,
   `378`).
10. **Does the agent emit a `heartbeat` or expect one?** Bespoke treats
    `heartbeat` as inbound only (`src/acp/reader.rs:596`–`603`); verify a real
    agent produces liveness signals the stall detector can consume, or whether
    reliance on `heartbeat` leaves ACP agents looking stalled.
