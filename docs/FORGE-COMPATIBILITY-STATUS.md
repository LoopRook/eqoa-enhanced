# Forge compatibility status

Last updated: 2026-10-28

## Authority and scope

- User approved the Forge compatibility initiative on 2026-10-28.
- Plan: [Forge Compatibility plan](superpowers/plans/2026-10-28-forge-compatibility.md).
- Implementation worktree: `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`, branch `forge-compatibility`.
- Planned base: enhanced commit `63cdc77d96ba00de720e50aaffb7b06897479b96`.
- Root agent owns reviews and future assignments. No production implementation or runtime success is claimed in this ledger.

## Current steering

- Both native maps remain the defaults. Forge `world_map` and `mini_map` are optional capability comparisons/proof cases, with no required ports or replacements and no blocker status for generic addon-host completion.
- Required runtime target: FF_Example, non-map included mods, and the native author API. Compatibility reports must keep unsupported APIs visible for either Forge map and avoid universal Forge-parity claims.
- Prioritize the render-scale bugfix before Stage 1's Lua/render feasibility spike.

## Research and workers

Stage 0 source research is complete. The machine-readable API/mod matrix was due when this ledger was first written; a matrix file is now present in the shared worktree, but its review/acceptance state is not established by this documentation update. Stage 0 remains open until the matrix is reviewed.

Research coverage recorded in the plan:

- Terra: `ff_example` API and semantic inventory.
- Flash 3.8: included-mod inventory.
- Luna: native repository/mod integration inventory.
- Sol: architecture review.
- Lead: review, sequencing, integration, and evidence; root retains reviews and future assignments.

At the time of the original ledger entry, no addon host, Forge adapter, or compatibility runtime had been implemented or playtested. This update does not validate runtime behavior. Static source inspection must remain distinct from native runtime, live-game, and visual parity validation.

## Source-state audit

The local audit was read-only and used the refs available locally; it did not fetch. The original enhanced worktree, `forge-map-parity` worktree, and Forge Compatibility worktree all had HEAD `63cdc77d96ba00de720e50aaffb7b06897479b96`. The first two were clean. Forge Compatibility had one untracked file, this plan at the time of audit; there were no tracked source edits. The `codex/forge-map-parity` branch at `8aeaf9181f401e4b8b93f92347d407cc81eb4856` is an ancestor of enhanced and its commit is already included in that branch history.

At audit time, local `looprook/enhanced` matched HEAD. It was 28 commits ahead and 3 behind the locally available `origin/master` ref. The published `v0.1.0` tag remained at `c7fddfc35a2f2d4185793af95b2e595a12e6a0e9`; enhanced had four later commits. This distinguishes current source history from the published game code.

Separately, the root agent reported a live `ls-remote` check: remote enhanced and preview heads both resolved to `63cdc77`. That live remote observation is separate from the local ref comparison above; this ledger's author did not independently run the network check.

Paseo's displayed change count was also checked against its comparison bases: `+25,685/-1,226` equals enhanced versus master. Enhanced versus `looprook/enhanced` is `0/0`, confirmed both by local comparison and the root-reported live remote check. The large count describes the branch's accumulated difference from master; it is not a count of uncommitted Forge Compatibility edits.

At the time this ledger was written, the worktree also showed a tracked modification in `crates/eqoa/src/map/mini.rs`, adding a render-scale minimap test. It was not part of this documentation change and has not been runtime-verified here; its presence is recorded without attributing authorship.

## Next gates

1. Deliver and review the machine-readable API/mod matrix to close Stage 0.
2. Prioritize the render-scale bugfix and record focused evidence.
3. Proceed to the Lua/render feasibility spike only after that bugfix, following the plan's implementation and review gates. Keep Forge map investigations optional and report their unsupported capabilities explicitly.

All entries above describe planning and source-state evidence only. Runtime behavior and Forge compatibility remain unverified; neither map has been ported or validated for parity.

## Verified progress: 2026-10-28

- Source-state question resolved: Paseo All changes exactly matches enhanced versus master (+25,685/-1,226); enhanced versus live looprook/enhanced is 0/0. No main-worktree push is missing.
- Matrix baseline delivered and parsed: schemaVersion 1, 9 included mods, 21 direct FF_Example imports; world_map and mini_map explicitly optional with no required rollout priority. Method is lexical scanning plus manual review, not AST analysis or runtime proof.
- Native minimap fix: terrain remains normalized in physical window coordinates; UI mask follows actual target dimensions queried from TextureView. Red regression failed at x2; final 13 focused mini tests passed, including GPU checks through 1/2/4/2/1. Antigravity independent spec and quality review approved. Full workspace exhaustive EE test was stopped, so no full-suite claim. See MINIMAP-RENDER-SCALE-FIX.md.
- Stage1 Lua/ImGui bridge remains in progress. Sol owns current implementation/build; after its bounded report, Antigravity will own the next task. No full Forge compatibility, live gameplay comparison, release, push, or merge is claimed.
- User budget steering: stagger implementation/builds, prefer Antigravity Flash 3.8 for remaining bulk work; use Sol only for bounded integration needs and Terra/Luna for small constrained work.

## Bounded Stage 1 review

Lead reviewed the four native-only pinned Lua/ImGui dependency additions and lock resolution: fourteen new locked packages; existing rustc-hash references are version-disambiguated, not upgraded. Viewer additions only store/init the opt-in backend, forward relevant events and render after supersampling. Antigravity's independent specification/quality review approved the constrained spike. Six focused tests, native development build and WASM cargo check passed; live DPI/input/package acceptance and general addon support remain open. Exact shared footprints were explicitly reviewed as Cargo.lock +153/-3, eqoa/Cargo.toml +8/-0, viewer.rs +221/-12 relative to the unchanged reviewed upstream base. Ownership policy records these reviewed hunks; no render-crate or license change was authorized.

Input release bookkeeping is retained for the spike; per-mod press/release ownership is a Stage 2 review item. Protected calls/FFI/general file resources remain explicitly unsupported in this constrained spike. Current priority is the separate fix-only preview build at a7feb4d; no merge, push, tag or publication is approved yet.

## Local minimap preview ready

User requested a local fix-only playtest before any enhanced merge/release. Clean branch fix/minimap-render-scale at a7feb4d72a2f4602766412ea7c826715d51f1c2f contains only minimap fix/tests/callers/notes. Antigravity built the release executable and package; 13 minimap tests and broader 113 map tests passed, with one no-disc fixture skip. Preview footprint check passed. Full exhaustive workspace suite and live gameplay were not run.

Preview directory: C:/Users/hocka/.paseo/worktrees/1usktejc/minimap-scale-fix-preview/run/minimap-scale-fix-preview. Launcher: Play Map Preview.cmd (isolated profile). ZIP: map-preview-c8b84f50e2b9.zip. Root independently verified clean manifest source commit, no private/profile/disc ZIP paths, executable SHA256 c8b84f50e2b95a0ac01e827785156d7ef3788e6916e2d5179afc7796d8b9af7f and ZIP SHA256 238131c0f2483ac4e692eef7fe9414621e2af02a975b4922a49bac1eda44a87c. No merge/push/tag/release yet; publication awaits user's playtest approval.

Forge spike's reviewed footprint checker also passed against 7912c7393df7d85655d5816980fae1217cc48a32 after explicit dependency/hook review. Next compatibility work is Stage2 discovery/lifecycle via staggered Antigravity implementation; retain native maps and report current spike limits.

## User playtest: world-map failure blocks release

User confirmed the native minimap works in preview a7feb4d, but supplied a screenshot showing the full world-map terrain reduced into the upper-left while labels/controls remain full-size and the game scene is visible underneath. Treat previous world-map acceptance as unverified/failed; no release approval was given. Antigravity cef1254c-be20-41d4-aa60-f340be6e00e2 owns diagnosis, failing GPU regression, narrow world-map fix and rebuilt fix-only preview on fix/minimap-render-scale. This is the only implementation/build worker. Preserve profiles and old content-addressed artifacts. Both native maps must pass the next live playtest before any enhanced merge/publication. Forge runtime work remains separate.

World-map followup 773674e631e778c6bc30640712192aa45f71e279 now has failing-before/fixed-after square and wide GPU regression across1/2/4/2/1;114maptests pass with one expected no-disc fixture skip. Independent Antigravity spec/quality review approved. Rebuilt clean preview manifest and EXE SHA verified by root: eqoa-map-preview-f77eed3bd8b6.exe, SHA256 f77eed3bd8b66eeb8108976a364cec98da616d7815a9d635b2e0bc6968b5e8c2. Same Play Map Preview.cmd now points to this build; ZIP map-preview-f77eed3bd8b6.zip contains no profile/disc/account paths. Await user playtest of BOTH maps; no merge/push/release.

Stage2A pure discovery assigned to Antigravity36323821 with ignored discovery-brief.md as requirements. No other implementation/build workers active. Keep source-side world-map followup available for later integration into Forge branch, which currently still has its original Map::draw signature. Native preview remains independent.

## Owner approved merge and publication

User confirmed the rebuilt preview works and explicitly instructed: merge into enhanced and do a quick release. That approval supersedes the prior publication hold and authorizes the isolated map fixes plus normal patch-release metadata/push/tag/CI publication. Antigravity cef1254c is preparing the release from fix/minimap-render-scale773674e, checking live latest/version availability, scoped0.1.1 metadata/tests, then reporting for root review before push. No further user approval is required for this requested publication. Preserve immutable tags; use only LoopRook/eqoa-enhanced and enhanced release workflow. No Forge code in release. Discovery worker36323821 instructed to finish current bounded work and idle so release remains primary/staggered.

Updater answer: official0.1.0 CI builds embed enhanced channel and exact commit, follow the fork latest/download/update.txt, validate payload hash/size, check normal startup and periodically offer restart. Local preview builds have updating disabled. Verify actual published0.1.1 manifest/assets and compatibility before final release success statement; do not claim real players have updated.

Release recovery: cef1254c provider errored during tooling tests with a Windows taskkill unsupported-operation error. Root checked both worktrees: no metadata edits/commits/merge/push/tag had occurred, enhanced still63cdc77 and fixbranch773674e/VERSION0.1.0. Fresh Antigravity9ee7c0d9-3719-4927-95e7-d50647458a71 now owns release preparation from that verified state; do not steer/interrrupt it while active. Await its PREPARE report, root scope review, then sendPUBLISH NOW under existing explicit owner authorization.

User supplied texture-pack reference C:/Users/hocka/Github Projects/EQOA Native Rust/Testing Resources/eqoa-hd-pack-v2.7.zip (2,217,985,448bytes). Only filename/size inventory performed; no extraction, content change or feature work. User explicitly says keep current priorities; investigate after release/Forge unless a small compatible addition. Preserve this known untracked folder, do not stage/delete/stash it. Run release footprint checks from clean fixworktree. DiscoveryStage2A reported6tests green/buildlockfree and remains idle pending later review; current priority is publication.

Release PREPARE reviewed: metadata58fdbac selected0.1.1 after live latest0.1.0/tagunused check. OriginalenhancedFFmerged to58fdbac; only11mapfix/tests/release-docfiles changed from63cdc77, noCargo/Forgechanges, knownTestingResources untracked. FootprintPASS,41Python toolingtests green,22updater/releasetests green,114maptests previousgreen. Root reviewed exactdiff and sent PUBLISH NOW toAntigravity9ee7c0d9 with tinyplayernoteswordingcleanup allowed beforetag. It now owns pushenhanced+immutablev0.1.1, existingCI fullsuite/build/publication, publicZIP/checksum/manifestverification. Awaitactualcompletion; don'tduplicatepush/tag or claimreleasepublished beforeverified. Followupdocpolish may change exactreleasehead from58fdbac. Old0.1.0annotatedtagobject187ceda mustnotbeconfusedwith peeledsourcec7fddfc.


## Release 0.1.1 completed and texture-pack findings

- **Release 0.1.1 published**: Following user authorization, `docs/releases/0.1.1.md` received docs-only player notes polish (`fcd3923eb76ba037e49f1cb7380c829f66c5980b`). Atomic push of `enhanced` and immutable tag `v0.1.1` to `LoopRook/eqoa-enhanced` succeeded. GitHub Actions CI run `40786055566` completed green across full workspace tests and packaging. Release `v0.1.1` is published as non-draft, latest release with all 5 verified assets (`EQOA-Enhanced-0.1.1-Windows-x64.zip`, `eqoa-enhanced-windows-x86_64.exe`, `update.txt`, `update-notes.md`, `SHA256SUMS`). Independent post-release verification confirmed `update.txt` resolves version `0.1.1`, commit `fcd3923`, payload hash `80c03483c8a36adcc859b021b6e03d88f674302956ff87c58f5cc2a4c4a69ecd`, and byte count `23871488`. The update format remains fully compatible with official 0.1.0 builds; no live player client upgrades were observed or simulated.
- **Texture-pack technical assessment**: Root read-only central-directory inspection of `Testing Resources/eqoa-hd-pack-v2.7.zip` verified it contains `SLUS-20744/replacements/` hashed PNG/JPEG textures formatted for PCSX2 replacement. The native engine's `LibraryTextures` decodes raw `esf::Library` assets, and no replacement texture loader exists in native client code. Native texture replacement support remains completely deferred; the archive remains untracked and untouched.
- **Map priority preserved**: Both native maps remain the primary defaults. Forge map implementations remain optional comparison/proof mods only. No changes have been merged into the Forge branch or worktree, and no release changes remain pending.

## Stage 2B Package Loader & Resource Resolver Verified

- **Per-Addon VM Isolation**: Delivered `Host::for_addon(&DiscoveredAddon)` in `crates/eqoa/src/fork_ext/addons/host.rs` coexisting with `Host::new()`. Each addon receives an independent Lua 5.1 VM with isolated globals, callback table, and `package.loaded` cache.
- **Ordered Module Resolution (`require`)**: `crates/eqoa/src/fork_ext/addons/package.rs` resolves module candidates `<name.replaceDotsSlash>.lua` then `<name>/init.lua` in private then shared `module_dirs`. Enforces strict dotted syntax without parent traversal or slashes.
- **Native Reserved Modules**: Native registry controls `frontiers_forge.*` (util snapshot-backed, unknown fails naming unsupported API), `imgui`, and `uiforge`. Deceptive or mock definitions on disk are strictly ignored. `ffi` and OS loaders remain unavailable.
- **Containment & Protection**: Module reads bounded to 1 MiB (`Read::take`); recursion depth limited to 32; canonical containment within `script_root` verified on every load; cycles formatted and detected (`A -> B -> A`); sentinels cleared on failure; nested `require` shares `Host.run`'s instruction quota without resetting.
- **Contained Resource Resolver**: `resolve_resource` provides Rust-only relative path resolution across private then shared `resource_dirs`, rejecting absolute/UNC/drive/parent traversals across both separators on all operating systems.
- **Verification**: Red-to-green test cycle completed for 8 focused `forge_package_` tests; full `fork_ext::addons::` suite passed with 26 tests green (0 failed, 4 symlink tests skipped for Windows privilege 1314, 1 GPU fixture skipped). See `docs/FORGE-PACKAGE-LOADER-REPORT.md`.

## Stage 2C Native UI-Addon PNG Textures Verified

- **Addon File Texture Loader**: Added `Textures::install_for_addon(owner, &Lua, &DiscoveredAddon)` in `crates/eqoa/src/fork_ext/addons/textures.rs`. Installs `UiForge.LoadTexture(path)` and legacy alias `UiForge.IGraphicsApi.CreateTextureFromFile(path)`.
- **Path Resolution & Containment**: Relative paths resolve via `package::resolve_resource`; legacy absolute paths using `UiForge.resources_path` (`<script_root>/resources`) are checked for active script root and resource directory containment on every load. Normalizes separators, rejects `..` parent traversal and foreign roots. Absolute paths do not prefer private files.
- **Strict PNG Decoding**: Decodes PNG files (bounded to 16 MiB read) via `png 0.18.1`. Unsupported formats (JPEG) and corrupt files return `nil` with diagnostics (matching upstream `LoadTexture` returning `nullptr`). Normalizes RGBA8, RGB8, Grayscale, and Indexed formats.
- **GPU Resource Management & Budgets**: Explicit `TexturesBudget` defaults to 2048 max dimension (clamped to GPU hardware limits), 128 handles, and 64 MiB total decoded bytes. Opaque owner handles; deferred release; `collect` accounting resets; 32-entry bounded diagnostics ring with `drain_diagnostics`.
- **Verification**: Red-to-green cycle completed for 6 focused `forge_texture_` tests. Full serial addon test suite (`--test-threads=1`) with command-local `EQOA_FORGE_SPIKE` completed with 32 passed, 0 failed, 4 symlink skips for Windows OS error 1314. Pinned grid GPU fixture ran and captured `target/forge-spike-grid.png`. See `docs/FORGE-TEXTURE-RESOURCES-REPORT.md`.

## Stage 2C Texture Resources Remediation Verified

- **Platform-Aware Absolute Path Normalization ([HIGH-01])**: Implemented `normalize_path_str` and `normalize_texture_path_for_platform`. Preserves Windows verbatim (`\\?\`) and UNC (`\\`) prefixes while normalizing legacy Unix script backslashes (`\`) to forward slashes (`/`). Direct helper test and production absolute alias test verified.
- **Transactional UserData Rollback ([MED-01])**: Implemented `rollback_upload(id)` and `upload_and_create_userdata` helper. Drops `bank` borrow prior to calling `lua.create_userdata`. If Lua allocation fails (e.g. hitting memory quota), unregisters the external texture from `WgpuRenderer`, removes the entry, and decrements allocated bytes. Applied to both `CreateTextureFromMemory` and `LoadTexture`.
- **Diagnostic Ring Deduplication Ordering ([MED-02])**: Checks `!self.diagnostics.contains(&msg)` prior to checking capacity and evicting the oldest entry, preventing duplicate spam from shrinking the buffer or evicting older diagnostics.
- **Pre-Reserved ID Counter ([LOW-01])**: Computes `self.next.checked_add(1)` before WGPU texture creation and external texture registration; commits counter only after successful registration, preventing orphan external textures on counter exhaustion.
- **Byte Memory Budget Isolated from Count**: Verified that `max_total_bytes` memory budget limit triggers `nil` and logs diagnostics when byte capacity is exceeded while handle count has capacity.
- **Framebuffer Color Readback Verification**: Verified `forge_texture_explicit_shared_absolute_path_alias` via headless WGPU render and readback, asserting rendered pixels are shared BLUE (`[0, 0, 255]`) and NOT private YELLOW (`[255, 255, 0]`).
- **Verification**: Full serial addon test suite (`--locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1`) with command-local `EQOA_FORGE_SPIKE` passed with 41 passed, 0 failed, 4 Windows symlink privilege skips (os error 1314), and 0 fixture skips (`forge_gpu_pinned_grid_and_texture_fixture` ran and captured `target/forge-spike-grid.png`). All 12 texture tests passed. See `docs/FORGE-TEXTURE-RESOURCES-REVIEW.md` and `docs/FORGE-TEXTURE-RESOURCES-REPORT.md`.

## Stage 2D backend lifecycle manager — implemented and locally verified

- Added `manager.rs`/`storage.rs` and native addon module declarations only; no app/viewer/options wiring, dependency, frozen Host/Bridge/Textures, policy or map changes.
- Backend reuses caller-owned Context/platform/shared renderer, lazily instantiates at most 16 VMs, queues lifecycle commands at frame boundaries, preserves globals on normal disable, distinguishes DisableScript from final OnEject, and defers forced texture retirement to `after_render`. Compile-before-replace reload and removed/duplicate rescan preserve healthy neighbors.
- Storage uses bounded plain JSON, a schema-1 enabled registry, canonical length-delimited SHA-256 identities and same-directory create_new/write/flush/sync/rename replacement. Repeated Windows replacement, failed-write preservation and numeric-key profiles have controls.
- RED observed: five storage/four manager stub failures, plus a callback-retirement regression. Focused GREEN: five storage and five manager tests. Final `cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1` with command-local pinned `EQOA_FORGE_SPIKE`: **51 passed, 0 failed**, four explicit Windows symlink1314 privilege skips, no pinned-GPU/manager fixture skips. Actual shared-WGPU readback proves a healthy addon renders after its neighbor fails with an open Begin, while already-emitted textures survive until post-render retirement.
- See `docs/FORGE-LIFECYCLE-MANAGER-REPORT.md` for exact APIs, callback/cleanup ordering, limits and evidence. This is a backend slice, not player-ready/full-FF compatibility. No release/build/package/account/server actions; build slot free after verification.

## Stage 2E native app integration verified (source only)

- Added lazy native Runtime, single managed Context/backend, Addon Manager action1053, controls and intentional settings window. Empty/disabled-only roots allocate no graphics until requested. Initial physical press ownership preserves repeats/releases and game-held drags; focus/lifecycle bookkeeping remains intact.
- Native mods root is executable-relative even when missing; companion launch behavior remains tested. Existing config directory owns profiles. Small native viewer hooks cover pregame/live final-target rendering, actual snapshots, events/raw motion and explicit shutdown. No backend/dependency/render/gameplay/map/policy edits in this phase.
- Final command-local pinned-GPU serial `cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1`: **61 passed, 0 failed**, four explicit Windows1314 symlink-operation skips; pinned GPU and common runtime settings/physical-target GPU fixtures executed. Companion/path tests **5 passed**; fork-options tests **3 passed**. Native `cargo build --locked -p eqoa` and WASM `cargo check --locked -p eqoa --target wasm32-unknown-unknown` both passed; `git diff --check` passed.
- Footprint checker FAIL14 against advanced origin/master recorded honestly; lead owns reviewed-base/shared-diff policy review. No budget relaxation. Published0.1.1 map fixes still need integration before packaging. No native live-window/input/DPI/account/server acceptance, preview packaging, FF_Example or health-hearts success claim.
- See `docs/FORGE-APP-INTEGRATION-REPORT.md` and `docs/FORGE-ADDONS-TESTING.md` for exact commands, scope, install steps and remaining gates. Build slot free; no commit/merge/push/release performed.
### 2026-10-29 root integration checkpoint

Root reviewed the Stage 2E viewer hooks and recorded the exact +234/-12 shared
footprint against upstream 7912c7393df7d85655d5816980fae1217cc48a32. The footprint
checker passes against that reviewed base; newly advanced origin/master is a
separate upstream sync. Runtime error recovery was corrected and reproduced by
same-Context GPU regressions. Root's combined locked, serial addon run passed
70 tests, including the pinned grid and all GPU fixtures; four Windows symlink
operations explicitly skipped for privilege error 1314. Health bindings are
unit-integrated but not yet wired to real client snapshots or HUD visibility.
This is a local foundation checkpoint, not full Forge compatibility or a release.
Published enhanced 0.1.1 map changes will be merged before any preview package.

### 2026-10-29 published map release integration

Merged published enhanced 0.1.1 source fcd3923eb76ba037e49f1cb7380c829f66c5980b
into the Forge branch after foundation checkpoint 3dc2a70. Resolution retained the
unreleased addon notes, adopted the released Map::draw target-derived signature,
and replaced the earlier minimap-only report with the released combined report.
The native minimap and world map remain defaults; their published fixes are now
included. Root verification after merge: 114 map tests passed (one explicit
missing-disc prerequisite skip), locked WASM check passed (one existing unused
variable warning), and reviewed-base footprint check passed. No Forge push,
public release, or full compatibility claim was made.
