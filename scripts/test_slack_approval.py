"""
scripts/test_slack_approval.py — Drive monocoque-agent-rc to post a Slack approval request.

Steps:
  1. Seed a test "active" session directly into the SQLite DB.
  2. Open an SSE connection to /sse to get the per-connection MCP endpoint.
  3. Complete the MCP initialize handshake via POST.
  4. Call the `ask_approval` tool — the server will enqueue a Slack Block Kit
     message with Accept/Reject buttons, then block waiting for your response.
  5. Script exits after 6 s; the blocking tool call continues server-side until
     you click a button in Slack (or the 3600-second timeout fires).

Usage:
  python scripts/test_slack_approval.py
"""

import datetime
import json
import sqlite3
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid

# ── Configuration ────────────────────────────────────────────────────────────
DB_PATH       = r"D:\Source\GitHub\monocoque-agent-rc\data\agent-rc.db"
BASE_URL      = "http://localhost:3000"
CHANNEL_ID    = "C0AG6S5D87N"  # workspace channel from .vscode/mcp.json
WORKSPACE_ROOT = r"D:\Source\GitHub\monocoque-agent-rc"
OWNER_USER_ID = "U_TESTOP"   # arbitrary placeholder Slack user ID

SAMPLE_DIFF = """\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 #![forbid(unsafe_code)]
 
+// TEST: simulated change injected by approval integration test
 //! `monocoque-agent-rc` — MCP remote agent server binary.
 //!
 //! Bootstraps configuration, starts the MCP transport (HTTP/SSE or stdio),"""

# ── Step 1: Seed a test session into SQLite ──────────────────────────────────
db_session_id = str(uuid.uuid4())
now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"

print(f"[1] Seeding active session {db_session_id} into {DB_PATH}")
try:
    conn = sqlite3.connect(DB_PATH)
    conn.execute(
        """
        INSERT INTO session
            (id, owner_user_id, workspace_root, status, prompt, mode,
             created_at, updated_at, nudge_count, stall_paused)
        VALUES (?, ?, ?, 'active', 'Test approval flow', 'remote', ?, ?, 0, 0)
        """,
        (db_session_id, OWNER_USER_ID, WORKSPACE_ROOT, now, now),
    )
    conn.commit()
    conn.close()
    print(f"    DB session row created OK")
except sqlite3.Error as exc:
    print(f"[!] SQLite error: {exc}")
    sys.exit(1)

# ── Step 2: Connect to SSE and capture the per-connection message endpoint ───
mcp_endpoint: list[str] = []           # mutable container for cross-thread sharing
endpoint_ready = threading.Event()


def sse_reader() -> None:
    """Background thread: opens /sse, parses the 'endpoint' event, prints others."""
    req = urllib.request.Request(
        f"{BASE_URL}/sse?channel_id={CHANNEL_ID}",
        headers={"Accept": "text/event-stream", "Cache-Control": "no-cache"},
    )
    try:
        resp = urllib.request.urlopen(req, timeout=120)
        current_event: str | None = None
        for raw in resp:
            line = raw.decode("utf-8").rstrip("\r\n")
            if line.startswith("event:"):
                current_event = line[6:].strip()
            elif line.startswith("data:"):
                data = line[5:].strip()
                print(f"    [SSE] event={current_event!r}  data={data}")
                if not mcp_endpoint:
                    # rmcp SseServer sends the post path in the first data line
                    mcp_endpoint.append(data)
                    endpoint_ready.set()
            elif line == "":
                current_event = None
    except Exception as exc:  # noqa: BLE001
        print(f"    [SSE] connection ended: {exc}")
        endpoint_ready.set()   # unblock waiter so we don't hang forever


print("\n[2] Opening SSE connection to /sse …")
sse_thread = threading.Thread(target=sse_reader, daemon=True, name="sse-reader")
sse_thread.start()

endpoint_ready.wait(timeout=10)
if not mcp_endpoint:
    print("[!] Timed out waiting for SSE endpoint event — is the server running on port 3000?")
    sys.exit(1)

raw_endpoint = mcp_endpoint[0]
post_url = f"{BASE_URL}{raw_endpoint}" if raw_endpoint.startswith("/") else raw_endpoint
print(f"    MCP POST url: {post_url}")


# ── Step 3: MCP helper ───────────────────────────────────────────────────────
def mcp_post(payload: dict) -> None:
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        post_url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        resp = urllib.request.urlopen(req, timeout=15)
        body = resp.read().decode("utf-8")
        if body.strip():
            print(f"    -> HTTP {resp.status}: {body[:300]}")
        else:
            print(f"    -> HTTP {resp.status} (empty body — response comes via SSE)")
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8")
        print(f"    -> HTTP {exc.code}: {body[:300]}")
    except Exception as exc:  # noqa: BLE001
        print(f"    -> error: {exc}")


# ── Step 4: MCP initialize handshake ─────────────────────────────────────────
print("\n[3] Sending MCP initialize …")
mcp_post({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "monocoque-test-client", "version": "0.1.0"},
    },
})
time.sleep(0.5)

print("\n[4] Sending notifications/initialized …")
mcp_post({"jsonrpc": "2.0", "method": "notifications/initialized"})
time.sleep(0.5)

# ── Step 5: Call ask_approval ─────────────────────────────────────────────────
# The server posts the Slack Block Kit message immediately, then blocks on a
# oneshot channel waiting for the operator to click Accept/Reject.  We fire
# the POST in a daemon thread so this script does not hang.
print("\n[5] Calling ask_approval …")
print("    The server will post an approval request to your Slack channel.")
print("    Click Accept or Reject in Slack to complete the flow.\n")


def do_approval_call() -> None:
    mcp_post({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "ask_approval",
            "arguments": {
                "title": "Test: Add debug comment to main.rs",
                "description": (
                    "This is a test approval request from the monocoque-agent-rc "
                    "integration test script. Please Accept or Reject it to verify "
                    "the end-to-end Slack approval workflow."
                ),
                "diff": SAMPLE_DIFF,
                "file_path": "src/main.rs",
                "risk_level": "low",
            },
        },
    })


approval_thread = threading.Thread(target=do_approval_call, daemon=True, name="approval-call")
approval_thread.start()

# Give the server time to post to Slack before we exit
print("[*] Waiting 6 s for Slack post to be enqueued …")
time.sleep(6)

print("\n[+] Script complete.")
print(f"    DB session ID : {db_session_id}")
print( "    The ask_approval call is still blocking server-side.")
print( "    Accept or Reject in Slack to resolve it.")
print( "    Or use monocoque-ctl approve/reject with the request ID from server logs.")
print("\n    To clean up the test session when done:")
print(f"    sqlite3 \"{DB_PATH}\" \"UPDATE session SET status='terminated' WHERE id='{db_session_id}';\"")
