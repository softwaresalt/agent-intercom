<#
.SYNOPSIS
    Search installed skills by keyword.
.DESCRIPTION
    Scans all SKILL.md files under .github/skills/ and returns matches where
    the keyword appears in the skill name or its YAML frontmatter description.
.PARAMETER Keyword
    Search term or phrase describing the capability needed.
.NOTES
    Part of the autoharness skill-search system.
    Referenced by: skill-search/SKILL.md
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Keyword
)

$ErrorActionPreference = 'Stop'

if (-not $Keyword) {
    Write-Error "Usage: search.ps1 <keyword>"
    exit 1
}

$skillsDir = Join-Path $PSScriptRoot ".." ".github" "skills"
$skillsDir = Resolve-Path $skillsDir -ErrorAction SilentlyContinue

if (-not $skillsDir -or -not (Test-Path $skillsDir)) {
    Write-Error "Skills directory not found at .github/skills/"
    exit 1
}

$results = @()
$skillDirs = Get-ChildItem -Path $skillsDir -Directory

foreach ($dir in $skillDirs) {
    $skillFile = Join-Path $dir.FullName "SKILL.md"
    if (-not (Test-Path $skillFile)) { continue }

    $content = Get-Content $skillFile -Raw -ErrorAction SilentlyContinue
    if (-not $content) { continue }

    $description = ""
    if ($content -match '(?ms)^---\s*\n(.*?)\n---') {
        $frontmatter = $Matches[1]
        if ($frontmatter -match 'description:\s*[''"]?(.*?)[''"]?\s*$') {
            $description = $Matches[1].Trim()
        }
    }

    $nameMatch = $dir.Name -like "*$Keyword*"
    $descMatch = $description -like "*$Keyword*"

    if ($nameMatch -or $descMatch) {
        $relativePath = ".github/skills/$($dir.Name)/SKILL.md"
        $results += [PSCustomObject]@{
            Skill       = $dir.Name
            Description = if ($description.Length -gt 70) { $description.Substring(0, 67) + "..." } else { $description }
            Path        = $relativePath
        }
    }
}

if ($results.Count -eq 0) {
    Write-Host "No skills found matching '$Keyword'"
    Write-Host ""
    Write-Host "Try broader keywords or list all skills:"
    Write-Host "  Get-ChildItem .github/skills/ -Directory | Select-Object Name"
    exit 0
}

$results | Format-Table -AutoSize -Wrap
