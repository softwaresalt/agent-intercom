# run-debug.ps1 — Start monocoque-agent-rc in debug mode.
# Loads Slack credentials from Windows user-level environment variables
# (so this works in any fresh terminal without a VS Code restart).

$env:SLACK_APP_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_APP_TOKEN", "User")
$env:SLACK_BOT_TOKEN = [System.Environment]::GetEnvironmentVariable("SLACK_BOT_TOKEN", "User")
$env:SLACK_TEAM_ID   = [System.Environment]::GetEnvironmentVariable("SLACK_TEAM_ID", "User")
$env:SLACK_MEMBER_IDS = [System.Environment]::GetEnvironmentVariable("SLACK_MEMBER_IDS", "User")
$env:RUST_LOG        = "info"

Write-Host "Starting monocoque-agent-rc (debug) on http://127.0.0.1:2000 ..."
.\target\debug\monocoque-agent-rc.exe --config config.toml --transport sse --port 2000
