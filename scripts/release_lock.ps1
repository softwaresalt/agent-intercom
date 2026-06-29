<#
.SYNOPSIS
    Release an advisory file lock.
.DESCRIPTION
    Removes the .{filename}.lock file for the specified target file.
    Exits with code 0 regardless — warns if the lock file was not found.
.PARAMETER FilePath
    Path to the file to unlock, relative to the workspace root.
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
    Write-Error "Usage: release_lock.ps1 <filepath>"
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

if (-not (Test-Path $lockPath)) {
    Write-Warning "No lock file found for: $FilePath (already released)"
    exit 0
}

try {
    Remove-Item $lockPath -Force -ErrorAction Stop
    Write-Host "Lock released: $FilePath"
    exit 0
} catch {
    Write-Warning "Failed to remove lock file: $_"
    exit 0
}
