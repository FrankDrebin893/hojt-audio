---
name: release
description: Finish and ship a Højte DSP release - take the draft release pull request through its readiness checklist yourself, then, on "ship it", merge and tag. Use when a release/<version> pull request exists, or when cutting a plugin version by hand (the piano viz has its own, /piano-release).
argument-hint: <version, e.g. 0.5.0> [finish | ship]
---

# /release

A release has one path, and the weekly `/release-prep` starts it: a **draft** pull request
`release/<version>` whose description has a section `## Ready to ship` (the checklist is in
`RELEASING.md`, "Readiness"). This skill finishes that checklist and ships. `/release <version>`
with no draft open prepares the pull request first, by `/release-prep` step 5 on with `force`.

Run it in a thread whose computer is linked (Rasmus's PC, EldenDesktop; `list_devices` says),
because the checklist needs Windows. Never ask Rasmus to take a screenshot, build a bundle or run
a script: every box is done by this session, by a Remote Control session
(`start_remote_control` on the PC's repository folder, with the exact commands), or by a CI run
started with the GitHub tools (`actions_run_trigger`). What a session truly cannot do (a Mac, a
listen) is not a box: it goes under `## Not checked` in the description, with a link, and does not
hold the release.

## finish (default)

1. Read the pull request description and the branch. Say which boxes are open.
2. Work the open boxes in the order of `RELEASING.md`, "Readiness". For each, do it and tick it
   (edit the description) **with the evidence in the box**: the run link, or the image path.
   - The `Release` workflow, started on `release/<version>`: wait with Monitor and an until-loop,
     read the log when it fails, never `sleep`. `Test the installer` in the Windows job must be green
   - Screenshots: a Remote Control session on the PC runs `scripts/screenshots.ps1 <plugin>` for
     each plugin whose look changed or is new (the amp's window changes whenever an editor-wide
     element does), commits the images to `release/<version>` and pushes. Then fetch them and **look
     at each one** yourself (Read the image): the pedal is whole, nothing is cut, it shows what the
     changelog says. A bad one is retaken, not ticked
   - Bundles: `cargo xtask bundle <plugin> --release` and `cargo xtask check-art` on the PC
   - Real-machine lines that the PC can answer (the standalones from the installer, 44.1 kHz and 48
     kHz playback, a preset name with a space): run them in the same session, tick them on the
     standing issue "Checks on a real machine" too
   - A doubt the prep run left: read the code or the page and settle it, say how
3. Fix what a box finds on the release branch (a commit, `scripts/check.sh quick`), and redo the
   boxes it touches.
4. When every box under `## Ready to ship` is ticked, `Release ready` is green: mark the pull
   request **ready for review**, and send Rasmus one line: the version, what is in it in two
   sentences, the link, and "say ship it and I merge and tag". Nothing else is asked of him.

## ship

Only after Rasmus says "ship it" (or merges the pull request himself) in the thread or chat:
that one sentence covers the merge and the tag, nothing else does.

1. Check `Release ready` is green and the draft flag is off. If a box opened since, go back to finish
2. Merge the pull request (squash, as the repository does it; auto-merge stays off until then)
3. Tag the merge commit `v<version>` and push the tag. The cloud cannot push tags (the proxy
   refuses), so a Remote Control session on the PC does it: `git fetch origin main`,
   `git tag v<version> <merge commit>`, `git push origin v<version>`. Verify with the GitHub tools
   that the tag exists on that commit
4. `release.yml` builds and publishes: give Rasmus its run link, then watch it to the end and
   report the release page link, or the failing step
5. Tick `## After the merge` in the description, and close the loop: the version's `## Unreleased`
   is already empty above it, and the page was updated by the release commit

Never merge, tag or take a pull request out of draft on a release that has an open box, a failing
`Release ready` or no "ship it". Never tick a box that was not done in this release.
