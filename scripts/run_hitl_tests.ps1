<#
.SYNOPSIS
    Launches the HITL test suite: starts monocoque-agent-rc in debug mode,
    then prompts the user to start the test agent.

.DESCRIPTION
    This script optionally starts the monocoque-agent-rc server via run_debug.ps1,
    performs a health check, then instructs the user to invoke the test-hitl agent
    in VS Code to begin the HITL test scenarios.

.PARAMETER SkipServerStart
    If set, assumes the server is already running and skips the startup step.

.EXAMPLE
    .\scripts\run_hitl_tests.ps1
    .\scripts\run_hitl_tests.ps1 -SkipServerStart
#>

param(
    [switch]$SkipServerStart
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=== HITL Test Suite ===" -ForegroundColor Cyan
Write-Host ""

# ── 1. Verify prerequisites ──────────────────────────────────

if (-not (Test-Path "config.toml")) {
    Write-Host "ERROR: config.toml not found. Cannot start server." -ForegroundColor Red
    exit 1
}

$mcpJson = $null
if (Test-Path ".vscode/mcp.json") {
    $mcpJson = Get-Content ".vscode/mcp.json" -Raw | ConvertFrom-Json
}

$serverUrl = $null
if ($mcpJson -and $mcpJson.servers."agent-rc") {
    $serverUrl = $mcpJson.servers."agent-rc".url
    Write-Host "MCP server URL: $serverUrl" -ForegroundColor Gray
} else {
    Write-Host "WARNING: No 'agent-rc' server found in .vscode/mcp.json." -ForegroundColor Yellow
    Write-Host "         Make sure the MCP server is configured." -ForegroundColor Yellow
}

if (-not (Test-Path ".github/agents/test-hitl.agent.md")) {
    Write-Host "ERROR: .github/agents/test-hitl.agent.md not found." -ForegroundColor Red
    exit 1
}

if (-not (Test-Path ".github/skills/hitl-test/SKILL.md")) {
    Write-Host "ERROR: .github/skills/hitl-test/SKILL.md not found." -ForegroundColor Red
    exit 1
}

if (-not (Test-Path ".github/skills/hitl-test/scenarios.md")) {
    Write-Host "ERROR: .github/skills/hitl-test/scenarios.md not found." -ForegroundColor Red
    exit 1
}

Write-Host "Prerequisites OK." -ForegroundColor Green

# ── 2. Start server ──────────────────────────────────────────

if (-not $SkipServerStart) {
    Write-Host ""
    Write-Host "Starting monocoque-agent-rc in debug mode..." -ForegroundColor Green

    $serverProcess = Start-Process -FilePath "pwsh" `
        -ArgumentList "-NoExit", "-File", ".\run-debug.ps1" `
        -PassThru -WindowStyle Normal

    Write-Host "Server process started (PID $($serverProcess.Id))." -ForegroundColor Green
    Write-Host "Waiting for server initialization..." -ForegroundColor Gray

    # Extract base URL for health check
    $healthUrl = "http://127.0.0.1:3000/health"
    if ($serverUrl -match "http://([^/]+)") {
        $baseHost = $Matches[1] -replace "\?.*$", ""
        $healthUrl = "http://$baseHost/health"
    }

    $attempts = 0
    $healthy = $false
    while ($attempts -lt 15 -and -not $healthy) {
        Start-Sleep -Seconds 1
        $attempts++
        try {
            $response = Invoke-RestMethod -Uri $healthUrl -Method Get -TimeoutSec 2 -ErrorAction Stop
            $healthy = $true
        } catch {
            Write-Host "  Attempt $attempts/15 — waiting..." -ForegroundColor Gray
        }
    }

    if (-not $healthy) {
        Write-Host "ERROR: Server did not become healthy after 15 seconds." -ForegroundColor Red
        Write-Host "       Check run-debug.ps1 output for errors." -ForegroundColor Red
        Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
        exit 1
    }

    Write-Host "Server is healthy." -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "Skipping server start (-SkipServerStart)." -ForegroundColor Yellow
    Write-Host "Ensure the server is already running." -ForegroundColor Yellow
}

# ── 3. Instructions ──────────────────────────────────────────

Write-Host ""
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "  Server is running and connected to Slack." -ForegroundColor White
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "To run the HITL tests:" -ForegroundColor White
Write-Host ""
Write-Host "  1. Open a VS Code chat panel (Ctrl+Shift+I)" -ForegroundColor White
Write-Host "  2. Type: @test-hitl Run HITL tests" -ForegroundColor Yellow
Write-Host "  3. Monitor your Slack channel" -ForegroundColor White
Write-Host "  4. Follow the APPROVE / REJECT instructions for each scenario" -ForegroundColor White
Write-Host ""
Write-Host "The agent will execute 12 scenarios and produce a summary." -ForegroundColor Gray
Write-Host ""

if (-not $SkipServerStart) {
    Write-Host "Press Ctrl+C to stop the server when testing is complete." -ForegroundColor Yellow
    try {
        Wait-Process -Id $serverProcess.Id
    } catch {
        # User pressed Ctrl+C
    } finally {
        Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
        Write-Host ""
        Write-Host "Server stopped." -ForegroundColor Gray
    }
}
