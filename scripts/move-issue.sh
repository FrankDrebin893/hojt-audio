#!/usr/bin/env bash
# Moves a backlog issue to a state: its state:* label and its State on the board
# "HØJT backlog" (added to the board if it is not there). Never to approved: that is Rasmus's.
# Usage: scripts/move-issue.sh <issue> <idea|spec|building|review>
set -euo pipefail

issue=$1
state=$2
case "$state" in
    idea | spec | building | review) ;;
    *) echo "state must be idea, spec, building or review" >&2; exit 1 ;;
esac

repo=FrankDrebin893/hojt-audio

old=$(gh issue view "$issue" --repo "$repo" --json labels --jq '[.labels[].name | select(startswith("state:"))] | join(",")')
gh issue edit "$issue" --repo "$repo" ${old:+--remove-label "$old"} --add-label "state:$state" > /dev/null

"$(dirname "$0")/board-state.sh" "$issue" "$state" > /dev/null
echo "#$issue is $state"
