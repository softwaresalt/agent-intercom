# run-debug-acp.ps1 — Start agent-intercom in ACP (Agent Communication Protocol) mode.
#
# ACP mode spawns agent CLIs as subprocesses and communicates via NDJSON streams
# instead of the MCP HTTP/SSE transport. MCP transports are disabled in this mode.
#
# Prerequisites:
#   1. Slack credentials set as Windows user-level environment variables
#      (or stored in the OS keychain under service "agent-intercom-acp").
#   2. At least one [[workspace]] entry in config.toml with a channel_id
#      matching the Slack channel where you'll run /session-start.
#   3. host_cli pointing to a valid agent binary in config.toml.
#
# ACP credentials resolve in this order:
#   SLACK_BOT_TOKEN_ACP  → SLACK_BOT_TOKEN  (shared fallback)
#   SLACK_APP_TOKEN_ACP  → SLACK_APP_TOKEN  (shared fallback)
#   SLACK_MEMBER_IDS_ACP → SLACK_MEMBER_IDS (shared fallback)
#
# To run MCP and ACP side-by-side, set the _ACP variants to a separate
# Slack app's tokens. Otherwise the shared vars work fine for single-mode use.

# ── Load ACP-specific credentials (fall back to shared if _ACP not set) ──────

$env:SLACK_APP_TOKEN = (
    [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN_ACP", "User")
) ?? (
    [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
)

$env:SLACK_BOT_TOKEN = (
    [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN_ACP", "User")
) ?? (
    [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
)

$env:SLACK_TEAM_ID = (
    [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID_ACP", "User")
) ?? (
    [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
)

$env:SLACK_MEMBER_IDS = (
    [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS_ACP", "User")
) ?? (
    [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
)

$env:RUST_LOG = "info,agent_intercom::acp=debug,agent_intercom::driver=debug,agent_intercom::slack=debug"

# ── Preflight checks ────────────────────────────────────────────────────────

$missing = @()
if (-not $env:SLACK_APP_TOKEN)  { $missing += "SLACK_APP_TOKEN(_ACP)" }
if (-not $env:SLACK_BOT_TOKEN)  { $missing += "SLACK_BOT_TOKEN(_ACP)" }
if (-not $env:SLACK_TEAM_ID)    { $missing += "SLACK_TEAM_ID(_ACP)" }
if (-not $env:SLACK_MEMBER_IDS) { $missing += "SLACK_MEMBER_IDS(_ACP)" }

if ($missing.Count -gt 0) {
    Write-Host "ERROR: Missing credentials: $($missing -join ', ')" -ForegroundColor Red
    Write-Host "Set them as Windows user-level env vars or in the OS keychain." -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path "config.toml")) {
    Write-Host "ERROR: config.toml not found in current directory." -ForegroundColor Red
    exit 1
}

$binary = ".\target\debug\agent-intercom.exe"
if (-not (Test-Path $binary)) {
    Write-Host "Binary not found at $binary — building..." -ForegroundColor Yellow
    cargo build
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Build failed." -ForegroundColor Red
        exit 1
    }
}

# ── Launch ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "Starting agent-intercom in ACP mode" -ForegroundColor Cyan
Write-Host "  IPC pipe:  agent-intercom-acp (auto-suffixed)" -ForegroundColor DarkGray
Write-Host "  Database:  data/agent-intercom.db" -ForegroundColor DarkGray
Write-Host "  Log level: $env:RUST_LOG" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Use /intercom session-start in a mapped Slack channel to spawn an agent." -ForegroundColor Green
Write-Host ""

& $binary --config config.toml --mode acp
