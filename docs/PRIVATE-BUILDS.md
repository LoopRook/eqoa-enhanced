# Private builds, public source and public releases

The complete private fork is the source of official player builds. The public
repository remains useful, buildable source and owns all player downloads,
issues, documentation and the Enhanced update channel. Its source archives do
not contain every component of the official executable.

## Repository and branch layout

| Repository | Branch/ref | Purpose |
| --- | --- | --- |
| `LoopRook/eqoa-enhanced` | `develop` | Reviewed public integration; no private implementation or history |
| Public | `enhanced` | Approved public release snapshot |
| Public | `vX.Y.Z` | Immutable tag on public `enhanced`; source archives contain public code only |
| `LoopRook/eqoa-enhanced-private` | `private-builds` | Complete integration branch, including private features and merged public changes |
| Private | feature branches | Private implementation or preparation of reviewed public exports |
| Private | `vX.Y.Z` | Immutable tag on `private-builds`; the actual official build source |

The two version tags have the same name but different commit IDs. Never push a
private tag or branch to the public repository. Use separate clones and exact
refspecs; never `push --all`, `push --mirror` or `push --tags` from the private
clone. In a private-only clone, omit a public push remote or give its fetch-only
remote a disabled push URL (`git remote set-url --push public DISABLED`). This
is a local accident guard, not an access-control boundary.

The private repository currently defaults to a feature branch. After approval,
install the prepared private feature as `private-builds` and select that as its
default branch. Manual Actions dispatch needs the workflow on the default
branch. This preparation does not change GitHub settings or shared branches.

## Sync policy

Public to private: fetch public `develop`, review it, and **merge** it into
`private-builds` through a private feature branch. Retain the private workflow,
release pin and private-only policy entries during conflicts. Never resolve a
conflict by deleting the private functionality. At release preparation, also
merge the exact public `enhanced` commit that will receive the version tag.

Private to public: create an export branch **from public develop**, apply only
explicitly reviewed non-private commits with `cherry-pick --no-commit`, inspect
the complete staged diff, sanitize notes, then commit. Before any approved
public push, run the private export audit against that local public branch:

```sh
python tools/private_release.py audit-public --public-root '<public clone>' --public-ref '<export branch>'
```

Run this command from the private clone; it reads local history without pushing. A selected commit may
have private parents: the new public commit must not retain those parents.
Never merge a private branch into public, even if its final tree looks safe;
the merge would disclose its history. Reverts do not remove disclosed history.

Export work must also build and test without the private feature. Merge the
accepted public export back into private so future public merges have the same
history. Do not repeatedly cherry-pick the same export back and forth.

## Workflows and credentials

- Public `.github/workflows/public-source.yml` runs on develop/enhanced
  pushes, version tags, pull requests and manual dispatch. It has `contents:
  read`, tests/builds public source, and uploads no artifacts or release assets.
  These builds have no official updater channel stamp.
- Every job in inherited `dev.yml` and `notes.yml` is restricted to the upstream
  repository identity. Neither Enhanced repository can run those publishers.
- Private `.github/workflows/private-release.yml` runs on private `v*` tags or
  manual dispatch with an existing immutable tag. Its default token is
  `contents: read`. Builds run only inside the private repository, pin and
  validate both source commits, and retain runtime artifacts there briefly.
- Only the private publication job receives `PUBLIC_RELEASE_TOKEN`, from the
  private repository's Actions secrets. It publishes to the fixed public repo;
  the token is never placed in checkout credentials or the build job.

Use a **fine-grained PAT** owned by the publisher, scoped to **only
LoopRook/eqoa-enhanced**, with repository **Contents: read and write** and
mandatory Metadata read. No Actions write, Workflows write, administration or
private-source access is needed. Store it only as `PUBLIC_RELEASE_TOKEN` in the
private repository's Actions secrets, set an expiry, and rotate it before then.
For an organization-managed publisher, a GitHub App installed only on that
public repository with Contents write is the alternative; mint its short-lived
token in the private publication job. The supplied YAML uses the PAT for the
smallest setup. A deploy key authenticates Git transport and is not a substitute
for the REST API token used to upload release assets.

Where the account plan permits it, optionally add a protected environment
on the private publication job and restrict it to version tags. The supplied
workflow uses a repository secret so it also works on GitHub Free; private
environment secrets require a paid plan, and private required-reviewer gates
have further plan restrictions. Protect both repos' version
tags against update/deletion, and restrict release branches to reviewed merges.
Do not grant the publisher bypass access to move tags. Environment/ruleset setup
is an owner action after reviewing this change; no settings were changed here.

## Source pin and version identity

The private `.github/private-release.json` pins the full public commit. Private
preflight checks that this exact commit is both the public version tag and an
ancestor of private HEAD, and that each tag is on its own release branch. It
also verifies equal VERSION, public player notes, release notes, package inputs,
public workflow gates, and absence of private material from public history.
Private notes stay outside the public notes and packaged file allowlist.

Both trees' VERSION and both tags use the same canonical `X.Y.Z`. The executable
has `EQOA_UPDATE_CHANNEL=enhanced` and `EQOA_BUILD_COMMIT=<full private tag SHA>`.
The existing packager stamps that **same private SHA** into `update.txt` and
`release-manifest.json` inside the ZIP. An HTML metadata marker records opaque
source and build hashes for verification. The visible release description
contains only the reviewed player notes.

The updater URL remains:

`https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/`

Its manifest remains exactly three lines:

```text
version X.Y.Z
commit PRIVATE_BUILD_SHA
build windows-x86_64 eqoa-enhanced-windows-x86_64.exe SHA256 BYTE_LENGTH
```

The updater currently compares **commit identity**, not semantic version order.
If the manifest commit differs, it downloads the named payload, checks its hash
and size, and swaps the executable. Once restarted, the embedded private SHA
matches the manifest and it stops updating. A second byte-identity check also
avoids restarting into identical bytes. The publication guard, not the updater,
enforces increasing semantic versions. Do not publish public-source binaries or
a second manifest for the same version. Local/public CI builds are unstamped and
do not participate in automatic updates. Existing official public-built clients
will update to the first different private SHA normally, even if their previous
builds predate this migration. Use a new higher version for that migration.

## Activation checklist (requires owner approval)

1. Review both prepared feature branches and the private-only handoff. The public
   branch must contain only the defaults, safe notes, CI gates and documentation.
2. Before the first new tag, disable the retired public workflow IDs for
   enhanced-release.yml, dev.yml and notes.yml in GitHub Actions, and cancel all
   their in-flight runs. Disabling prevents historical reruns as well; it does
   not cancel existing runs. This is required to close the old-definition race.
   Source CI now uses public-source.yml, so disabling the retired identity does
   not disable source checks. With owner approval, the CLI commands are:

   ```sh
   gh workflow disable enhanced-release.yml --repo LoopRook/eqoa-enhanced
   gh workflow disable dev.yml --repo LoopRook/eqoa-enhanced
   gh workflow disable notes.yml --repo LoopRook/eqoa-enhanced
   ```

   Merge the public feature through develop to enhanced before creating any new
   release tag. Check default branch workflows, retired workflow disabled state
   and the new tag's tree. Restrict who can write releases.
3. Merge the prepared private feature into `private-builds`; set the private
   default branch as described above. Merge approved public changes into it.
4. Configure the private repository credential and tag/branch protection. Check
   that no equivalent publishing token exists in public Actions secrets.
5. Run public source CI and private tests/builds before the first official tag.
   Review the test matrix below. No live release was created by this preparation.

## Release runbook (commands are for the owner-approved release)

Use a new version higher than every published semantic release. The commands
below run in separate clean public/private clones; substitute the chosen version
for `X.Y.Z`. Never move an existing version tag.

1. **Prepare public source.** On a public feature branch from develop, update
   VERSION, FORK-UPDATE-NOTES.md and `docs/releases/X.Y.Z.md`. Include only public
   changes. Pass source build/tests, tooling tests and footprint checks. After
   approval merge through develop into enhanced. Record `PUBLIC_SHA` as the full
   `git rev-parse enhanced` value. Do not create a release yet.
2. **Prepare private source.** In the private clone fetch public enhanced and
   merge it into a feature branch from private-builds. Set `publicCommit` in
   `.github/private-release.json` to PUBLIC_SHA. Retain the private-only code and
   workflow, match the public VERSION and player-facing files, and pass private
   tests/build/footprint checks. Merge into private-builds after review and live
   acceptance. Record its full `PRIVATE_SHA`.
3. **Tag the public snapshot, then the private build.** In the public clone:

   ```sh
   git tag -a vX.Y.Z PUBLIC_SHA -m "EQOA Enhanced X.Y.Z public source"
   git push public refs/tags/vX.Y.Z:refs/tags/vX.Y.Z
   ```

   Confirm public source CI runs without a publication job. In the private clone:

   ```sh
   git tag -a vX.Y.Z PRIVATE_SHA -m "EQOA Enhanced X.Y.Z official build"
   git push private refs/tags/vX.Y.Z:refs/tags/vX.Y.Z
   ```

   A private tag push starts the private build. For a workflow/credential-only
   retry, use Private official release -> Run workflow on private-builds, with
   the existing tag. The workflow definition can be newer; checked-out source,
   scripts and stamp must remain from the immutable tag. A source/tooling change
   in that tagged build requires a fresh version/tag.
4. **Build and publish.** Private CI validates tag ancestry and the public pin,
   runs tooling/workspace tests, builds Windows x64 with static CRT, packages the
   existing five assets, and verifies hashes, PE headers and provenance. The
   serialized publisher rechecks the public tag, refuses published/nonmonotonic
   versions, creates or resumes a draft, uploads only missing matching assets,
   downloads/verifies the complete remote set, then publishes it as latest.
   Public release text comes from the pinned public source plus the two hashes.
5. **Verify anonymously.** Download the latest ZIP, SHA256SUMS and update.txt from
   the public page. Verify all four listed checksums, the ZIP's sourceCommit is
   PRIVATE_SHA, and update.txt names the same SHA/payload hash/size. Check that
   the public Source code archives contain only public source. On a clean
   installation run the ZIP and confirm the private functionality. From the
   previous official executable verify update, restart, then no second update
   offer. Confirm public CI uploaded nothing. Record private CI run ID, both
   commit IDs, hashes, fixture skips and the owner's live result privately.

## Retry and rollback

A failed unpublished draft is safe to resume only with identical build identity
and matching existing asset bytes. The publisher never uses `--clobber` and
never overwrites a published release. A rebuilt executable whose bytes differ
from an incomplete draft requires explicit draft cleanup after inspection, or a
new version; never silently substitute another build. Transient failures before
publication leave the previous latest release active. If publication succeeded
but the response was lost, inspect the public release; a retry refuses it.

Preferred rollback: make a new, **higher-version** private release using the
last good implementation, with a fresh private commit and matching public
version/pin. Follow the same tests and publication path. Never force-move tags
or replace bytes under an existing published version. For an emergency, the
owner can select an earlier verified private-built release as latest. Because
the updater compares commits, clients may downgrade to it. Record that action
and follow it with a higher-version recovery release; ensure its companion
files and server support remain compatible.

## Test matrix and limits

| Case | Required result |
| --- | --- |
| Public source tree/history | No private implementation, ancestors or private local notes; public notes only |
| Public build/test | Workspace suite and release build pass without private code; no updater stamp |
| Private build/test | Private feature tests, workspace suite, footprint and release build pass |
| Wrong public pin/tag or missing merge | Private preflight refuses the build |
| Public CI on version tag/manual run | Tests/build only; no assets or release writes |
| Inherited workflows on either Enhanced repo | Every job skipped by repository identity guard |
| Manifest and package | Private SHA in binary, manifest and ZIP; exact runtime allowlist; valid hashes/size |
| Older official client using public URL | Downloads private-built payload, restarts, then remains current |
| Current private build / identical payload | No repeated download/restart offer |
| Existing published version, extra assets or different draft identity | Publication refuses without overwrite |
| Two private tag runs | Publication serialized; stale/nonmonotonic version refused after lock acquisition |
| Key/protocol change | Update only the private implementation and private tests; use a new higher release version |

Local tests can verify source boundaries, workflow gates, mock publication,
package fixtures and the updater's loopback HTTP/hash/swap behavior. GitHub token
permissions, hosted runner execution, anonymous latest redirects and live shard
acceptance require the owner-approved activation/release; offline tests do not
establish those results.

Concurrency groups are repository-local, and do not constrain a human or another
repository's writer. Keep one publisher, protect tags, and restrict release
writes. A changed tag is rechecked immediately before publication, but protected
tags are still required to close the external-writer race.

Private source is not a cryptographic secret store for a shipped client. An
embedded key can be extracted from its executable; keeping source private limits
source disclosure, not determined binary inspection. If the server changes its
key or handshake, older clients may stop working until a new private build is
released. Coordinate rollout/grace periods with the server owner and keep
diagnostic notes private. Do not place implementation details in public notes,
source archives, debug symbols, source bundles, artifact names or public CI logs.

The updater replaces only the EXE. Companion asset changes still require a ZIP
refresh or separate update support, as documented in DEVELOPMENT-FREEZE.md.
Existing MIT and third-party notices remain in every package.

## GitHub references

- [GITHUB_TOKEN is limited to its workflow repository](https://docs.github.com/en/actions/concepts/security/github_token)
- [Environment secret and protection-rule plan requirements](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments)
- [Fine-grained PAT permissions](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
- [Release API permissions and tags](https://docs.github.com/en/rest/releases/releases)
- [Disabling workflows also prevents historical reruns](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disable-and-enable-workflows)
- [Workflow concurrency is repository-scoped](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
