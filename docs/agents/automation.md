# Automation and health

Read by the scheduled routines and by whoever changes them. The rule every session needs, that the routines
pause while an issue titled `Pause automation` exists, is in `CLAUDE.md`; the timetable and the limits are here.

Scheduled Claude tasks keep the backlog moving and the releases coming; each runs a skill and stops early while an
open issue titled `Pause automation` exists:

| Task | When (UTC) | Runs | Its limit |
|------|------------|------|-----------|
| Scout | Mondays 06:41 | `/scout` | 3 ideas a run; none while 10 of its ideas are ungroomed |
| Specwriter | Daily 06:52 | `/specwriter` | 4 specs a run, an idea older than 7 days first; none while 10 wait for approval |
| Builder | Nightly 01:47 | `/next` | Only what Rasmus approved; nothing to do, it stops at once |
| Release Manager | Fridays 06:30 | `/release-prep` | Only when a release is due (`/release-prep`, step 2) and no `release/*` pull request is open; it opens that pull request with auto-merge off and never tags: Rasmus merges and tags |
| Release Analyst | Mondays 07:15 | `/release-review` | A review per release at 7 and 28 days on the issue "Release reviews"; 3 ideas a run (`from:review`), none while 10 of them are ungroomed, none from thin evidence |

The board's State follows the label by `board-sync.yml`, so a session that sets a label needs no
`scripts/move-issue.sh` (it needs the secret `BOARD_TOKEN`, a classic token with the `project`
scope; without it the board stays as it is). `health.yml` measures the project every Monday
(`scripts/health.sh --sound`: the backlog's flow, CI, releases, CPU and loudness), keeps the line
on the `metrics` branch and comments the digest on the issue "Project health"; what each number
is: `scripts/README.md`, Health.

## Releases

The weekly `/release-prep` prepares the release pull request when a release is due, with auto-merge off; merging
it and tagging `v<version>` stay with Rasmus. `/release-review` looks at how each release did, 7 and 28 days
after it, on the issue "Release reviews", and files what it learns as `from:review` ideas.
