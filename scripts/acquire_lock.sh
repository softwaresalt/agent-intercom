#!/usr/bin/env bash
# acquire_lock.sh — Acquire an advisory file lock
#
# Creates a .{filename}.lock file in the same directory as the target file.
# Exits with code 0 if the lock is acquired, code 1 if already held.
#
# Usage: scripts/acquire_lock.sh <filepath>
#
# Referenced by: concurrency.instructions.md, file-lock/SKILL.md

set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: acquire_lock.sh <filepath>" >&2
    exit 1
fi

FILEPATH="$1"
DIR=$(dirname "$FILEPATH")
FILENAME=$(basename "$FILEPATH")
LOCKPATH="${DIR}/.${FILENAME}.lock"

if [ -f "$LOCKPATH" ]; then
    echo "WARNING: Lock already held on: $FILEPATH" >&2
    cat "$LOCKPATH" >&2 2>/dev/null || true
    exit 1
fi

AGENT_NAME="${AGENT_NAME:-unknown}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
PID_VAL=$$

cat > "$LOCKPATH" <<EOF
agent: ${AGENT_NAME}
timestamp: ${TIMESTAMP}
pid: ${PID_VAL}
EOF

echo "Lock acquired: $FILEPATH"
exit 0
