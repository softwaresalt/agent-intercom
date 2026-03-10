<#
.SYNOPSIS
    Runs the automated API + Playwright regression harness.

.DESCRIPTION
    Executes Rust API coverage (unit, contract, integration), optional live
    Slack API tests, and the self-seeding Playwright UX suite in `tests/visual`.
    The visual suite is designed to avoid routine manual HITL passes by posting
    its own Slack fixtures before opening the browser.

.PARAMETER Suite
    Which suites to run:
      - all    : Rust API coverage + Playwright UX
      - api    : Rust API coverage only
      - visual : Playwright UX only

.PARAMETER IncludeLiveSlack
    Also run the feature-gated live Slack API tests.

.PARAMETER ServerMode
    Server mode to use if the harness starts `agent-intercom` for supplemental
    visual coverage. Defaults to MCP.

.PARAMETER SkipServerStart
    Assume any required server is already running.

.PARAMETER BootstrapVisualDeps
    Install `tests/visual` dependencies when `node_modules` is missing.

.EXAMPLE
    pwsh .\scripts\run_automated_test_harness.ps1

.EXAMPLE
    pwsh .\scripts\run_automated_test_harness.ps1 -Suite api -IncludeLiveSlack

.EXAMPLE
    pwsh .\scripts\run_automated_test_harness.ps1 -Suite visual -BootstrapVisualDeps
#>

param(
    [ValidateSet("all", "api", "visual")]
    [string]$Suite = "all",

    [switch]$IncludeLiveSlack,

    [ValidateSet("mcp", "acp")]
    [string]$ServerMode = "mcp",

    [switch]$SkipServerStart,

    [switch]$BootstrapVisualDeps
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

$script:HadFailures = $false
$script:Results = [System.Collections.Generic.List[object]]::new()
$script:ServerProcess = $null

function Add-PhaseResult {
    param(
        [string]$Phase,
        [string]$Status,
        [string]$Details
    )

    $script:Results.Add([pscustomobject]@{
        Phase   = $Phase
        Status  = $Status
        Details = $Details
    })
}

function Write-PhaseHeader {
    param([string]$Phase)

    Write-Host ""
    Write-Host "=== $Phase ===" -ForegroundColor Cyan
}

function Invoke-ExternalStep {
    param(
        [string]$Phase,
        [scriptblock]$Command,
        [string]$SuccessDetails
    )

    Write-PhaseHeader $Phase

    try {
        & $Command
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            Add-PhaseResult -Phase $Phase -Status "PASS" -Details $SuccessDetails
            return $true
        }

        $script:HadFailures = $true
        Add-PhaseResult -Phase $Phase -Status "FAIL" -Details "Exited with code $exitCode"
        return $false
    } catch {
        $script:HadFailures = $true
        Add-PhaseResult -Phase $Phase -Status "FAIL" -Details $_.Exception.Message
        return $false
    }
}

function Get-EffectiveEnvValue {
    param([string]$Name)

    foreach ($scope in @("Process", "User", "Machine")) {
        $value = [System.Environment]::GetEnvironmentVariable($Name, $scope)
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            return $value.Trim()
        }
    }

    return $null
}

function Get-DotEnvValue {
    param(
        [string]$Path,
        [string]$Name
    )

    if (-not (Test-Path $Path)) {
        return $null
    }

    $escaped = [regex]::Escape($Name)
    foreach ($line in Get-Content $Path) {
        if ($line -match "^\s*$escaped\s*=\s*(.+?)\s*$") {
            $value = $Matches[1].Trim()
            if ($value.StartsWith('"') -and $value.EndsWith('"')) {
                return $value.Trim('"')
            }

            if ($value.StartsWith("'") -and $value.EndsWith("'")) {
                return $value.Trim("'")
            }

            return $value
        }
    }

    return $null
}

function Get-VisualConfigValue {
    param([string]$Name)

    $envValue = Get-EffectiveEnvValue -Name $Name
    if (-not [string]::IsNullOrWhiteSpace($envValue)) {
        return $envValue
    }

    return Get-DotEnvValue -Path "tests\visual\.env" -Name $Name
}

function Get-MissingVisualAuthVariables {
    $required = @(
        "SLACK_WORKSPACE_URL",
        "SLACK_EMAIL",
        "SLACK_PASSWORD"
    )

    $missing = @()
    foreach ($name in $required) {
        if ([string]::IsNullOrWhiteSpace((Get-VisualConfigValue -Name $name))) {
            $missing += $name
        }
    }

    return $missing
}

function Get-MissingVisualFixtureVariables {
    $required = @(
        "SLACK_TEST_CHANNEL",
        "SLACK_TEST_BOT_TOKEN",
        "SLACK_TEST_CHANNEL_ID"
    )

    $missing = @()
    foreach ($name in $required) {
        if ([string]::IsNullOrWhiteSpace((Get-VisualConfigValue -Name $name))) {
            $missing += $name
        }
    }

    return $missing
}

function Test-SlackFixtureToken {
    $botToken = Get-VisualConfigValue -Name "SLACK_TEST_BOT_TOKEN"
    if ([string]::IsNullOrWhiteSpace($botToken)) {
        return [pscustomobject]@{
            IsValid = $false
            Status = "SKIP"
            Details = "SLACK_TEST_BOT_TOKEN is not set."
        }
    }

    try {
        $response = Invoke-RestMethod `
            -Uri "https://slack.com/api/auth.test" `
            -Method Post `
            -Headers @{ Authorization = "Bearer $botToken" } `
            -Body @{} `
            -TimeoutSec 10 `
            -ErrorAction Stop

        if ($response.ok -eq $true) {
            $teamName = if ($response.team) { $response.team } else { "unknown workspace" }
            return [pscustomobject]@{
                IsValid = $true
                Status = "PASS"
                Details = "Validated SLACK_TEST_BOT_TOKEN via Slack auth.test for $teamName."
            }
        }

        $errorCode = if ($response.error) { $response.error } else { "unknown" }
        return [pscustomobject]@{
            IsValid = $false
            Status = "FAIL"
            Details = "Slack auth.test returned '$errorCode' for SLACK_TEST_BOT_TOKEN."
        }
    } catch {
        return [pscustomobject]@{
            IsValid = $false
            Status = "FAIL"
            Details = "Slack auth.test request failed for SLACK_TEST_BOT_TOKEN: $($_.Exception.Message)"
        }
    }
}

function Invoke-InVisualProject {
    param([scriptblock]$Command)

    Push-Location "tests\visual"
    try {
        & $Command
    } finally {
        Pop-Location
    }
}

function Get-McpServerUrl {
    if (Test-Path ".vscode\mcp.json") {
        try {
            $mcpJson = Get-Content ".vscode\mcp.json" -Raw | ConvertFrom-Json
            if ($mcpJson.servers."agent-intercom".url) {
                return $mcpJson.servers."agent-intercom".url
            }

            if ($mcpJson.servers."agent-rc".url) {
                return $mcpJson.servers."agent-rc".url
            }
        } catch {
            Write-Host "Warning: could not parse .vscode\mcp.json — using default MCP URL." -ForegroundColor Yellow
        }
    }

    return "http://127.0.0.1:3000/mcp?workspace_id=agent-intercom"
}

function Get-HealthUrl {
    param([string]$ServerUrl)

    try {
        $uri = [System.Uri]$ServerUrl
        return "$($uri.Scheme)://$($uri.Host):$($uri.Port)/health"
    } catch {
        return "http://127.0.0.1:3000/health"
    }
}

function Wait-ForHealth {
    param(
        [string]$HealthUrl,
        [int]$Attempts = 15
    )

    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        try {
            Invoke-RestMethod -Uri $HealthUrl -Method Get -TimeoutSec 2 -ErrorAction Stop | Out-Null
            return $true
        } catch {
            Start-Sleep -Seconds 1
        }
    }

    return $false
}

function Ensure-VisualServer {
    if ($ServerMode -eq "acp") {
        Add-PhaseResult `
            -Phase "Visual server readiness" `
            -Status "SKIP" `
            -Details "ACP mode has no HTTP health endpoint. The self-seeding visual suite can run without ACP server startup."
        return
    }

    $healthUrl = Get-HealthUrl -ServerUrl (Get-McpServerUrl)
    if (Wait-ForHealth -HealthUrl $healthUrl -Attempts 2) {
        Add-PhaseResult `
            -Phase "Visual server readiness" `
            -Status "PASS" `
            -Details "Using existing server at $healthUrl"
        return
    }

    if ($SkipServerStart) {
        Add-PhaseResult `
            -Phase "Visual server readiness" `
            -Status "SKIP" `
            -Details "Server not healthy at $healthUrl and -SkipServerStart was supplied."
        return
    }

    $serverScript = "run-debug.ps1"
    if (-not (Test-Path $serverScript)) {
        $script:HadFailures = $true
        Add-PhaseResult `
            -Phase "Visual server readiness" `
            -Status "FAIL" `
            -Details "Could not find $serverScript"
        return
    }

    Write-PhaseHeader "Visual server readiness"
    $script:ServerProcess = Start-Process `
        -FilePath "pwsh" `
        -ArgumentList "-File", (Resolve-Path $serverScript) `
        -PassThru `
        -WindowStyle Normal

    if (Wait-ForHealth -HealthUrl $healthUrl -Attempts 15) {
        Add-PhaseResult `
            -Phase "Visual server readiness" `
            -Status "PASS" `
            -Details "Started local MCP server via $serverScript (PID $($script:ServerProcess.Id))"
        return
    }

    $script:HadFailures = $true
    Add-PhaseResult `
        -Phase "Visual server readiness" `
        -Status "FAIL" `
        -Details "Started $serverScript but /health never became ready at $healthUrl"
}

try {
    Write-Host ""
    Write-Host "=== Automated Test Harness ===" -ForegroundColor Green
    Write-Host "Suite: $Suite" -ForegroundColor Gray
    Write-Host "Include live Slack: $IncludeLiveSlack" -ForegroundColor Gray
    Write-Host "Server mode: $ServerMode" -ForegroundColor Gray

    $runApi = $Suite -in @("all", "api")
    $runVisual = $Suite -in @("all", "visual")

    if ($runApi) {
        Invoke-ExternalStep `
            -Phase "Rust unit + contract tests" `
            -Command { cargo test --lib --test unit --test contract } `
            -SuccessDetails "cargo test --lib --test unit --test contract"

        Invoke-ExternalStep `
            -Phase "Rust integration tests" `
            -Command { cargo test --test integration } `
            -SuccessDetails "cargo test --test integration"

        if ($IncludeLiveSlack) {
            $missingLive = @()
            foreach ($name in @("SLACK_TEST_BOT_TOKEN", "SLACK_TEST_CHANNEL_ID")) {
                if ([string]::IsNullOrWhiteSpace((Get-EffectiveEnvValue -Name $name))) {
                    $missingLive += $name
                }
            }

            if ($missingLive.Count -gt 0) {
                Add-PhaseResult `
                    -Phase "Live Slack API tests" `
                    -Status "SKIP" `
                    -Details ("Missing live Slack env vars: " + ($missingLive -join ", "))
            } else {
                Invoke-ExternalStep `
                    -Phase "Live Slack API tests" `
                    -Command { cargo test --features live-slack-tests --test live } `
                    -SuccessDetails "cargo test --features live-slack-tests --test live"
            }
        } else {
            Add-PhaseResult `
                -Phase "Live Slack API tests" `
                -Status "SKIP" `
                -Details "Not requested. Re-run with -IncludeLiveSlack to enable."
        }
    }

    if ($runVisual) {
        if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
            $script:HadFailures = $true
            Add-PhaseResult `
                -Phase "Playwright automated suite" `
                -Status "FAIL" `
                -Details "npm is not available on PATH."
        } else {
            if (-not (Test-Path "tests\visual\node_modules")) {
                if ($BootstrapVisualDeps) {
                    Invoke-ExternalStep `
                        -Phase "Playwright dependency bootstrap" `
                        -Command { Invoke-InVisualProject { npm install --no-package-lock } } `
                        -SuccessDetails "npm install --no-package-lock"

                    Invoke-ExternalStep `
                        -Phase "Playwright browser bootstrap" `
                        -Command { Invoke-InVisualProject { npx playwright install chromium } } `
                        -SuccessDetails "npx playwright install chromium"
                } else {
                    Add-PhaseResult `
                        -Phase "Playwright dependency bootstrap" `
                        -Status "SKIP" `
                        -Details "tests\visual\node_modules is missing. Re-run with -BootstrapVisualDeps or run npm install --no-package-lock in tests\visual."
                }
            }

            $missingVisualAuth = Get-MissingVisualAuthVariables
            $missingVisualFixtures = Get-MissingVisualFixtureVariables

            if ($missingVisualAuth.Count -gt 0) {
                $missingMessage = "Missing visual auth config: " + ($missingVisualAuth -join ", ")
                Add-PhaseResult `
                    -Phase "Playwright auth smoke" `
                    -Status "SKIP" `
                    -Details $missingMessage
                Add-PhaseResult `
                    -Phase "Slack fixture token preflight" `
                    -Status "SKIP" `
                    -Details "Skipped because visual auth config is incomplete."
                Add-PhaseResult `
                    -Phase "Playwright seeded fixture suite" `
                    -Status "SKIP" `
                    -Details "Skipped because visual auth config is incomplete."
            } elseif (-not (Test-Path "tests\visual\node_modules")) {
                Add-PhaseResult `
                    -Phase "Playwright auth smoke" `
                    -Status "SKIP" `
                    -Details "Visual dependencies are still missing."
                Add-PhaseResult `
                    -Phase "Slack fixture token preflight" `
                    -Status "SKIP" `
                    -Details "Skipped because visual dependencies are still missing."
                Add-PhaseResult `
                    -Phase "Playwright seeded fixture suite" `
                    -Status "SKIP" `
                    -Details "Visual dependencies are still missing."
            } else {
                Ensure-VisualServer

                $authSmokePassed = Invoke-ExternalStep `
                    -Phase "Playwright auth smoke" `
                    -Command { Invoke-InVisualProject { npm run test:auth-smoke } } `
                    -SuccessDetails "npm run test:auth-smoke (report: tests\visual\reports)"

                if (-not $authSmokePassed) {
                    Add-PhaseResult `
                        -Phase "Slack fixture token preflight" `
                        -Status "SKIP" `
                        -Details "Skipped because Playwright auth smoke failed."
                    Add-PhaseResult `
                        -Phase "Playwright seeded fixture suite" `
                        -Status "SKIP" `
                        -Details "Skipped because Playwright auth smoke failed."
                } elseif ($missingVisualFixtures.Count -gt 0) {
                    $missingMessage = "Missing seeded fixture config: " + ($missingVisualFixtures -join ", ")
                    Add-PhaseResult `
                        -Phase "Slack fixture token preflight" `
                        -Status "SKIP" `
                        -Details $missingMessage
                    Add-PhaseResult `
                        -Phase "Playwright seeded fixture suite" `
                        -Status "SKIP" `
                        -Details $missingMessage
                } else {
                    Write-PhaseHeader "Slack fixture token preflight"
                    $tokenCheck = Test-SlackFixtureToken
                    if ($tokenCheck.Status -eq "FAIL") {
                        $script:HadFailures = $true
                    }

                    Add-PhaseResult `
                        -Phase "Slack fixture token preflight" `
                        -Status $tokenCheck.Status `
                        -Details $tokenCheck.Details

                    if ($tokenCheck.IsValid) {
                        Invoke-ExternalStep `
                            -Phase "Playwright seeded fixture suite" `
                            -Command { Invoke-InVisualProject { npm run test:fixtures } } `
                            -SuccessDetails "npm run test:fixtures (report: tests\visual\reports)"
                    } else {
                        Add-PhaseResult `
                            -Phase "Playwright seeded fixture suite" `
                            -Status "SKIP" `
                            -Details "Skipped because Slack fixture token preflight did not pass."
                    }
                }
            }
        }
    }
} finally {
    if ($script:ServerProcess) {
        try {
            Stop-Process -Id $script:ServerProcess.Id -ErrorAction Stop
        } catch {
            Write-Host "Warning: failed to stop server process $($script:ServerProcess.Id)." -ForegroundColor Yellow
        }
    }

    Write-Host ""
    Write-Host "=== Automated Test Harness Summary ===" -ForegroundColor Cyan
    $script:Results | Format-Table -AutoSize

    $passed = ($script:Results | Where-Object { $_.Status -eq "PASS" }).Count
    $failed = ($script:Results | Where-Object { $_.Status -eq "FAIL" }).Count
    $skipped = ($script:Results | Where-Object { $_.Status -eq "SKIP" }).Count

    Write-Host ""
    Write-Host "Passed: $passed" -ForegroundColor Green
    Write-Host "Failed: $failed" -ForegroundColor Red
    Write-Host "Skipped: $skipped" -ForegroundColor Yellow

    if ($script:HadFailures) {
        exit 1
    }
}
