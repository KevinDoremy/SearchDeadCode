#!/bin/bash
# SearchDeadCode Pre-Commit Hook
# Blocks commits that introduce new dead code
#
# Installation:
#   cp scripts/pre-commit-hook.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit
#
# Or use with pre-commit framework (https://pre-commit.com):
#   Add to .pre-commit-config.yaml:
#   - repo: local
#     hooks:
#       - id: searchdeadcode
#         name: SearchDeadCode
#         entry: scripts/pre-commit-hook.sh
#         language: script
#         pass_filenames: false

set -e

# Configuration
MIN_CONFIDENCE="${SEARCHDEADCODE_MIN_CONFIDENCE:-high}"
FAIL_ON_FINDINGS="${SEARCHDEADCODE_FAIL_ON_FINDINGS:-true}"
BASELINE_FILE="${SEARCHDEADCODE_BASELINE:-.searchdeadcode-baseline.json}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running SearchDeadCode...${NC}"

# Check if searchdeadcode is installed
if ! command -v searchdeadcode &> /dev/null; then
    echo -e "${RED}Error: searchdeadcode is not installed${NC}"
    echo "Install it with: brew tap KevinDoremy/tap && brew install searchdeadcode"
    echo "Or: cargo install searchdeadcode"
    exit 1
fi

# Build command
CMD="searchdeadcode . --min-confidence $MIN_CONFIDENCE --format json"

# Use baseline if it exists
if [ -f "$BASELINE_FILE" ]; then
    CMD="$CMD --baseline $BASELINE_FILE"
    echo "Using baseline: $BASELINE_FILE"
fi

# Run analysis
OUTPUT=$(eval "$CMD" 2>/dev/null || true)

# Count findings
FINDINGS=$(echo "$OUTPUT" | jq 'length' 2>/dev/null || echo "0")

if [ "$FINDINGS" -gt 0 ]; then
    echo -e "${RED}Found $FINDINGS dead code issue(s):${NC}"
    echo ""

    # Show summary of findings
    echo "$OUTPUT" | jq -r '.[] | "  \(.file):\(.line) - \(.name) (\(.issue_type))"' 2>/dev/null || echo "$OUTPUT"

    echo ""
    echo -e "${YELLOW}To see full details, run:${NC}"
    echo "  searchdeadcode . --min-confidence $MIN_CONFIDENCE"
    echo ""
    echo -e "${YELLOW}To generate a baseline (ignore existing issues):${NC}"
    echo "  searchdeadcode . --generate-baseline $BASELINE_FILE"
    echo ""

    if [ "$FAIL_ON_FINDINGS" = "true" ]; then
        echo -e "${RED}Commit blocked. Fix the issues above or update the baseline.${NC}"
        exit 1
    else
        echo -e "${YELLOW}Warning: Dead code found but SEARCHDEADCODE_FAIL_ON_FINDINGS=false${NC}"
    fi
else
    echo -e "${GREEN}No new dead code found.${NC}"
fi

exit 0
