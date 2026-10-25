#!/usr/bin/env bash
# The weekly run (.github/workflows/weekly.yml): checks the repository and the backlog, and files
# what it finds as `state:idea` issues. It needs no model and no key, only `gh` and its token. It
# only reports and files ideas: it never approves, builds or merges anything, and the one thing
# it changes is taking a stale state label off an issue that is closed.
#
#   scripts/weekly-checks.sh --dry-run    print what it would file or change, and change nothing
#   scripts/weekly-checks.sh              file it
#
#   REPO=owner/name   the repository (default FrankDrebin893/hojt-audio)
#   STALE_DAYS=14     days without activity before a building or review issue is flagged
#
# The checks: (1) closed issues still labelled state:review or state:building, (2) issues that sat
# in building or review with no activity for STALE_DAYS, (3) newer compatible dependencies, the
# nih-plug pin against upstream, the Rust that ci.yml pins against stable, (4) AGENTS.md naming
# files that no longer exist, (5) one summary of the new ideas, for /groom. A check whose tool is
# missing is skipped with a note. A finding is filed once: an issue (open or closed) with its title
# stops it, so titles carry what changed (a version, a month, an issue number).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

repo="${REPO:-FrankDrebin893/hojt-audio}"
stale_days="${STALE_DAYS:-14}"
dry=0
case "${1:-}" in
"") ;;
--dry-run) dry=1 ;;
*)
    echo "usage: scripts/weekly-checks.sh [--dry-run]" >&2
    exit 64
    ;;
esac

filed=()   # the titles filed, or that a dry run would file
existing=""

note() { echo "-- $*"; }

# True when an issue of this title exists already, open or closed.
exists() {
    if [[ -z "$existing" ]]; then
        existing="$(gh issue list --repo "$repo" --state all --limit 1000 --json title --jq '.[].title')"
    fi
    grep -Fxq -- "$1" <<< "$existing"
}

# file <title> <body>: files a state:idea issue unless one of the title exists.
file() {
    local title="$1" body="$2"
    if exists "$title"; then
        note "already filed: $title"
        return 0
    fi
    filed+=("$title")
    if ((dry)); then
        echo "WOULD FILE: $title"
        sed 's/^/    | /' <<< "$body"
        return 0
    fi
    gh issue create --repo "$repo" --title "$title" --label state:idea,kind:feature,area:infra \
        --body "$body

_Filed by the weekly run (\`scripts/weekly-checks.sh\`)._"
    existing="$existing
$title"
}

# 1. Closed issues that still carry a state label: only a label tidy, so the label comes off.
check_closed_with_state() {
    local n labels
    while IFS=$'\t' read -r n labels; do
        [[ -n "$n" ]] || continue
        if ((dry)); then
            echo "WOULD TIDY: #$n is closed, take off $labels"
        else
            gh issue edit "$n" --repo "$repo" --remove-label "$labels"
            echo "tidied: #$n is closed, took off $labels"
        fi
    done < <(gh issue list --repo "$repo" --state closed --limit 1000 --json number,labels \
        --jq '.[] | [.number, ([.labels[].name | select(. == "state:review" or . == "state:building")] | join(","))] | select(.[1] != "") | @tsv')
}

# 2. Open issues in building or review with no activity for a while.
check_stale() {
    local limit n state title age
    limit="$(date -u -d "$stale_days days ago" +%Y-%m-%dT%H:%M:%SZ 2> /dev/null ||
        date -u -v-"${stale_days}"d +%Y-%m-%dT%H:%M:%SZ)"
    while IFS=$'\t' read -r n state title age; do
        [[ -n "$n" ]] || continue
        file "Weekly: #$n has been $state with no activity since ${age%%T*}" \
            "#$n (\"$title\") is labelled \`$state\` and has had no activity for more than $stale_days days (last: ${age%%T*}).

Is it stuck, abandoned, or only waiting? Say what should happen to it: pick it up again, move it back, or close it."
    done < <(gh issue list --repo "$repo" --state open --limit 1000 --json number,title,labels,updatedAt \
        --jq ".[] | . as \$i | [\$i.labels[].name | select(. == \"state:review\" or . == \"state:building\")] as \$s | select((\$s | length) > 0 and \$i.updatedAt < \"$limit\") | [\$i.number, (\$s[0] | sub(\"state:\"; \"\")), \$i.title, \$i.updatedAt] | @tsv")
}

# 3. Dependencies, the nih-plug pin and the Rust toolchain: read-only.
check_dependencies() {
    if ! command -v cargo > /dev/null; then
        note "skipped: no cargo, so no check of newer dependencies"
        return 0
    fi
    local updates count
    updates="$(cargo update --dry-run 2>&1 | grep -E '^ +(Updating|Adding) [^ ]+ v' || true)"
    count="$(grep -c . <<< "$updates" || true)"
    if [[ "$count" -gt 0 ]]; then
        file "Weekly: dependencies have newer compatible versions ($(date -u +%Y-%m))" \
            "\`cargo update --dry-run\` would move these in \`Cargo.lock\`:

\`\`\`
$(head -n 40 <<< "$updates")
\`\`\`
$( ((count > 40)) && echo "(and $((count - 40)) more)" )

Update them together in one change, and run the full check and the sound reports."
    fi
}

check_nih_plug() {
    local rev behind
    rev="$(grep -m1 'nih_plug = ' Cargo.toml | sed -n 's/.*rev = "\([0-9a-f]*\)".*/\1/p')"
    if [[ -z "$rev" ]]; then
        note "skipped: no nih-plug pin found in Cargo.toml"
        return 0
    fi
    local fork
    fork="$(grep -m1 'nih_plug = ' Cargo.toml | sed -n 's#.*github.com/\([^/]*\)/nih-plug.*#\1#p')"
    if ! behind="$(gh api "repos/robbert-vdh/nih-plug/compare/master...$fork:$rev" --jq .behind_by 2> /dev/null)"; then
        note "skipped: could not compare the nih-plug pin with upstream"
        return 0
    fi
    if [[ "$behind" -gt 0 ]]; then
        local head
        head="$(gh api repos/robbert-vdh/nih-plug/commits/master --jq '.sha[0:7]')"
        file "Weekly: the nih-plug pin is $behind commits behind upstream ($head)" \
            "The pin \`${rev:0:7}\` (the fork \`$fork/nih-plug\`) lacks $behind commits of upstream \`master\` (now \`$head\`).

AGENTS.md says how to move it: rebase the fork's commits onto upstream and pin the result."
    fi
}

check_toolchain() {
    local pinned latest
    pinned="$(grep -o 'rust-toolchain@[0-9][0-9.]*' .github/workflows/ci.yml | head -1 | cut -d@ -f2)"
    latest="$(curl -fsS https://static.rust-lang.org/dist/channel-rust-stable.toml 2> /dev/null |
        grep -m1 -A1 '^\[pkg.rust\]' | sed -n 's/^version = "\([0-9.]*\).*/\1/p' || true)"
    if [[ -z "$pinned" || -z "$latest" ]]; then
        note "skipped: could not read the pinned or the latest Rust version"
        return 0
    fi
    # 1.93.0 is behind 1.99.1; compare as versions, not text
    if [[ "$pinned" != "$latest" && "$(printf '%s\n%s\n' "$pinned" "$latest" | sort -V | tail -1)" == "$latest" ]]; then
        file "Weekly: ci.yml pins Rust $pinned, stable is $latest" \
            "The lint job of \`.github/workflows/ci.yml\` pins Rust $pinned; the stable release is $latest.

Move the pin (and the clippy fixes it brings) in one change."
    fi
}

# 4. A backticked name in AGENTS.md that looks like a file or a folder and is not there.
check_drift() {
    local doc dir name missing="" files
    files="$(git ls-files)"
    for doc in AGENTS.md plugins/*/AGENTS.md; do
        [[ -f "$doc" ]] || continue
        dir="$(dirname "$doc")"
        while IFS= read -r name; do
            [[ -n "$name" ]] || continue
            # a path from the root, from the file's folder, or the tail of a tracked path (`ci.yml`)
            [[ -e "$name" || -e "$dir/$name" ]] && continue
            grep -Fq -- "/${name%/}" <<< "/$files" && continue
            missing+="- \`$name\` in \`$doc\`"$'\n'
        done < <(grep -o '`[^` ]*`' "$doc" | tr -d '`' |
            grep -E '^[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)+/?$|^[A-Za-z0-9_-]+\.(md|sh|rs|toml|yml|yaml|py|ps1|json|html|css)$' |
            grep -Ev '^(target|http|www)|^(\.\./)|/(ids\.txt)$|^(owner|path)/|^FrankDrebin893/' | sort -u)
    done
    if [[ -n "$missing" ]]; then
        file "Weekly: AGENTS.md names files that are not there ($(date -u +%Y-%m))" \
            "These names are written in an instruction file and no file or folder of that name exists (a name that is only a pattern or an example may be listed by mistake):

$missing
Fix the instruction or restore the file."
    fi
}

# 5. One issue that lists the ideas of the last week, for /groom to go through.
summarize_ideas() {
    local week since list
    week="$(date -u +%G-W%V)"
    since="$(date -u -d "7 days ago" +%Y-%m-%d 2> /dev/null || date -u -v-7d +%Y-%m-%d)"
    list="$(gh issue list --repo "$repo" --state open --label state:idea --limit 200 \
        --search "created:>=$since" --json number,title --jq '.[] | select(.title | startswith("Weekly 2") | not) | "- #\(.number) \(.title)"')"
    local planned=""
    local t
    for t in "${filed[@]:-}"; do
        [[ -n "$t" ]] && planned+="- (new) $t"$'\n'
    done
    list="$planned$list"
    [[ -n "${list//[$'\n ']/}" ]] || {
        note "no new ideas this week"
        return 0
    }
    file "Weekly $week: new ideas to groom" \
        "New \`state:idea\` issues since $since. Go through them with \`/groom\`:

$list"
}

check_closed_with_state
check_stale
check_dependencies
check_nih_plug
check_toolchain
check_drift
summarize_ideas
echo "== Weekly run done: ${#filed[@]} to file$( ((dry)) && echo " (dry run, nothing changed)")"
