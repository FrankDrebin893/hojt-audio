---
name: release
description: Publish a new public GitHub Release of the Piano MIDI Visualization app. Picks the next version, pushes a vX.Y.Z tag, waits for the release workflow to build the zip and attach it, and reports the public download link. Use only when the user asks to create, cut or publish a release.
argument-hint: "[patch|minor|major|vX.Y.Z]"
disable-model-invocation: true
---

# Publish a release

A release is a pushed `vX.Y.Z` tag. `.github/workflows/release.yml` sees the tag, builds the
self-contained exe, zips it with `LICENSE` and `THIRD-PARTY-NOTICES.txt`, and creates the
GitHub Release with that zip attached. Anyone can download it there, without a GitHub account.
Workflow artifacts from ordinary pushes to main are not a substitute: GitHub only serves them
to signed-in users.

So this skill never builds or uploads anything itself. It checks that main is releasable,
pushes the tag, and then confirms the release came out right.

`gh` isn't installed here. Use GitHub's public REST API (no auth needed for this public repo,
60 requests an hour) with PowerShell's `Invoke-RestMethod`, which parses the JSON.

```powershell
$api = 'https://api.github.com/repos/FrankDrebin893/PianoVisualization'
```

## 1. Check that main is releasable

```bash
git fetch --tags origin
git status -sb
```

- **Uncommitted changes** won't be in the release. Ask whether they belong in it. If they do,
  commit and push them first (the normal workflow), then start this step again.
- **Local main ahead of origin**: push first. The tag goes on `origin/main`, which is what CI tested.
- **Behind**: that's fine, since the release is cut from `origin/main`. Pull if you'll keep working.

Then check that CI passed on the commit you're about to tag:

```powershell
$sha = git rev-parse origin/main
(Invoke-RestMethod "$api/actions/runs?head_sha=$sha").workflow_runs |
    Where-Object head_branch -eq 'main' | Select-Object status, conclusion, html_url
```

`in_progress`: wait for it to finish (step 4 shows how to poll). `failure`: stop and report the
failing step (see step 5). Never release a commit whose build is red.

## 2. Pick the version

The argument is `patch` (the default), `minor`, `major`, or an explicit `vX.Y.Z`. Tags are plain
`vMAJOR.MINOR.PATCH`, with no prerelease suffixes. The first release is `v0.1.0`.

```powershell
$bump = 'patch'   # from the argument
$last = git tag -l 'v*' --sort=-v:refname | Select-Object -First 1
if (-not $last) { $next = 'v0.1.0' } else {
    $v = [version]$last.TrimStart('v')
    $next = switch ($bump) {
        'major' { "v$($v.Major + 1).0.0" }
        'minor' { "v$($v.Major).$($v.Minor + 1).0" }
        default { "v$($v.Major).$($v.Minor).$($v.Build + 1)" }
    }
}
```

An explicit version must be higher than `$last` and not already a tag.

List what's in the release: `git log --oneline $last..origin/main`, or the whole log for the
first one. If nothing changed since `$last`, stop. There's nothing to release.

If the install steps changed since the last release (for example, a SoundFont is now bundled,
or there's now an installer), update `.github/release-notes.md` and push that before tagging.
It sits at the top of every release page, above GitHub's generated changelog.

## 3. Tag and push

```bash
git tag -a vX.Y.Z -m "vX.Y.Z" origin/main
git push origin vX.Y.Z
```

Pushing the tag is what publishes, and the user asked for that by running this skill. If the
permission system still blocks the push, don't retry it or look for another route. The tag
already exists locally, so tell the user to run `git push origin vX.Y.Z` themselves, and carry
on from step 4 once they have.

## 4. Wait for the release build

The tag starts its own run on the same commit as main's run, so filter by `head_branch`. Poll
in the background (the Windows build takes about 2 minutes), not with a foreground sleep:

```powershell
$tag = 'vX.Y.Z'
$sha = git rev-parse "$tag^{commit}"
for ($i = 0; $i -lt 30; $i++) {
    $run = (Invoke-RestMethod "$api/actions/runs?head_sha=$sha").workflow_runs |
        Where-Object head_branch -eq $tag | Select-Object -First 1
    "$(Get-Date -Format T) $($run.status) $($run.conclusion)"
    if ($run.status -eq 'completed') { $run.id; $run.html_url; break }
    Start-Sleep 30
}
```

## 5. Confirm the release, or explain the failure

On success, check that the release exists and has the zip:

```powershell
$rel = Invoke-RestMethod "$api/releases/tags/$tag"
$rel.html_url
$rel.assets | Select-Object name, size, browser_download_url
```

Expect exactly one asset, `PianoMidiVisualizationApp-X.Y.Z-win-x64.zip`, of roughly 70 MB.

On failure, find the step that broke:

```powershell
(Invoke-RestMethod "$api/actions/runs/$runId/jobs").jobs.steps |
    Where-Object conclusion -eq 'failure' | Select-Object name
```

Report it. Don't delete or move the tag unless the user asks, because other people may
already have fetched it. The usual fix is a new commit and the next patch version.

## 6. Report

Give the user:
- the release page, `https://github.com/FrankDrebin893/PianoVisualization/releases/tag/vX.Y.Z`
- the link that always points to the newest release, which is the one to share:
  `https://github.com/FrankDrebin893/PianoVisualization/releases/latest`
- the version and a one-line summary of what's new since the last release

A release changes no code, so there's nothing to publish to the desktop afterwards.
