#!/usr/bin/env bash
# The week's health in a few lines of Markdown, from the last two lines of the history that
# scripts/health.sh writes: the headline numbers, what moved, and what crossed a line.
# Usage: scripts/health-digest.sh <history.jsonl>
set -euo pipefail

history="$1"
tail -n 2 "$history" | jq -rs '
  (if length > 1 then .[0] else null end) as $was | last as $now |
  def num(x): if x == null then "–" else (x | tostring) end;
  def delta(a; b): if a == null or b == null or a == b then ""
    else " (" + (if b > a then "+" else "" end) + ((b - a) * 10 | round / 10 | tostring) + ")" end;
  def days(h): if h == null then null else (h / 24 * 10 | round / 10) end;
  ($now.flow) as $f | ($was.flow // {}) as $wf |
  [
    "**Health, \($now.at[0:10])** (commit `\($now.commit)`)",
    "",
    "- Backlog: \($f.wip.idea) ideas, \($f.wip.spec) specs waiting, \($f.wip.approved) approved, \($f.wip.building) building, \($f.wip.review) in review",
    "- Merged: \($f.merged_7d) this week\(delta($wf.merged_7d; $f.merged_7d)), \($f.merged_28d) in 28 days",
    "- Waiting on approval: median \(num(days($f.approval_wait_h_median))) days; oldest spec now \(num(days($f.oldest_spec_wait_h))) days",
    "- Build, approved to merged: median \(num(days($f.build_h_median))) days",
    "- Scout: \($f.scout.filed) filed, \($f.scout.approved) approved, \($f.scout.declined) declined, hit rate \(num($f.scout.hit_rate))%",
    "- CI: main \(num($now.ci.main_pass_pct))% green\(delta($was.ci.main_pass_pct; $now.ci.main_pass_pct)), pull requests \(num($now.ci.pr_pass_pct))%, median \(num($now.ci.pr_minutes_median)) min",
    "- Release: \(num($now.releases.last_tag)), \(num($now.releases.days_since)) days ago, \($now.releases.unreleased_entries) entries unreleased",
    (if $now.releases.downloads then "- Downloads: \($now.releases.downloads_total) in all\(delta($was.releases.downloads_total; $now.releases.downloads_total)); " +
      ([$now.releases.downloads | group_by(.tag) | sort_by(.[0].published) | reverse[] | "\(.[0].tag) \(map(.count) | add)"] | join(", "))
      else empty end),
    (if $now.plugins then "- Plugins: \($now.plugins.tests) tests; CPU " +
      ([$now.plugins.cpu | group_by(.plugin)[] | "\(.[0].plugin) \(map(.core_pct) | max)%"] | join(", ")) else empty end)
  ] +
  ( [
      (if ($now.ci.main_pass_pct // 100) < 90 then "CI on main is green only \($now.ci.main_pass_pct)% of the time" else empty end),
      (if $now.ci.main_red_now then "CI on main is red now" else empty end),
      (if ($f.oldest_spec_wait_h // 0) > 168 then "a spec has waited over a week for approval" else empty end),
      (if ($now.releases.days_since // 0) > 30 and $now.releases.unreleased_entries > 0
        then "\($now.releases.unreleased_entries) changes wait for a release, \($now.releases.days_since) days after the last" else empty end),
      (if $now.plugins and $was.plugins then
        ($now.plugins.cpu[] as $c | ($was.plugins.cpu[] | select(.plugin == $c.plugin and .what == $c.what)) as $o
          | select($o.core_pct > 0 and $c.core_pct > $o.core_pct * 1.1)
          | "CPU up \((($c.core_pct / $o.core_pct - 1) * 100) | round)%: \($c.plugin), \($c.what)") else empty end),
      (if $now.plugins and $was.plugins then
        ($now.plugins.loudness[] as $l | ($was.plugins.loudness[] | select(.render == $l.render)) as $o
          | select($l.lufs != null and $o.lufs != null and (($l.lufs - $o.lufs) | fabs) > 1)
          | "\($l.render) is \((($l.lufs - $o.lufs) | fabs) * 10 | round / 10) LU \(if $l.lufs > $o.lufs then "louder" else "quieter" end) than last week") else empty end)
    ] | if length > 0 then ["", "**Look at:**"] + map("- " + .) else [] end )
  | join("\n")'
