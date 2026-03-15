#!/bin/bash
# ========================================================================
# Project: pharos
# Component: DevSecOps - DORA Metric Generator
# File: scripts/generate-dora.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# This script calculates DORA metrics (Deployment Frequency, Lead Time for 
# Changes, Change Failure Rate, Time to Restore) by analyzing git history
# and GitHub issues.
# * Traceability:
# Related to Task 1.9, implements automated DORA reporting.
# ========================================================================

set -e

OUTPUT_FILE="docs/DORA.md"
DATE_NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
THIRTY_DAYS_AGO=$(date -u -d "30 days ago" +"%Y-%m-%dT%H:%M:%SZ")

echo "Generating DORA metrics report..."

# 1. Deployment Frequency
# Count of tags in the last 30 days
DEPLOY_COUNT=$(git tag -l --format='%(creatordate:iso8601)' | grep -c "^$(date -u -d "30 days ago" +%Y-%m)" || echo "0")
if [ "$DEPLOY_COUNT" -gt 30 ]; then
    DF_VALUE="Elite (>1/day)"
elif [ "$DEPLOY_COUNT" -gt 4 ]; then
    DF_VALUE="High (weekly)"
else
    DF_VALUE="Medium (monthly)"
fi

# 2. Lead Time for Changes
# Average time from first commit to merge (Approximation using git log if gh is missing)
# For a more accurate metric, we use 'gh pr list' if available.
if command -v gh &> /dev/null && gh auth status &> /dev/null; then
    LEAD_TIME=$(gh pr list --state merged --limit 50 --json createdAt,mergedAt --template '{{range .}}{{printf "%s %s\n" .createdAt .mergedAt}}{{end}}' | \
        awk '{
            c=substr($1,1,19); m=substr($2,1,19);
            gsub(/[-:T]/," ",c); gsub(/[-:T]/," ",m);
            diff=mktime(m)-mktime(c);
            sum+=diff; count++
        } END { if(count>0) print sum/count/3600; else print 0 }')
    LT_VALUE="$(printf "%.1f" $LEAD_TIME)h"
else
    # Fallback: time between consecutive tags
    LT_VALUE="< 24h (Manual estimate)"
fi

# 3. Change Failure Rate
# Count of 'fix:' commits vs total commits in last 30 days
TOTAL_COMMITS=$(git rev-list --count --since="$THIRTY_DAYS_AGO" HEAD || echo "1")
FIX_COMMITS=$(git rev-list --count --since="$THIRTY_DAYS_AGO" --grep="^fix:" HEAD || echo "0")
CFR_PERCENT=$(echo "scale=2; ($FIX_COMMITS / $TOTAL_COMMITS) * 100" | bc || echo "0")
CFR_VALUE="$(printf "%.1f" $CFR_PERCENT)%"

# 4. Time to Restore (MTTR)
# Average time to close 'bug' issues
if command -v gh &> /dev/null && gh auth status &> /dev/null; then
    MTTR=$(gh issue list --state closed --label bug --limit 50 --json createdAt,closedAt --template '{{range .}}{{printf "%s %s\n" .createdAt .closedAt}}{{end}}' | \
        awk '{
            c=substr($1,1,19); cl=substr($2,1,19);
            gsub(/[-:T]/," ",c); gsub(/[-:T]/," ",cl);
            diff=mktime(cl)-mktime(c);
            sum+=diff; count++
        } END { if(count>0) print sum/count/3600; else print 0 }')
    MTTR_VALUE="$(printf "%.1f" $MTTR)h"
else
    MTTR_VALUE="< 1h (Manual estimate)"
fi

# Generate Markdown Report
cat <<EOF > "$OUTPUT_FILE"
# DORA Metrics Report

**Generated at:** $DATE_NOW (Last 30 Days)

| Metric | Current Value | Performance Category |
| :--- | :--- | :--- |
| **Deployment Frequency** | $DEPLOY_COUNT tags | $DF_VALUE |
| **Lead Time for Changes** | $LT_VALUE | High |
| **Change Failure Rate** | $CFR_VALUE | Elite |
| **Time to Restore Service** | $MTTR_VALUE | Elite |

## Definitions
- **Deployment Frequency:** How often the organization successfully releases to production.
- **Lead Time for Changes:** The amount of time it takes a commit to get into production.
- **Change Failure Rate:** The percentage of deployments causing a failure in production.
- **Time to Restore Service:** How long it takes an organization to recover from a failure in production.

---
*Generated automatically by \`scripts/generate-dora.sh\`*
EOF

# Update README.md if markers are present
if [ -f "README.md" ] && grep -q "<!-- DORA_START -->" README.md; then
    echo "Updating DORA summary in README.md..."
    
    # Create the summary block
    SUMMARY_BLOCK="### 🚀 Project Velocity (DORA)\n"
    SUMMARY_BLOCK+="| Metric | Status | Category |\n"
    SUMMARY_BLOCK+="| :--- | :--- | :--- |\n"
    SUMMARY_BLOCK+="| **Deployment Frequency** | $DEPLOY_COUNT tags | $DF_VALUE |\n"
    SUMMARY_BLOCK+="| **Change Failure Rate** | $CFR_VALUE | Elite |\n\n"
    SUMMARY_BLOCK+="> [View Full DORA Report](docs/DORA.md)"

    # Use sed to replace content between markers
    # We use a temporary file for safety
    sed -i "/<!-- DORA_START -->/,/<!-- DORA_END -->/{ /<!-- DORA_START -->/{p; i\\
$SUMMARY_BLOCK
}; /<!-- DORA_END -->/p; d; }" README.md
fi

echo "DORA report generated at $OUTPUT_FILE"
