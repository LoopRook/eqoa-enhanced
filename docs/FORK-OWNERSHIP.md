# Fork ownership and footprint enforcement

This record accompanies FORK.md. `origin` means upstream; `looprook` means this
fork. The root README remains the enhanced GitHub landing page by the owner's
explicit override of the original hygiene brief.

The executable policy is [fork-footprint.json](fork-footprint.json), checked by
[check_fork_footprint.py](../tools/check_fork_footprint.py). Its reviewed upstream
base is `eb5916bfd25053dcc047c3a050a2529577842306` (sync 2026-11-07, upstream's
AGPL relicence); the per-file budgets and their reasons live in the JSON. The
reviews below were measured against earlier bases.

**Reviewed budgets:** exact final shared footprints after adapter extraction,
including the approved animation guards and regressions. Existing restored map
integrations intentionally retain their larger footprints. Passing the checker
does not by itself approve a budget change; review each changed hunk first.

The 2026-10-29 addon integration review inspected each viewer hunk against
`63cdc77`: native Runtime field/init, window ownership routing, raw mouse gate,
both final-target frame hooks and exiting cleanup. Implementation remains in
`fork_ext/addons/`; the aggregate viewer footprint is +233/-12 against the
reviewed upstream base above. Later upstream ref movement is a separate sync.

The 2026-10-29 health integration review inspected the native Play receipt field,
complete stats-message observation, WorldEntry/disconnect reset, HUD cache
invalidation, and health-only gauge gate. Owned adapters extract snapshots and
apply claims; viewer only calls snapshot extraction. Reviewed totals are Play
+756/-48 and HUD +299/-8. Snapshot extraction reduces viewer to +233/-12.
These exact budgets cover the inspected hooks, not general gameplay changes.

## Fork-owned files

The manifest lists explicit ownership patterns for:

- `AGENTS.md`, `FORK.md`, `FORK-UPDATE-NOTES.md`, and the enhanced `README.md`.
- `docs/`, `design/map-parity/`, `mods/`, `bundled-mods/`, the desktop launcher, and the three named
  Windows start scripts.
- Restored app modules `map/`, `questguide/`, `groupmark.rs`, `mapsuggest.rs`,
  `statbox.rs`, and `tracker.rs`; map UI font assets and their OFL notice; map data catalog
  (crates/eqoa/resources/map-data/**) and its FrontiersForge MIT notice.
- `crates/eqoa/src/fork_ext/` adapters and implementations, including the retained
  `report_core.rs`, `report_state.rs`, `report_viewer.rs` and `report_sync_tests.rs`; reporting policy and
  its native transport fixture tests.
- The restored render map module/shader and map-circle/terrain test modules.
- The named map generators, map-bundle helper, parser launcher helper/tests,
  and footprint checker/tests. Ownership does not extend to every `tools/` file.

A new file outside these patterns requires explicit review. A binary edit in a
shared file always fails; fork-owned binary map assets remain permitted.

Versioned delivery is fork-owned too: VERSION, .github/workflows/public-source.yml,
the retired .github/workflows/enhanced-release.yml,
and the named player package/verifier/remote-release guard tools and tests.
The Windows ZIP contains runtime files only; release tooling stays in source.
The private-build migration also owns the explicit repository-identity gates in
.github/workflows/{dev,notes}.yml and tools/test_release_workflows.py. These keep
legacy upstream publication jobs inactive in both Enhanced repositories.

## Exact shared-file allowlist

Each budget is an independent maximum of added and deleted lines relative to
upstream. A replacement consumes both budgets. This list includes inherited
restoration changes, subsequent map work, the approved animation fix, and current
integration seams. No directory-wide shared allowlist exists.

| Shared upstream path | Added max | Deleted max | Reviewed purpose |
| --- | ---: | ---: | --- |
| `.gitignore` | 7 | 0 | Fork-local generated output and scratch configuration exclusions, plus `*.stackdump` (Git Bash crash notes). |
| `Cargo.lock` | 153 | 3 | Reviewed native Lua 5.1 and Dear ImGui 0.18 spike dependency resolution, including rustc-hash version disambiguation; license metadata stays unchanged. |
| `crates/eqoa/Cargo.toml` | 9 | 1 | Reviewed native-only mlua and Dear ImGui spike dependencies, Win32_System_Com for Shell known-folder CoTaskMemFree, plus existing fork/release settings; license metadata stays unchanged. |
| `crates/eqoa/build.rs` | 11 | 3 | One hook into fork_ext/app_icon_ico for the owner-supplied program icon: stage the chosen .ico in OUT_DIR and feed its images to the unchanged SIZES/dib/ico/resources path. Stack and linker behavior unchanged. |
| `crates/eqoa/src/bindpage.rs` | 16 | 16 | Restored and map input settings rows. |
| `crates/eqoa/src/binds.rs` | 93 | 80 | Restored/map controls and saved binding defaults. |
| `crates/eqoa/src/client_image.rs` | 10 | 0 | Map theme font/image resources. |
| `crates/eqoa/src/hideui.rs` | 3 | 3 | Restored map and tracker visibility. |
| `crates/eqoa/src/hud.rs` | 315 | 14 | Existing Enhanced HUD plus a local report button rectangle hook and Pick variant; drawing implementation lives in fork_ext/report_core.rs. Review follow-up adds +6/-3 for the restored Report pick assertion, 28px atlas fixture and displaced-position negative control. |
| `crates/eqoa/src/icon.rs` | 4 | 1 | Two hooks into fork_ext::app_icon: the window/Dock picture comes from the chosen icon, and poll returns before the save-icon switch. Upstream save-icon code remains present and compiled. |
| `crates/eqoa/src/input.rs` | 32 | 3 | Map controls and focus/input behavior. |
| `crates/eqoa/src/lod.rs` | 9 | 8 | Restored map terrain observation. |
| `crates/eqoa/src/main.rs` | 33 | 0 | Fork module alias for moved report core and CLI-login redaction hook; upstream logfile lifecycle remains unchanged. |
| `crates/eqoa/src/options.rs` | 202 | 42 | Existing restored option entries plus one adapter reference per fork option hook. Trimmed to measured upstream diff in the b5e6310 review follow-up. |
| `crates/eqoa/src/play.rs` | 754 | 55 | Reviewed restored state plus native health receipt/reset, cached HUD invalidation, addon hide-UI draw/click hooks for compass, active effects, chat, and HUD menus, live esf::Library icon provider hook, F9 art_changed icon cache drop, and creature journal discovery recording hook in map_frame; implementation in fork_ext and map. Trimmed to measured upstream diff in the b5e6310 review follow-up. Integration 2026-11-03 adds the R2 chat-history key tap and mode-33 ability-bar exception. |
| `crates/eqoa/src/settings.rs` | 334 | 23 | Restored map/quest settings plus one adapter reference per settings hook (re-baselined at sync 2026-10-31), and one-line in-memory report cache update on save_login. Trimmed to measured upstream diff in the b5e6310 review follow-up. |
| `crates/eqoa/src/startmenu.rs` | 1 | 1 | Expose existing bounded zone_name helper for fork reports; function body unchanged. |
| `crates/eqoa/src/touch.rs` | 81 | 2 | Reconcile Enhanced long-press release suppression with upstream full-size touch slop; preserve the map behavior and new adjustable pad size. |
| `crates/eqoa/src/ui.rs` | 10 | 6 | Map UI state and input integration plus one-line pub(crate) visibility on UiAssets.surfaces for fork-owned HUD tests. |
| `crates/eqoa/src/viewer.rs` | 255 | 15 | Existing fork integration plus native report adapter registration and one-line input, draw, screenshot/surface hooks; preserve upstream resize and Android code. |
| `crates/esf/src/mesh.rs` | 9 | 0 | Terrain mesh lookup for restored map. |
| `crates/net/src/session.rs` | 449 | 30 | Restored group/map/quest protocol state and upstream unanswered offers test (re-baselined at sync 2026-10-31). |
| `crates/render/src/fans.rs` | 0 | 0 | Restored UI render behavior (re-baselined at sync 2026-10-31). |
| `crates/render/src/lib.rs` | 50 | 4 | Restored map rendering pipeline and exports. |
| `crates/render/src/ui.rs` | 97 | 8 | Map circle clipping and map font rendering. |
| `crates/render/src/ui.wgsl` | 16 | 7 | Circle clipping in UI shader. |
| `crates/vi/src/csprite.rs` | 85 | 11 | Animation bounds guards and negative-control regression tests. |
| `crates/vi/src/hsprite.rs` | 69 | 3 | Animation bounds guards and negative-control regression tests. |
| `crates/vi/src/ui.rs` | 3 | 2 | Restored player-facing UI behavior. |
| `release/README.txt` | 1 | 1 | Enhanced Windows bundle startup instructions. |
| `web/main.js` | 48 | 0 | Restored browser input/map bindings. |
| `crates/eqoa/src/update.rs` | 4 | 4 | Fork release routing and fork-owned update notes; upstream updater mechanics unchanged. One test line skips Nightly's non-dated `## Unreleased` section; whats_new stays upstream's exactly. |
| `crates/eqoa/src/updatewnd.rs` | 3 | 2 | Fork release page link with fork/vanilla UI regression assertions. |
| `crates/eqoa/src/logfile.rs` | 9 | 1 | Upstream logger retained around eight hook lines: initialize fork crash capture before rotation, share the exact run ID, redact/feed the ring, add Enhanced header, capture panic, flush journal on close. One one-line swap replaces upstream's dev-commit log label with fork_ext::version's player-facing label: Nightly names its build commit, while Stable and local builds show VERSION. Its +1 added and +1 deleted over the previous eight-line budget is the minimum for that swap. |
| `crates/eqoa/src/pregame/station/api.rs` | 4 | 0 | Credential-noting hooks at login, account creation and password change preserve local report redaction after upstream removed them. |

`CLAUDE.md` is fork-owned (since 2026-10-30). Upstream's CLAUDE.md is kept
verbatim in `docs/upstream/CLAUDE.md`; the checker (`claude_upstream_copy`) fails
unless that copy equals upstream's at the reviewed base. `UPDATE-NOTES.md` and `LICENSE` must match
upstream. The checker also compares `license` and `license-file` values in both
package/workspace-package sections of every upstream Cargo manifest; dependency
or feature budgets cannot authorize a license change. Third-party license notices
remain intact in the fork-owned resource and documentation trees.

## Named extension seams

The manifest requires each named hook's declared occurrence count in its shared
parent file. This is a targeted guard, not a Rust parser; normal build/test review
still verifies execution and placement. Do not add duplicate hook names in comments.

| Shared file | Checked seams |
| --- | --- |
| `crates/eqoa/src/main.rs` | `mod fork_ext;`, `mod report_policy;` |
| `crates/eqoa/src/play.rs` | `self.fork_combat_event(...)`, `fork_hud::chat_history_key(...)`, `fork_hud::ability_bar_shown(...)` |
| `crates/eqoa/src/hud.rs` | `fork_hud::draw(...)` |
| `crates/eqoa/src/options.rs` | `fork_options::{CATEGORY, ITEMS, change, activate, value_text}`, once each |
| `crates/eqoa/src/settings.rs` | `fork_settings::{write, parse}`, once each |
| `crates/eqoa/src/viewer.rs` | `fork_report::surface(...)`, report input/capture hooks |
| `crates/eqoa/src/hud.rs` | `crate::fork_ext::report_core::hud_button` |
| `crates/eqoa/build.rs` | `app_icon_ico::{stage, images}`, once each |
| `crates/eqoa/src/icon.rs` | `fork_ext::app_icon::{picture, keeps_icon}`, once each |

`fork_ext/{play,hud,options,settings}.rs` contains app-private adapters imported as
child modules by the corresponding shared parent. `fork_ext/mod.rs` owns the common
mods/telemetry boundary. Saved setting keys and enhanced option IDs (1000+) remain
stable. Existing restored/map/quest code is outside this one-call cleanup; its
shared touch points are documented by the allowlist above and MAP-RELEASE-HANDOFF.md.
Future map isolation behind a crate or feature flag remains a separate candidate.

The updater also crosses the fork boundary once through fork_ext::release::base;
its notes and release-page link are fork-owned. Released builds require channel
enhanced; local and unrelated build channels do not start an updater.

The release manifest additionally checks the updater channel resolver, compiled
fork notes and update-window release page with the exact counts in fork-footprint.json.

## Run and review

From the repository root:

```powershell
python -Xutf8 tools/test_fork_footprint.py
python -Xutf8 tools/check_fork_footprint.py
python -Xutf8 tools/check_fork_footprint.py --show-diff
```

The default report lists actual `+added -deleted` counts and the reviewed budget
for every changed shared file. `--show-diff` also prints their exact changed lines.
The comparison is `git diff origin/master` against the current working tree,
including staged and unstaged changes, plus `git ls-files --others
--exclude-standard` for untracked files. It does not use a merge-base/triple-dot
comparison, which could hide upstream changes after synchronization. Git-ignored
local data is outside the review and must remain untracked.

The checker only reads files and Git state. It fails on an unreviewed shared path,
a budget overrun, changed upstream base, immutable content, changed Cargo license,
or a missing/duplicated named hook. Its self-tests create real disposable Git
repositories with positive controls and negative controls for these protections.

After each upstream merge or intentional integration edit:

1. Review `git diff --no-renames --numstat origin/master` and the exact shared
   diff using `--show-diff`. Keep new implementation in fork-owned files.
2. Review each changed shared path and its reason. Change only the corresponding
   explicit budget in `docs/fork-footprint.json`; reduce it after extraction.
   Never increase budgets just to make a failure disappear. Unknown paths need
   an ownership decision and a specific integration reason.
3. If upstream advanced, review the entire integration diff before updating
   `reviewed_base` to the full `git rev-parse origin/master` value.
4. Update this exact allowlist/table and FORK.md's integration record as needed.
   Finalize `review_state` only after the final code/budget review.
5. Run both commands above, then the applicable native/browser checks.

There is no budget refresh or auto-approve switch. Conflicts in shared files should
retain upstream's implementation and reapply the documented seam; preserve both
intents where an existing restored system still requires inline integration.

## 2026-11-03 report ownership transition

Upstream deleted report.rs and web/report.js. Enhanced keeps the native core in
fork_ext/report_core.rs; fork_ext/report_viewer.rs owns event routing and capture.
Shared logfile.rs gets eight added hook lines and Station api.rs four credential
hooks. main.rs adds three lines for the alias/CLI secret; HUD adds two lines for
the retained pick/draw seam. viewer.rs grows by 20 additions/3 replacements for
input ownership, capture and draw/quiet hooks. touch.rs grows by two default
initializers to keep Enhanced map holds/d-pad alongside upstream's new constructor.
Settings description hooks disappeared with upstream's unused report describer.
These are reviewed integration costs for retaining the owner's local-only reports,
not a general permission to expand shared code. Web deletion is accepted because
the Enhanced release workflow ships Windows x64 only.


Review follow-up: options +194/-42, play +750/-53 and settings +322/-17 are
trimmed to the measured diff. HUD +315/-14 includes the restored pick test:
its nonzero-height atlas and Report assertion add six lines/replace three
relative to the merge budget. The HUD button and capture surface hooks are now
checked explicitly. The local report core keeps only the report tail, redaction,
crash files and dialog; logfile alone owns native stderr and the panic hook.

Owner-requested report snapshot (2026-11-03): `fork_ext/report_state.rs` is a
child of the fork Play adapter. Its existing `frame_library` call now forwards
the walker, with one throttled `crate::report::note_state` call in fork-owned
code. No new shared hook line or budget increase; play remains +752/-54.
The visibility adapter publishes an allocation-free bitmask of claimed HUD
elements; names are formatted only when a snapshot is accepted.

Integration snapshot review: fork_ext/report_background.rs owns the bounded I/O
worker and 30-second addon cache. The existing frame_library line forwards the
walker in place; play remains +752/-54 after the R2 integration. startmenu.rs
exposes its existing zone_name helper (+1/-1) so reports share the exact bounds
rule; its body is unchanged. map/mini.rs shares Dot::kind between map and report.

Owner-approved defaults export: options adds eight test assertions/setup lines;
settings adds two named defaults and changes default/serialization comparisons
and invalid-input test expectations. Reviewed increments are options +8/-0 and
settings +12/-6 relative to the previous policy. Retail lower bounds remain.
The defaults export also makes two existing Play tests select retail slots and
distance explicitly (+2/-1). They previously inherited old defaults; their
retail-behavior assertions remain unchanged. Production Play code is unchanged.



## 2026-11-09 map restoration sync review

Pinned upstream base: `91f9e72dd6d111e20a37b73cea461444a18ad79c` (134 commits).
The owner selected combine: retain Enhanced map/search/journal/theme/circle and
port upstream master switches, offered GM travel, cache precedence, quest rim and
O/G layout. Detailed decisions and verification are in
[the sync report](plans/sync-2026-11-09-report.md).

The upstream map restoration makes formerly fork-added code part of the baseline.
Existing substitutions therefore consume more deletion budget. Reviewed maxima:
HUD +316/-44 (previous -15), options +46/-142 (previous +41/-41),
Play +759/-89 (previous -53), settings +334/-38 (previous -23).
The manifest gives each reason: preserve HUD claims/circle fixtures, represent
colliding stock map rows through owned rows and tests, keep search/journal/live
dot adapters, and retain Enhanced saved settings rather than vanilla assertions
that they disappeared. Lighting IDs 56/57 remain upstream's; existing fork IDs
remain unchanged, masters use 1054/1055. Every page still has at most six rows.
The extra options references also cover the new combined-registry uniqueness test
and explicit 24/100 retail-boundary fixtures under Enhanced's 64/650 defaults.

A new shared `texpack.rs` budget is +1/-1 for a Windows-portable test fixture:
Unix-shaped absolute fixture paths gain a drive on Windows. Its AppImage and
relative-path controls then exercise the same production helper on this host;
production texture code is unchanged. The master category has its own named hook
guard. Renderer and network budgets did not increase.
