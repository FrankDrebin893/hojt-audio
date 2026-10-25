#!/usr/bin/env bash
# Sets an issue's State on the board "HØJT backlog" (and adds it to the board if it is not there),
# without touching its label. `move-issue.sh` moves both; `board-sync.yml` calls this when a
# label changes. The board belongs to a user, so the token needs the `project` scope: the
# built-in token of a workflow cannot reach it.
# Usage: scripts/board-state.sh <issue> <idea|spec|approved|building|review>
set -euo pipefail

issue=$1
state=$2
case "$state" in
    idea | spec | approved | building | review) ;;
    *) echo "state must be idea, spec, approved, building or review" >&2; exit 1 ;;
esac

owner=FrankDrebin893
repo=$owner/hojt-audio
board=2

name=$(echo "${state:0:1}" | tr a-z A-Z)${state:1}
project=$(gh project view $board --owner $owner --format json --jq .id)
fields=$(gh project field-list $board --owner $owner --format json --jq '.fields[] | select(.name=="State") | .id, (.options[] | "\(.name) \(.id)")')
field_id=$(echo "$fields" | head -1)
option=$(echo "$fields" | sed -n "s/^$name //p")
if [ -z "$option" ]; then
    echo "the board has no State \"$name\"" >&2
    exit 1
fi
item=$(gh project item-add $board --owner $owner --url "https://github.com/$repo/issues/$issue" --format json --jq .id)
gh project item-edit --id "$item" --project-id "$project" --field-id "$field_id" --single-select-option-id "$option" > /dev/null
echo "#$issue is $name on the board"
