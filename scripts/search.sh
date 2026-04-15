#!/usr/bin/env bash
# search.sh — Search installed skills by keyword
#
# Scans all SKILL.md files under .github/skills/ and returns matches where
# the keyword appears in the skill name or its YAML frontmatter description.
#
# Usage: scripts/search.sh <keyword>
#
# Referenced by: skill-search/SKILL.md

set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: search.sh <keyword>" >&2
    exit 1
fi

KEYWORD="$1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SKILLS_DIR="${SCRIPT_DIR}/../.github/skills"

if [ ! -d "$SKILLS_DIR" ]; then
    echo "ERROR: Skills directory not found at .github/skills/" >&2
    exit 1
fi

printf "%-28s %-68s %s\n" "SKILL" "DESCRIPTION" "PATH"
printf "%-28s %-68s %s\n" "-----" "-----------" "----"

found=0

for skill_dir in "$SKILLS_DIR"/*/; do
    [ -d "$skill_dir" ] || continue
    skill_file="${skill_dir}SKILL.md"
    [ -f "$skill_file" ] || continue

    skill_name=$(basename "$skill_dir")
    description=""

    # Extract description from YAML frontmatter
    in_frontmatter=false
    while IFS= read -r line; do
        if [ "$line" = "---" ]; then
            if $in_frontmatter; then
                break
            else
                in_frontmatter=true
                continue
            fi
        fi
        if $in_frontmatter; then
            case "$line" in
                description:*)
                    description="${line#description:}"
                    # Strip leading/trailing whitespace and quotes
                    description=$(echo "$description" | sed "s/^[[:space:]]*['\"]\\{0,1\\}//;s/['\"]\\{0,1\\}[[:space:]]*$//")
                    ;;
            esac
        fi
    done < "$skill_file"

    # Check if keyword matches name or description (case-insensitive)
    name_match=$(echo "$skill_name" | grep -i "$KEYWORD" 2>/dev/null || true)
    desc_match=$(echo "$description" | grep -i "$KEYWORD" 2>/dev/null || true)

    if [ -n "$name_match" ] || [ -n "$desc_match" ]; then
        # Truncate description if too long
        if [ ${#description} -gt 65 ]; then
            description="${description:0:62}..."
        fi
        rel_path=".github/skills/${skill_name}/SKILL.md"
        printf "%-28s %-68s %s\n" "$skill_name" "$description" "$rel_path"
        found=$((found + 1))
    fi
done

if [ $found -eq 0 ]; then
    echo ""
    echo "No skills found matching '$KEYWORD'"
    echo ""
    echo "Try broader keywords or list all skills:"
    echo "  ls -d .github/skills/*/"
fi
