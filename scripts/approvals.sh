#!/usr/bin/env bash
# The approval list (.github/workflows/approve-specs.yml): one open issue, labelled
# `approval-list`, that lists every spec waiting for the owner with a box to tick. A tick, or a
# comment `/approve all` or `/approve 130 133`, approves; so does `/approve` on a spec's own issue.
# Only the workflow calls the approving commands, and only for the owner (it checks who acted).
#
#   scripts/approvals.sh list              write the list again from the open state:spec issues
#   scripts/approvals.sh approve <n|all>…  approve those specs (open and state:spec only)
#   scripts/approvals.sh ticked            approve what is ticked on the list
#
#   REPO=owner/name   the repository (default FrankDrebin893/hojt-audio)
#   RUN_URL           the workflow run that approves; it goes in the comment, which `/next` reads
#   BOARD_TOKEN       when set, the board's State follows (scripts/board-state.sh)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
repo="${REPO:-FrankDrebin893/hojt-audio}"
marker='<!-- approval-list -->'

list_issue() {
    gh issue list --repo "$repo" --state open --label approval-list --limit 1 --json number --jq '.[0].number // empty'
}

# One line of a spec: its title, what the player gets, its size and how many questions it asks.
# The spec's parts are bold headings (.agents/skills/spec/SKILL.md): **What changes**, **Risks and
# size** (opening with S, M or L) and **Questions** (a numbered list)
describe() {
    local n="$1" title spec player size questions
    title="$(gh issue view "$n" --repo "$repo" --json title --jq .title)"
    spec="$(gh api "repos/$repo/issues/$n/comments" --paginate \
        --jq '[.[] | select(.body | startswith("## Spec"))] | last | .body // ""')"
    player="$(awk '/^\*\*What changes/ {on = 1; next} on && NF {print; exit}' <<< "$spec" |
        sed -E 's/([.!?]) .*/\1/' | cut -c1-160 || true)"
    size="$(awk '/^\*\*Risks and size/ {on = 1; next} on && NF {print; exit}' <<< "$spec" |
        grep -oE '^[SML]\b' || true)"
    questions="$(awk '/^\*\*Questions/ {on = 1; next} /^\*\*|^#+ |^---/ {on = 0} on && /^[0-9]+\. / {n++} END {print n + 0}' <<< "$spec")"
    printf -- '- [ ] #%s %s' "$n" "$title"
    [[ -n "$size" ]] && printf ' · size %s' "$size"
    ((questions > 0)) && printf ' · %s question%s' "$questions" "$([[ $questions -gt 1 ]] && echo s)"
    [[ -n "$player" ]] && printf '\n  %s' "$player"
    printf '\n'
}

cmd_list() {
    local specs body n
    gh label create approval-list --repo "$repo" --color 0e8a16 \
        --description "The list of specs to approve (approve-specs.yml)" --force > /dev/null
    specs="$(gh issue list --repo "$repo" --state open --label state:spec --limit 100 --json number --jq 'sort_by(.number) | .[].number')"
    if [[ -z "$specs" ]]; then
        body="$marker
No spec is waiting for approval.

_Written by \`approve-specs.yml\` on $(date -u +%Y-%m-%d)._"
    else
        body="$marker
Tick a spec to approve it, or tick **Approve all**. A comment \`/approve all\` or \`/approve 130 133\` does the same, and so does \`/approve\` on a spec's own issue. Answering a spec's questions is done on its issue.

- [ ] **Approve all** ($(wc -l <<< "$specs" | tr -d ' '))

$(for n in $specs; do describe "$n"; done)

_Written by \`approve-specs.yml\` on $(date -u +%Y-%m-%d)._"
    fi
    n="$(list_issue)"
    if [[ -z "$n" ]]; then
        # The number from the new issue's URL: a search right after the create does not find it yet
        n="$(gh issue create --repo "$repo" --title "Specs to approve" --label approval-list --body "$body" | grep -oE '[0-9]+$')"
    else
        gh issue edit "$n" --repo "$repo" --body "$body" > /dev/null
    fi
    gh issue pin "$n" --repo "$repo" > /dev/null 2>&1 || true
    echo "the list is #$n"
}

approve_one() {
    local n="$1" labels
    labels="$(gh issue view "$n" --repo "$repo" --json state,labels --jq 'select(.state == "OPEN") | [.labels[].name] | join(",")')"
    if [[ ",$labels," != *",state:spec,"* ]]; then
        echo "#$n is not an open spec: left as it is"
        return 0
    fi
    gh issue edit "$n" --repo "$repo" --remove-label state:spec --add-label state:approved > /dev/null
    # `/next` reads this comment: who started the run is who approved (see .agents/skills/next/SKILL.md)
    gh issue comment "$n" --repo "$repo" --body "Approved by the owner through the approval list.

<!-- approved-by-run: ${RUN_URL:-unknown} -->" > /dev/null
    if [[ -n "${BOARD_TOKEN:-}" ]]; then
        GH_TOKEN="$BOARD_TOKEN" "$root/scripts/board-state.sh" "$n" approved || echo "#$n: the board did not follow"
    fi
    echo "#$n approved"
}

cmd_approve() {
    local n
    if [[ "${1:-}" == all ]]; then
        set -- $(gh issue list --repo "$repo" --state open --label state:spec --limit 100 --json number --jq '.[].number')
    fi
    for n in "$@"; do
        [[ "$n" =~ ^#?[0-9]+$ ]] || { echo "not an issue number: $n" >&2; continue; }
        approve_one "${n#\#}"
    done
}

cmd_ticked() {
    local n body
    n="$(list_issue)"
    [[ -n "$n" ]] || { echo "no approval list"; return 0; }
    body="$(gh issue view "$n" --repo "$repo" --json body --jq .body)"
    if grep -qE '^- \[[xX]\] \*\*Approve all\*\*' <<< "$body"; then
        cmd_approve all
    else
        # shellcheck disable=SC2046
        cmd_approve $(grep -oE '^- \[[xX]\] #[0-9]+' <<< "$body" | grep -oE '[0-9]+$' || true)
    fi
}

case "${1:-}" in
list) cmd_list ;;
approve)
    shift
    cmd_approve "$@"
    cmd_list
    ;;
ticked)
    cmd_ticked
    cmd_list
    ;;
*)
    echo "usage: scripts/approvals.sh list | approve <n|all>... | ticked" >&2
    exit 64
    ;;
esac
