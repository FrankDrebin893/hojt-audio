#!/usr/bin/env bash
# The project's health in one line of JSON (.github/workflows/health.yml keeps one a week on the
# `metrics` branch, in health/history.jsonl). Needs `gh` with a token and `jq`; reads only.
#
#   scripts/health.sh            the backlog, CI and releases, from GitHub (a few seconds)
#   scripts/health.sh --sound    and the plugins: CPU from the reports' timing lines, the loudness
#                                and true peak of every render (ffmpeg), the number of tests.
#                                Builds everything optimised, so minutes
#
#   REPO=owner/name   the repository (default FrankDrebin893/hojt-audio)
#
# What each number is, and the thresholds the Monday digest flags, are in scripts/README.md, Health.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

repo="${REPO:-FrankDrebin893/hojt-audio}"
sound=0
case "${1:-}" in
"") ;;
--sound) sound=1 ;;
*)
    echo "usage: scripts/health.sh [--sound]" >&2
    exit 64
    ;;
esac

now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ago() { date -u -d "$1 days ago" +%Y-%m-%dT%H:%M:%SZ 2> /dev/null || date -u -v-"$1"d +%Y-%m-%dT%H:%M:%SZ; }
week="$(ago 7)"
month="$(ago 28)"

# jq helpers: hours between two times, and the median of a list (null when empty)
jqlib='def hours(a; b): ((b | fromdateiso8601) - (a | fromdateiso8601)) / 3600 | . * 10 | round / 10;
def minutes(a; b): ((b | fromdateiso8601) - (a | fromdateiso8601)) / 60 | round;
def median: sort | if length == 0 then null else .[length / 2 | floor] end;'

# When an issue last got a label, from its timeline (empty when never)
labelled_at() {
    gh api "repos/$repo/issues/$1/timeline" --paginate \
        --jq "[.[] | select(.event == \"labeled\" and .label.name == \"$2\") | .created_at] | last // empty"
}

# The Pitch Shifter's algorithm and its tests live in suite_common (the amp's pitch pedal
# plays it too); every other plugin's are in its own package
package_of() {
    case "$1" in
        pitch_shifter) echo suite_common ;;
        *) echo "$1" ;;
    esac
}

# --- The backlog -------------------------------------------------------------------------------

open_issues="$(gh issue list --repo "$repo" --state open --limit 1000 --json number,labels)"
wip="$(jq -c '[.[] | .labels[].name | select(startswith("state:")) | sub("state:"; "")]
    | reduce .[] as $s ({idea: 0, spec: 0, approved: 0, building: 0, review: 0}; .[$s] += 1)' <<< "$open_issues")"

merged_week="$(gh pr list --repo "$repo" --state merged --search "merged:>=${week%%T*}" --limit 200 --json number --jq length)"
merged_month="$(gh pr list --repo "$repo" --state merged --search "merged:>=${month%%T*}" --limit 500 --json number --jq length)"

# Open specs: how long each has waited on the owner
waiting="[]"
for n in $(jq -r '.[] | select(any(.labels[]; .name == "state:spec")) | .number' <<< "$open_issues"); do
    at="$(labelled_at "$n" state:spec)"
    [[ -n "$at" ]] && waiting="$(jq -c --arg at "$at" --arg now "$now" "$jqlib . + [hours(\$at; \$now)]" <<< "$waiting")"
done

# Issues closed as done in the last 28 days: spec -> approved (the owner's wait) and approved ->
# closed (the build)
approval_hours="[]"
build_hours="[]"
while IFS=$'\t' read -r n closed; do
    [[ -n "$n" ]] || continue
    spec_at="$(labelled_at "$n" state:spec)"
    approved_at="$(labelled_at "$n" state:approved)"
    if [[ -n "$spec_at" && -n "$approved_at" ]]; then
        approval_hours="$(jq -c --arg a "$spec_at" --arg b "$approved_at" "$jqlib . + [hours(\$a; \$b)]" <<< "$approval_hours")"
    fi
    if [[ -n "$approved_at" ]]; then
        build_hours="$(jq -c --arg a "$approved_at" --arg b "$closed" "$jqlib . + [hours(\$a; \$b)]" <<< "$build_hours")"
    fi
done < <(gh issue list --repo "$repo" --state closed --search "closed:>=${month%%T*} reason:completed" --limit 200 \
    --json number,closedAt --jq '.[] | [.number, .closedAt] | @tsv')

# The Scout's ideas: decided = approved at some point (or built) or turned down
scout="$(gh issue list --repo "$repo" --state all --label from:scout --limit 500 --json state,stateReason,labels \
    --jq '{filed: length,
           approved: [.[] | select((.state == "CLOSED" and .stateReason == "COMPLETED")
               or any(.labels[]; .name == "state:approved" or .name == "state:building" or .name == "state:review"))] | length,
           declined: [.[] | select(.state == "CLOSED" and .stateReason == "NOT_PLANNED")] | length}
          | .hit_rate = (if .approved + .declined == 0 then null else (.approved / (.approved + .declined) * 100 | round) end)' \
    2> /dev/null || echo '{"filed":0,"approved":0,"declined":0,"hit_rate":null}')"

flow="$(jq -nc --argjson wip "$wip" --argjson mw "$merged_week" --argjson mm "$merged_month" \
    --argjson waiting "$waiting" --argjson ah "$approval_hours" --argjson bh "$build_hours" --argjson scout "$scout" \
    "$jqlib"' {wip: $wip, merged_7d: $mw, merged_28d: $mm,
      specs_waiting: ($waiting | length), oldest_spec_wait_h: ($waiting | max),
      approval_wait_h_median: ($ah | median), build_h_median: ($bh | median), scout: $scout}')"

# --- CI ----------------------------------------------------------------------------------------

runs() {
    gh run list --repo "$repo" --workflow ci.yml --limit "$1" "${@:2}" \
        --json conclusion,createdAt,updatedAt --jq '[.[] | select(.conclusion != "" and .conclusion != "cancelled" and .conclusion != "skipped")]'
}
ci="$(jq -nc --argjson main "$(runs 30 --branch main)" --argjson prs "$(runs 60 --event pull_request)" "$jqlib"'
    def rate: if length == 0 then null else ([.[] | select(.conclusion == "success")] | length) / length * 100 | round end;
    {main_pass_pct: ($main | rate), main_runs: ($main | length), main_red_now: (($main | first | .conclusion) != "success"),
     pr_pass_pct: ($prs | rate), pr_runs: ($prs | length),
     pr_minutes_median: ([$prs[] | minutes(.createdAt; .updatedAt)] | median)}')"

# --- Releases ----------------------------------------------------------------------------------

# The plugins' releases only (vX.Y.Z): the piano viz releases on piano-vX.Y.Z tags in the same repo
release="$(gh release list --repo "$repo" --limit 20 --json tagName,publishedAt \
    --jq '[.[] | select(.tagName | startswith("v"))] | first // {}')"
unreleased="$(awk '/^## Unreleased/ {on = 1; next} /^## / {on = 0} on && /^- / {n++} END {print n + 0}' CHANGELOG.md)"
# Downloads so far of every file of every published release (GitHub counts each download of an asset)
downloads="$(gh api "repos/$repo/releases" --paginate \
    --jq '[.[] | select((.draft | not) and (.tag_name | startswith("v"))) | .tag_name as $t | .published_at as $p | .assets[]
        | {tag: $t, published: $p, asset: .name, count: .download_count}]' | jq -sc 'add // []')"
releases="$(jq -nc --argjson r "$release" --arg now "$now" --argjson u "$unreleased" --argjson d "$downloads" "$jqlib"'
    {last_tag: ($r.tagName // null), days_since: (if $r.publishedAt then (hours($r.publishedAt; $now) / 24 | floor) else null end),
     unreleased_entries: $u, downloads_total: ([$d[].count] | add // 0), downloads: $d}')"

# --- The plugins (--sound) ---------------------------------------------------------------------

plugins="null"
if ((sound)); then
    # The reports that print what a block costs: "... (12.34 % of a core) ..."
    cpu="[]"
    while read -r plugin report; do
        [[ -n "$plugin" ]] || continue
        out="$(cargo test -q -p "$(package_of "$plugin")" --release --locked "$report" -- --ignored --nocapture 2> /dev/null || true)"
        while IFS= read -r line; do
            pct="$(grep -oE '[0-9.]+ % of a core' <<< "$line" | head -1 | cut -d' ' -f1)"
            [[ -n "$pct" ]] || continue
            # What was timed, without its numbers, so that weeks can be compared line by line
            what="$(sed -E 's/^ *//; s/:.*//; s/[0-9]+(\.[0-9]+)?/#/g' <<< "$line")"
            cpu="$(jq -c --arg p "$plugin" --arg w "$what" --argjson v "$pct" \
                '. + [{plugin: $p, what: $w, core_pct: $v}]' <<< "$cpu")"
        done < <(grep '% of a core' <<< "$out")
    done <<< '
amp effects_report
drums kit_report
bass bass_report
pad pad_report
bells bells_report
pitch_shifter quality_report'

    # The amp prints its cost per amp model instead: "Klar       48000 Hz  playing   21.3 us   1.6 %   silent ..."
    while read -r name pct; do
        [[ -n "$name" ]] || continue
        cpu="$(jq -c --arg w "$name playing at # kHz" --argjson v "$pct" \
            '. + [{plugin: "amp", what: $w, core_pct: $v}]' <<< "$cpu")"
    done < <(cargo test -q -p amp --release --locked amp_report -- --ignored --nocapture 2> /dev/null |
        awk '$2 == "48000" && $3 == "Hz" && $4 == "playing" {print $1, $7}' || true)

    # Every render: integrated loudness and true peak
    rm -rf target/renders
    for plugin in amp drums bass pad bells pitch_shifter; do
        cargo test -q -p "$(package_of "$plugin")" --release --locked render_wavs -- --ignored > /dev/null 2>&1 || true
    done
    loud="[]"
    if command -v ffmpeg > /dev/null; then
        for wav in target/renders/*.wav; do
            [[ -f "$wav" ]] || continue
            summary="$(ffmpeg -nostats -hide_banner -i "$wav" -af ebur128=peak=true -f null - 2>&1 | sed -n '/Summary:/,$p')"
            lufs="$(awk '/ I:/ {print $2; exit}' <<< "$summary")"
            peak="$(awk '/Peak:/ {print $2; exit}' <<< "$summary")"
            loud="$(jq -c --arg f "$(basename "$wav" .wav)" --arg i "$lufs" --arg p "$peak" \
                '. + [{render: $f, lufs: ($i | tonumber? // null), true_peak_db: ($p | tonumber? // null)}]' <<< "$loud")"
        done
    fi

    tests="$(cargo test --workspace --locked -- --list 2> /dev/null | grep -c ': test$' || true)"
    plugins="$(jq -nc --argjson cpu "$cpu" --argjson loud "$loud" --argjson t "${tests:-0}" \
        '{cpu: $cpu, loudness: $loud, tests: $t}')"
fi

jq -nc --arg at "$now" --arg commit "$(git rev-parse --short HEAD 2> /dev/null || echo unknown)" \
    --argjson flow "$flow" --argjson ci "$ci" --argjson releases "$releases" --argjson plugins "$plugins" \
    '{at: $at, commit: $commit, flow: $flow, ci: $ci, releases: $releases, plugins: $plugins}'
