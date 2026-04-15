<#
.SYNOPSIS
    Acquire an advisory file lock to prevent concurrent modifications.
.DESCRIPTION
    Creates a .{filename}.lock file in the same directory as the target file.
    Exits with code 0 if the lock is acquired, code 1 if the lock is already held.
.PARAMETER FilePath
    Path to the file to lock, relative to the workspace root.
.NOTES
    Part of the autoharness concurrency control system.
    Referenced by: concurrency.instructions.md, file-lock/SKILL.md
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$FilePath
)

$ErrorActionPreference = 'Stop'

if (-not $FilePath) {
    Write-Error "Usage: acquire_lock.ps1 <filepath>"
    exit 1
}

$resolvedPath = Resolve-Path -Path $FilePath -ErrorAction SilentlyContinue
if (-not $resolvedPath) {
    $dir = Split-Path -Parent $FilePath
    $fileName = Split-Path -Leaf $FilePath
    if (-not $dir) { $dir = "." }
    $lockPath = Join-Path $dir ".$fileName.lock"
} else {
    $dir = Split-Path -Parent $resolvedPath
    $fileName = Split-Path -Leaf $resolvedPath
    $lockPath = Join-Path $dir ".$fileName.lock"
}

if (Test-Path $lockPath) {
    $lockContent = Get-Content $lockPath -Raw -ErrorAction SilentlyContinue
    Write-Warning "Lock already held on: $FilePath"
    if ($lockContent) {
        Write-Warning "Lock info: $lockContent"
    }
    exit 1
}

$agentName = if ($env:AGENT_NAME) { $env:AGENT_NAME } else { "unknown" }
$timestamp = Get-Date -Format "o"
$pid_val = $PID

$lockContent = @"
agent: $agentName
timestamp: $timestamp
pid: $pid_val
"@

try {
    Set-Content -Path $lockPath -Value $lockContent -NoNewline -ErrorAction Stop
    Write-Host "Lock acquired: $FilePath"
    exit 0
} catch {
    Write-Error "Failed to acquire lock: $_"
    exit 1
}
