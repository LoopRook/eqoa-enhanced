# EQOA Enhanced 0.1.0 independent release review

Reviewed 2026-10-28. Scope: cdb9c95 through candidate working tree, including upstream7912c73 merge and untracked release files. Read FORK.md, release plan, release handoff/roadmap, source diff and current updater, packaging, tests, ownership policy, player docs and workflow. No source edits, builds or tests were run by this reviewer.

## Verdict

Spec: substantially satisfied, but publication does not yet guarantee monotonic latest versions or an exact remote asset allowlist on a draft retry. Quality: request changes for the two findings below before public release. Runtime routing and local package validation look suitable. Final full workspace completion, clean stamped executable packaging and actual CI/public download verification remain root-owned gates; this review does not claim those are complete.

## Findings

### P1: Serialize publication across version tags

Location: .github/workflows/enhanced-release.yml:10-12; related check86-95 and publication104.

Concurrency is keyed by github.ref, so v0.1.1 and v0.1.2 use different groups. Both publish jobs can read existing releases and pass the numeric monotonic check before either publishes. If v0.1.2 publishes first and v0.1.1 completes its upload later, the latter unconditionally uses --latest and makes0.1.1 latest. update.rs:1063 compares only source commits, so clients on0.1.2 then install0.1.1. This violates the explicit later-semver-monotonic requirement and can roll player fixes back.

Fix: put the complete publish job in a constant repository-wide concurrency group with cancel-in-progress:false, keeping the monotonic check inside the serialized section. The Windows builds can retain per-tag concurrency. Ensure the later job re-reads current releases after acquiring the lock. Validate the scheduling property and version ordering logic without rerunning unrelated Cargo tests.

API evidence: GitHub concurrency groups serialize only jobs sharing a group, and gh release edit --latest explicitly sets latest. Sources: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency and https://cli.github.com/manual/gh_release_edit .

### P2: Reject unexpected assets retained in an existing draft

Location: .github/workflows/enhanced-release.yml:96-104.

The retry branch uploads out/* with --clobber, which replaces same-name files but does not remove other asset names already attached to the draft. The verified local directory may contain exactly the required five assets while the draft still contains an older preview ZIP, source bundle or unintended profile file from an earlier preparation attempt. The final edit publishes all those remote assets. This defeats the one-player-ZIP and no-private/development-assets guarantees specifically on the supported draft-retry path.

Fix: inspect the remote draft asset set and fail closed on unexpected names before publication; permit missing expected assets during retry, then require the exact expected five-name set after uploads. Alternatively recreate only the unpublished draft with a clearly bounded procedure. Preserve the existing refusal to change a published release. A small fixture for extra remote asset names would directly cover this failure mode.

API evidence: gh release upload --clobber overwrites existing assets of the same name, not the entire release asset collection: https://cli.github.com/manual/gh_release_upload .

## Satisfied requirements and evidence

- Only channel enhanced selects the production URL, and it selects LoopRook/eqoa-enhanced/releases/latest/download/. Local/unknown/dev channels return None. OWN_NOTES includes FORK-UPDATE-NOTES.md; the update window fallback points to the fork release page. Existing size/SHA256 validation and atomic rename mechanics remain upstream code.
- ZIP allowlist is eqoa.exe, map-entities.json, LICENSE, FrontiersForge-MIT.txt, SpaceMono-OFL.txt, README.txt and release-manifest.json. No recursive source/profile scan is used. Ignored account/ISO fixtures and poisoned output tests exercise privacy exclusions and refusal to overwrite. Output staging and final checks reject extra local assets and members.
- Raw updater payload equals ZIP eqoa.exe byte-for-byte. update.txt has canonical semantic version/full commit/platform/filename/hash/size fields. All outer checksums, manifest file hashes, catalog pin and ZIP membership are checked again by the independent publication verifier.
- Executable checks inspect x64 PE32+ headers, GUI subsystem, 8MiB minimum stack and embedded full commit/fork URL. Clean Git provenance is checked twice. These are consistency checks, not cryptographic source attestation.
- Workflow starts only on v* tags, validates canonical VERSION/tag equality and enhanced ancestry, stamps full github.sha/channel enhanced, tests/builds before packaging, and waits for successful Windows artifact completion before publication. It renames checkout origin to looprook then fetches pinned public upstream for the footprint checker. Upstream dev/notes workflows are unchanged; there is no master push or force-tag action.
- Existing map/quest/HUD files are not changed by release integration. Upstream gameplay changes are the reviewed regular merge. Catalog lookup reads map-entities.json next to current_exe, matching ZIP layout. Player README explains the separate optional parser and incomplete Forge parity. No parser repository changes or alternative license selection are present.
- Read release-packaging-report.md:31 Python tests, including11 package tests, passed according to the packaging agent. Negative cases include real ZIP/PE handling and recomputed outer hashes. Read release-goto-disc-tests.log:6 passed with actual ground-arrival positive and negative controls (new path stands around53.72; old respawn path falls to-70.39). Browser build log contains generated assets; root reports success. Final workspace log was still running during review. Root reports corrected stale vanilla-link test and current fork notes compiled into fresh tests; inspected updatewnd.rs now requires enhanced and rejects vanilla.

## Considered and set aside

- Latest manifest/payload can cross a release transition between HTTP requests. Existing hash/size validation rejects mismatched bytes and leaves the installed program usable; retry obtains matching assets. No new corruption path.
- Self-update replaces only the EXE. The initial ZIP provisions the pinned catalog/notices, and player docs explicitly require complete ZIP extraction when a future catalog changes. Catalog migration automation is not necessary for unchanged0.1.0 data.
- Raw updater EXE plus player ZIP is intentional; only one player ZIP is promised, not only one release asset. GitHub-generated source archive links are platform behavior, not source inclusion in the player package.
- Unstamped/local builds have no updater, and unsupported platforms have no published build row. This matches the Windows-only first release.
- EQOA_UPDATE_URL remains the pre-existing explicit fixture override. The default production route is fork-owned. Broader override hardening is outside this release seam.
- Legacy dev-oriented updater comments/user-agent wording do not change routing. Cosmetic cleanup would expand shared diffs without resolving a functional defect.
- Missing signing/source attestation is not a regression or an agreed0.1.0 requirement. Hashes defend consistency; trusted source and CI remain the provenance boundary.
- Fixed --limit100 release enumeration is sufficient for the initial release and ordinary monotonic history, but eventual pagination is preferable if many unpublished drafts/nonsemantic releases can hide the highest published semantic release. Not elevated independently for0.1.0.
- Git tag immutability is respected by this workflow because it never changes tags and refuses published-release replacement. Repository-level tag rules/immutable-release settings were not inspected and are not asserted enabled.

## Additional root finding accepted during review

### P1: Eliminate the unbundled Visual C++ runtime dependency

Location: .github/workflows/enhanced-release.yml:56-59; tools/build_player_release.py:validate_executable.

Root inspected the candidate native PE imports and found VCRUNTIME140.dll. The player archive contains only the EXE/catalog/notices/docs, and its instructions require no runtime installation. A clean Windows installation without the Visual C++ redistributable can therefore fail to start before the disc picker opens. The present PE verifier checks machine/subsystem/stack/stamp but not imported non-system runtime DLLs. This is a concrete distribution gap, based on root's import inspection rather than an independently rerun check.

Recommended correction: compile the explicit x86_64-pc-windows-msvc target with static CRT (target-feature=+crt-static), applying the target flags to both release-channel tests and production builds while avoiding host procedural-macro flag propagation. Update the workflow's executable path for the explicit target. Extend PE inspection with a negative import fixture to reject unprovisioned Visual C++ runtime imports, then inspect the actual stamped release executable imports. Root is carrying out this correction; no changes were made by this reviewer.

Verdict remains request changes until all three concrete findings are resolved and root's remaining final validation gates pass.


# Scoped release fix review

Reviewed 2026-10-28 against release-fix-package.patch, current fixed files and the appended release-packaging-report.md. This is a re-review of the combined fix wave only. No source mutations, subagents, builds or test reruns were performed. This report is the only file written by the reviewer.

Verdict: all three findings ADDRESSED. No new actionable release blocker found in the fix diff. The release may proceed to root's remaining clean metadata-stamped build, production package checks and real CI/publication verification; this review does not claim those gates already passed.

## Finding dispositions

| Finding | Verdict | Evidence in current source |
| --- | --- | --- |
| P1: cross-tag publication can move latest backward | ADDRESSED | .github/workflows/enhanced-release.yml:74-79 gives every publish job the constant enhanced-release-publication concurrency group with cancellation disabled. The remote history fetch and numeric monotonic check at98-99 execute after acquiring that job lock, and publication at113 remains inside it. Per-tag build concurrency cannot bypass the shared publication lock. |
| P2: retry can publish stale/unexpected remote draft assets | ADDRESSED | Workflow100-109 checks an existing unpublished draft before upload, permits only missing expected names at that stage, and requires the exact five-name set afterward. tools/check_release_assets.py:15-35 rejects extras, duplicate names, malformed asset entries and published releases. Workflow110-112 downloads all remote assets to a fresh directory and runs the full byte/hash/ZIP/commit/version verifier before113 publishes. |
| P1: unbundled Visual C++ runtime can prevent clean-Windows launch | ADDRESSED | Workflow21 uses +crt-static; both tests61 and production build64 explicitly target x86_64-pc-windows-msvc; packaging66 uses the matching target output. tools/build_player_release.py:51-118 reads ordinary and delay imports;140-146 rejects redistributable runtime families and debug UCRT. tests244-257 exercise regular/delay runtime negatives and system-import positives. Root reports the actual static executable passes and the earlier VCRUNTIME140.dll executable fails as a negative control. |

## Fix quality and related changes

- The shell fails closed with set -euo pipefail. A missing release causes draft creation only; upload, remote exact-set check and remote-byte verification must all finish before publication. An authentication/network failure during release inspection cannot publish an unchecked release because draft creation/upload or a later guard will fail.
- check_history compares canonical versions numerically rather than lexically; fixtures include0.9.9 versus0.10.0 and reject equal or newer published versions. Drafts do not block a valid higher published version. Checks remain independent of upload timing now that the publication job is serialized.
- The remote asset helper accepts the actual gh JSON shape (isDraft and assets containing name fields). A retry may repair missing expected files but cannot silently expose an unrelated file. The downloaded directory is verified with tagged commit/version assertions using the same full package verifier already reviewed.
- Static CRT flags are applied with an explicit target on both Cargo invocations, which scopes target rustflags away from host procedural macros. The output path change is consistent with the selected target. Cargo documents this explicit-target behavior: https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags .
- Downloading all named release assets into a fresh directory matches the CLI contract: https://cli.github.com/manual/gh_release_download . Draft readback/publication remain subject to actual CI credentials and API execution, as expected for the final release gate.
- Import validation still runs in both initial packaging and the verifier used on downloaded draft bytes. The runtime rejection does not rely on the manifest's claims. Existing machine, GUI subsystem, stack reserve, commit and fork URL checks remain.
- README.txt now substitutes the exact {version} token before payload hashing. The added test commits VERSION0.1.1 in a real fixture checkout, refreshes the executable's commit stamp, packages it and checks the archive README's exact two lines. No broad string replacement of unrelated numbers is introduced.
- Full tools discovery in CI includes the new guard tests. Scratch EQOA_CONFIG and serial, visible Cargo test output improve isolation and fixture-skip reporting without changing runtime game behavior.

## Evidence and remaining gates

Read the packaging agent's appended report:40 Python tests passed, zero failures/skips, covering real Git/ZIP packaging, recomputed-tamper negatives, draft/history guards, regular/delayed imports and README0.1.1. YAML parsing, publication Bash syntax and PowerShell AST checks are reported passing. This reviewer inspected those implementations/tests rather than duplicating their runs.

Root reports the warm actual static executable has only Windows system imports and passes validate_executable with its HEAD stamp; the older dynamic executable is rejected specifically for VCRUNTIME140.dll. Root also reports full workspace completion with1,228 passed, zero failures, two ignored across91 summaries;170 default fixture omissions are reported, with the two new omitted disc-dependent checks additionally exercised in the six-test owned-disc Goto run. The actual browser build passed. These are attributed execution results, not reruns by this reviewer.

Still required before final release completion: clean committed metadata-stamped native build, real production catalog/player package verification, CI execution and verification of the public versioned assets/updater manifest. The reviewed workflow provides the necessary checks, but a source review cannot substitute for those final executions.

## Parked observations outside this fix wave

- Existing history enumeration remains capped at100 releases. This was already recorded as a future pagination consideration, not an initial0.1.0 blocker, and was not introduced by this fix.
- Actions concurrency may replace excess pending jobs; this can require rerunning an unpublished queued version but cannot reintroduce the reviewed downgrade race because each running publisher rechecks monotonic history under the shared lock.
- The import guard addresses the demonstrated Visual C++ dependency contract. It is not a general Windows loader validator, signature attestation service or guarantee against arbitrary manually crafted PE files. No new affected valid build was identified, and actual static/dynamic executable controls support the intended contract.
- Repository tag protection and external manual mutations of release state remain outside this scoped implementation review. The workflow neither force-moves tags nor modifies an already-published release through its normal path.


# CI platform recovery review

Verdict: PASS. No actionable defect found in the narrow recovery diff. The new control workflow can build the original v0.1.0 source without moving its tag, and the Windows directory test retains its intended behavioral check.

Reviewed release-ci-recovery.patch, current .github/workflows/enhanced-release.yml and tools/test_parser_launch.py, and the appended recovery evidence in release-packaging-report.md. Read-only Git inspection confirms refs/tags/v0.1.0 resolves to c7fddfc35a2f2d4185793af95b2e595a12e6a0e9. No tests/builds were rerun, no subagents were used, and no source files were changed. This report is the only reviewer write.

## Source identity

- Workflow32-35 checks out the requested dispatch tag, with the existing push ref as fallback. Validation49-57 requires canonical VERSION, exact tag/version equality, a full actual HEAD hash, a peeled refs/tags/<tag> commit equal to HEAD, and enhanced ancestry.
- Validated tag and actual source commit are emitted only after all checks succeed. EQOA_BUILD_COMMIT is exported from that actual source commit at60, before Cargo runs. The dispatch configuration's GITHUB_SHA is no longer used as the executable stamp.
- Publish109-114 consumes the validated build outputs and checks out the actual source commit. Local and downloaded manifests are asserted against that same commit/version at124 and140. All release commands use the validated release tag. The original tagged packager, release notes and guards therefore remain the publication inputs.
- The original tag contains check_release_assets.py with the reviewed draft and monotonic checks; checking out the older tag does not lose those guards. There are no tag-create, tag-move or history-rewrite operations in the recovery diff.

## Windows recovery and assertion

- Workflow39-44 canonicalizes RUNNER_TEMP after Python setup, verifies that it is an existing directory, and exports TMPDIR/TEMP/TMP for subsequent processes. This changes the runner environment without changing the original tagged test or game code. The supplied evidence reproduces the original raw-string assertion failure with an actual short-path alias and its success with canonical TEMP.
- test_parser_launch.py now compares actual child CWD using Path.samefile, so alternate spellings of the same directory are accepted. It still asserts exact quoted/backslash arguments, rejects the installation root as the working directory, and adds a child-process control proving that selecting a different parser directory does not match the prior directory. This preserves the intended directory behavior rather than weakening it to string normalization alone.
- The updated test file runs in future source revisions. Rebuilding original v0.1.0 intentionally still uses its original test; the canonical runner environment is the fix exercised for that immutable tag.

## Publication safeguards

The constant publication concurrency group remains at101-103. Monotonic history checking127, unpublished-draft validation129, exact post-upload asset checking137, full downloaded-byte verification140 and final publication141 retain their order and fail-closed behavior. Static CRT flags, explicit MSVC target, the full workspace test gate and clean-source packaging remain unchanged. Concurrent push/dispatch builds can coexist, but their publication jobs serialize and recheck remote history.

## Evidence and limits

The appended implementation report records41 Python tests passing, ten parser tests passing under a real8.3 TEMP alias, original-tag red/green path reproduction, actual workflow-script fixtures for canonical temporary paths and source identity, invalid tag/source/ancestry rejection, and YAML/Bash/PowerShell syntax checks. These execution results are attributed to the implementing agent/root and were not repeated by this reviewer.

Root reports the original tagged production package already validates its seven ZIP members and static executable, with a map capture matching the baseline. Actual dispatch of the newly committed control workflow from enhanced with tag=v0.1.0, and successful CI/public asset verification, remain the final operational checks. This review approves the recovery source change; it does not claim that recovery CI has already completed.
