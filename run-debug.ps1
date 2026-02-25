# run-debug.ps1 — Start agent-intercom in debug mode.
# Loads Slack credentials from Windows user-level environment variables
# (so this works in any fresh terminal without a VS Code restart).

$env:SLACK_APP_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
$env:SLACK_BOT_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
$env:SLACK_TEAM_ID   = [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
$env:SLACK_MEMBER_IDS = [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
$env:RUST_LOG        = "info,agent_intercom::mcp::sse=debug"

Write-Host "Starting agent-intercom (debug) on http://127.0.0.1:3000 ..."
.\target\debug\agent-intercom.exe --config config.toml --transport sse --port 3000
