#!/usr/bin/env bash
# release_lock.sh — Release an advisory file lock
#
# Removes the .{filename}.lock file for the specified target file.
# Exits with code 0 regardless — warns if the lock file was not found.
#
# Usage: scripts/release_lock.sh <filepath>
#
# Referenced by: concurrency.instructions.md, file-lock/SKILL.md

set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: release_lock.sh <filepath>" >&2
    exit 1
fi

FILEPATH="$1"
DIR=$(dirname "$FILEPATH")
FILENAME=$(basename "$FILEPATH")
LOCKPATH="${DIR}/.${FILENAME}.lock"

if [ ! -f "$LOCKPATH" ]; then
    echo "WARNING: No lock file found for: $FILEPATH (already released)" >&2
    exit 0
fi

rm -f "$LOCKPATH"
echo "Lock released: $FILEPATH"
exit 0
