---
description: Shared Monocoque Agent RC development guidelines for custom agents.
maturity: stable
---
# monocoque-agent-rc Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-09

## Active Technologies

- Rust (stable, edition 2021) + `rmcp` 0.5 (official MCP SDK), `slack-morphism` (Slack Socket Mode), `axum` 0.8 (HTTP/SSE transport), `tokio` (async runtime), `serde`/`serde_json`, `diffy` 0.4 (diff/patch), `notify` (fs watcher), `tracing`/`tracing-subscriber` (001-mcp-remote-agent-server)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test; cargo clippy

## Code Style

Rust (stable, edition 2021): Follow standard conventions

## Recent Changes

- 001-mcp-remote-agent-server: Added Rust (stable, edition 2021) + `rmcp` 0.5 (official MCP SDK), `slack-morphism` (Slack Socket Mode), `axum` 0.8 (HTTP/SSE transport), `tokio` (async runtime), `serde`/`serde_json`, `diffy` 0.4 (diff/patch), `notify` (fs watcher), `tracing`/`tracing-subscriber`

<!-- MANUAL ADDITIONS START -->

## Terminal Command Execution Policy

**Do NOT chain terminal commands.** Run each command as a separate, standalone invocation.

### Rules

1. **One command per terminal call.** Never combine commands with `;`, `&&`, `||`, or `|` unless it falls under an allowed exception below.
2. **No `cmd /c` wrappers.** Run commands directly in the shell rather than wrapping them in `cmd /c "..."`. If `cmd /c` is genuinely required (e.g., for environment isolation), it must contain a single command only.
3. **No exit-code echo suffixes.** Do not append `; echo "EXIT: $LASTEXITCODE"` or `&& echo "done"` to commands. The terminal tool already captures exit codes.
4. **Check results between commands.** After each command, inspect the output and exit code before deciding whether to run the next command. This is safer and produces better diagnostics.
5. **Always use `pwsh`, never `powershell`.** When invoking PowerShell explicitly (e.g., to run a `.ps1` script), use `pwsh` — the cross-platform PowerShell 7+ executable. Never use `powershell` or `powershell.exe`, which refers to the legacy Windows PowerShell 5.1 runtime.
6. **Always use relative paths for output redirection.** When redirecting command output to a file, use workspace-relative paths (e.g., `target\results.txt`), never absolute paths (e.g., `d:\Source\...\target\results.txt`). Absolute paths break auto-approve regex matching.

### Allowed Exceptions

Output redirection is **not** command chaining — it is I/O plumbing that cannot execute destructive operations. The following patterns are permitted:

- **Shell redirection operators**: `>`, `>>`, `2>&1` (e.g., `cargo test > target/results.txt 2>&1`)
- **Pipe to `Out-File` or `Set-Content`**: `cargo test 2>&1 | Out-File target/results.txt` or `| Set-Content`
- **Pipe to `Out-String`**: `some-command | Out-String`

Use these when the terminal tool's ~60 KB output limit would truncate results (e.g., full `cargo test` compilation + test output).

### Why

Terminal auto-approve rules use regex pattern matching against the full command line. Chained commands create unpredictable command strings that cannot be reliably matched, forcing manual approval prompts that slow down the workflow. Single commands match cleanly and approve instantly.

### Correct Examples

```powershell
# Good: separate calls
cargo check
# (inspect output)
cargo clippy -- -D warnings
# (inspect output)
cargo test

# Good: output redirection to capture full results
cargo test 2>&1 | Out-File target\test-results.txt

# Good: shell redirect when output may be truncated
cargo test > target\test-results.txt 2>&1
```

### Incorrect Examples

```powershell
# Bad: chained with semicolons
cargo check; cargo clippy -- -D warnings; cargo test

# Bad: cmd /c wrapper with echo suffix
cmd /c "cargo test > target\test-results.txt 2>&1"; echo "EXIT: $LASTEXITCODE"

# Bad: AND-chained
cargo fmt && cargo clippy && cargo test

# Bad: pipe to something other than Out-File/Set-Content/Out-String
cargo test | Select-String "FAILED" | Remove-Item foo.txt
```
### Full List of Auto-Approve Commands with RegEx

"chat.tools.terminal.autoApprove": {
    ".specify/scripts/bash/": true,
    ".specify/scripts/powershell/": true,
    "/^cargo (build|test|run|clippy|fmt|check|doc|update|install|search|publish|login|logout|new|init|add|upgrade|version|help|bench)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^& cargo (build|test|run|clippy|fmt|check|doc|update|install|search|publish|login|logout|new|init|add|upgrade|version|help|bench)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cargo --(help|version|verbose|quiet|release|features)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^git (status|add|commit|diff|log|fetch|pull|push|checkout|branch|--version)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^& git (status|add|commit|diff|log|fetch|pull|push|checkout|branch|--version)(\\s[^;|&`]*)?(\\s*(>|>>|2>&1|\\|\\s*(Out-File|Set-Content|Out-String))\\s*[^;|&`]*)*$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(Out-File|Set-Content|Add-Content|Get-Content|Get-ChildItem|Copy-Item|Move-Item|New-Item|Test-Path)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^(echo|dir|mkdir|where\\.exe|vsWhere\\.exe|rustup|rustc|refreshenv)(\\s[^;|&`]*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "/^cmd /c \"cargo (test|check|clippy|fmt|build|doc|bench)(\\s[^;|&`]*)?\"(\\s*[;&|]+\\s*echo\\s.*)?$/": {
        "approve": true,
        "matchCommandLine": true
    },
    "New-Item": true,
    "Out-Null": true,
    "cargo build": true,
    "cargo check": true,
    "cargo doc": true,
    "cargo test": true,
    "git commit": true,
    "ForEach-Object": true,
    "cargo clippy": true,
    "cargo fmt": true,
    "git add": true,
    "git push": true
}
<!-- MANUAL ADDITIONS END -->
