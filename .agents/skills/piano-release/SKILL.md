---
name: piano-release
description: Publish a new public GitHub Release of the Piano MIDI Visualization app (piano/). Picks the next version, pushes a piano-vX.Y.Z tag, waits for the Piano workflow to build the zip and attach it, then points the page and README at it. Use only when the user asks to create, cut or publish a piano release.
argument-hint: "[patch|minor|major|X.Y.Z]"
disable-model-invocation: true
---

# Publish a piano release

Before anything else, read `.agents/learnings.md`, its sections **Everywhere** and **Page and releases**:
what Rasmus said about earlier results. Its rules bind this run as this file does.

A piano release is a pushed `piano-vX.Y.Z` tag. `.github/workflows/piano.yml` sees the tag,
builds the self-contained exe, zips it with `piano/LICENSE` and `piano/THIRD-PARTY-NOTICES.txt`,
and creates the GitHub Release with that zip attached. Anyone can download it there, without a
GitHub account. Workflow artifacts from ordinary pushes to main are not a substitute: GitHub
only serves them to signed-in users.

The plugins release from the same repository on plain `vX.Y.Z` tags (`/release`), and the page
links their zips by `releases/latest/download/`. So a piano release is never marked latest
(the workflow passes `--latest=false`), and there is no "latest" link for the piano: link the
version's own tag.

This skill never builds or uploads anything itself. It checks that main is releasable,
pushes the tag, confirms the release came out right, and then brings the page's download
links up to it.

Where `gh` is signed in, use it. Otherwise use GitHub's public REST API (no auth needed for
this public repo, 60 requests an hour), from PowerShell with `Invoke-RestMethod`, which parses
the JSON, or with `curl` and `jq`:

```powershell
$api = 'https://api.github.com/repos/FrankDrebin893/hojt-audio'
```

## 1. Check that main is releasable

```bash
git fetch --tags origin
git status -sb
```

- **Uncommitted changes** won't be in the release. Ask whether they belong in it. If they do,
  they go in through a pull request first (`main` is protected), then start this step again.
- The tag goes on `origin/main`, which is what CI tested.

Then check that the Piano workflow passed on the last commit of main that touched `piano/`:

```powershell
$sha = git log -1 --format=%H origin/main -- piano .github/workflows/piano.yml
(Invoke-RestMethod "$api/actions/runs?head_sha=$sha").workflow_runs |
    Where-Object { $_.name -eq 'Piano' -and $_.head_branch -eq 'main' } | Select-Object status, conclusion, html_url
```

`in_progress`: wait for it to finish (step 4 shows how to poll). `failure`: stop and report the
failing step (see step 5). Never release a commit whose build is red. No run at all (the piano
has not changed since it moved into this repository): start one with **Run workflow** on the
Piano workflow, or `gh workflow run piano.yml`, and wait for it.

## 2. Pick the version

The argument is `patch` (the default), `minor`, `major`, or an explicit `X.Y.Z`. Tags are
`piano-vMAJOR.MINOR.PATCH`, with no prerelease suffixes. Releases up to v1.0.0 were made in
the old repository, FrankDrebin893/hojt-audio-piano-viz, as `vX.Y.Z`; the first one here
follows on from them, so with no `piano-v*` tag yet the last version is `1.0.0`.

```powershell
$bump = 'patch'   # from the argument
$last = git tag -l 'piano-v*' --sort=-v:refname | Select-Object -First 1
$v = if ($last) { [version]($last -replace '^piano-v', '') } else { [version]'1.0.0' }
$next = 'piano-v' + $(switch ($bump) {
    'major' { "$($v.Major + 1).0.0" }
    'minor' { "$($v.Major).$($v.Minor + 1).0" }
    default { "$($v.Major).$($v.Minor).$($v.Build + 1)" }
})
```

An explicit version must be higher than the last one and not already a tag.

List what's in the release: `git log --oneline $last..origin/main -- piano` (with no `piano-v*`
tag yet, everything since the app moved in: `git log --oneline origin/main -- piano` up to the
old repository's last commit, *Replace standalone Ø app icon…*). If nothing changed, stop.
There's nothing to release.

If the install steps changed since the last release (for example, a SoundFont is now bundled,
or there's now an installer), update `piano/release-notes.md` through a pull request before
tagging. It sits at the top of every release page, above GitHub's generated changelog.

## 3. Tag and push

```bash
git tag -a piano-vX.Y.Z -m "Piano MIDI Visualization X.Y.Z" origin/main
git push origin piano-vX.Y.Z
```

Pushing the tag is what publishes, and the user asked for that by running this skill. If the
permission system still blocks the push, don't retry it or look for another route. The tag
already exists locally, so tell the user to run `git push origin piano-vX.Y.Z` themselves, and
carry on from step 4 once they have.

## 4. Wait for the release build

The tag starts its own run, so filter by `head_branch`. Poll in the background (the Windows
build takes a few minutes), not with a foreground sleep:

```powershell
$tag = 'piano-vX.Y.Z'
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

On success, check that the release exists, has the zip, and did not become latest:

```powershell
$rel = Invoke-RestMethod "$api/releases/tags/$tag"
$rel.html_url
$rel.assets | Select-Object name, size, browser_download_url
(Invoke-RestMethod "$api/releases/latest").tag_name   # still a plugin vX.Y.Z
```

Expect exactly one asset, `PianoMidiVisualizationApp-X.Y.Z-win-x64.zip`, of roughly 70 MB. If
the piano release became latest, the plugins' download buttons on the page are broken: mark
the newest plugin release latest again at once (`gh release edit vA.B.C --latest`, or the
release's **Set as the latest release** box on GitHub) and say so.

On failure, find the step that broke:

```powershell
(Invoke-RestMethod "$api/actions/runs/$runId/jobs").jobs.steps |
    Where-Object conclusion -eq 'failure' | Select-Object name
```

Report it. Don't delete or move the tag unless the user asks, because other people may
already have fetched it. The usual fix is a new commit and the next patch version.

## 6. Point the page at it

The piano page, `site/piano/index.html`, links the zip of one version by its tag (the
`hero-download-btn` and `primary-zip-link` buttons, and any text naming the version). On a
branch `release/piano-X.Y.Z`, change them to
`https://github.com/FrankDrebin893/hojt-audio/releases/download/piano-vX.Y.Z/PianoMidiVisualizationApp-X.Y.Z-win-x64.zip`
Until the first release here, the page's other release links still go to the old repository
(`hojt-audio-piano-viz/releases` and `…/releases/latest`): point them at
`https://github.com/FrankDrebin893/hojt-audio/releases?q=piano-v&expanded=true`. Then open a
pull request with auto-merge; the Page workflow publishes it when it merges. Also update the
version in `piano/README.md` if it names one.

## 7. Report

Give the user:
- the release page, `https://github.com/FrankDrebin893/hojt-audio/releases/tag/piano-vX.Y.Z`
- the page, `https://hojte.net/hojt-audio/piano/`, once the pull request of step 6 merged
- the version and a one-line summary of what's new since the last release
