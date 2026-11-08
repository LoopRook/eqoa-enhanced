# Versioned Enhanced releases

Official releases are built and published by this repository's
`.github/workflows/official-release.yml` from an immutable `vX.Y.Z` tag on
`enhanced`, with the repository's own `GITHUB_TOKEN`. Version tags exist only in
this repository. Until 0.4.10 official builds came from the retired private
repository (see [PRIVATE-BUILDS.md](PRIVATE-BUILDS.md)); 0.4.11 is the first
built from public source.

Players download one Windows ZIP and double-click eqoa.exe. Release builds follow
https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/; plain local
builds have no channel. Never point enhanced builds at the upstream vanilla release.
The updater reads `update.txt` there and offers an update when its commit differs
from the commit compiled into the running build.

## Publish

1. Integrate on `develop` after its own checks pass (a sync's are the sync
   script's). Official release's `windows` job is the release gate: it runs the
   footprint check, the workspace tests and the release build, and nothing is
   published unless they pass, so a release needs no second local suite or
   build (the owner, 2026-11-08). Run them locally only to debug a failed job.
2. Bump VERSION using MAJOR.MINOR.PATCH. Add docs/releases/<version>.md and a
   FORK-UPDATE-NOTES.md entry. Patch increments for fixes, minor for substantial
   additions; major 0 is early development.
3. With the owner's go-ahead, merge `develop` into `enhanced`, push it, then
   create and push the matching `v<version>` tag on that `enhanced` commit.
4. The tag starts Official release. The `windows` job (read-only) checks that
   the tag matches VERSION, is on `enhanced` and has release notes, runs the
   footprint check, tooling and workspace tests, builds Windows x64 with channel
   `enhanced` and the exact commit stamped in, and packages only the allowed
   runtime files. The `publish` job (the only one with `contents: write`)
   verifies the assets, creates a draft, uploads, downloads and re-verifies every
   byte, re-checks the tag, and only then marks the release latest. Published
   versions cannot be overwritten and each version must exceed every published
   one. A failed draft can be retried.
5. Check the public ZIP download, SHA256SUMS, update.txt and notes anonymously,
   and an update from the previous release in game.

## Assets

- EQOA-Enhanced-<version>-Windows-x64.zip: player download, with eqoa.exe, catalog,
  required license notices, short README and provenance manifest. No BAT, source,
  developer tools, profile/account files or game disc data.
- eqoa-enhanced-windows-x86_64.exe: raw updater payload, identical to the ZIP's EXE.
- update.txt: three lines: semantic version, source commit, and the payload's
  platform, name, SHA256 and byte length.
- update-notes.md: FORK-UPDATE-NOTES.md, matching the notes compiled into this build.
- SHA256SUMS: checksums for the four assets above.

The platform currently published is Windows x64. Other platforms still build
from source but need their own verified runtime packages before being added to
the updater manifest. Upstream master/dev workflows must not publish this fork;
`dev.yml` and `notes.yml` run only in the upstream repository, and the old
`enhanced-release.yml` stays inert. `public-source.yml` checks every push, pull
request and tag without publishing anything.

## Nightly (development channel)

Every push to `develop` starts `.github/workflows/enhanced-dev.yml`. It runs the
same footprint, tooling, workspace-test and release-build gate as the official
workflow, then refreshes one rolling GitHub release whose tag is `dev`. That
release is always a prerelease and is never marked latest, so
`releases/latest/download/` keeps pointing at the newest stable version. The
development build is stamped with update channel `enhanced-dev` and updates
itself from `releases/download/dev/`. Players see it as **Nightly** (release
title, ZIP `EQOA-Enhanced-Nightly-Windows-x64.zip`, README link). Its notes are
FORK-UPDATE-NOTES.md's `## Unreleased` section, or "Same as Stable." without
one; at a stable release that section becomes the version's entry.

The release keeps the same asset set as a versioned one, with `update.txt`
uploaded last. Assets left over from an older development build are removed.
If `develop` has moved on while the job was running, the publisher exits
without touching the release; the newer run publishes instead.

Pushes that change only top-level `*.md` files, `docs/` or `.github/` build no
Nightly (no update offer for documentation). To rebuild the current `develop`
head without pushing, run Nightly build
manually with **Run workflow** (`workflow_dispatch`). A versioned release still
comes only from the tag path above; publishing `dev` never changes `VERSION`,
FORK-UPDATE-NOTES.md or the stable channel.

## Retry an existing tag after a CI environment fix

Never move the tag. Commit the workflow-only fix to `enhanced`, then run
Official release manually with the existing tag (for example, v0.4.11). The
workflow definition comes from the selected branch, but the checkout, version
and ancestry checks, build stamp and publication all use the tag's commit. A
game-code change needs a new version instead.
