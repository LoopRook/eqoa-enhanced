# Fork ownership and footprint enforcement

This record accompanies FORK.md. `origin` means upstream; `looprook` means this
fork. The root README remains the enhanced GitHub landing page by the owner's
explicit override of the original hygiene brief.

The executable policy is [fork-footprint.json](fork-footprint.json), checked by
[check_fork_footprint.py](../tools/check_fork_footprint.py). Its reviewed upstream
base is `7912c7393df7d85655d5816980fae1217cc48a32`.

**Reviewed budgets:** exact final shared footprints after adapter extraction,
including the approved animation guards and regressions. Existing restored map
integrations intentionally retain their larger footprints. Passing the checker
does not by itself approve a budget change; review each changed hunk first.

## Fork-owned files

The manifest lists explicit ownership patterns for:

- `AGENTS.md`, `FORK.md`, `FORK-UPDATE-NOTES.md`, and the enhanced `README.md`.
- `docs/`, `design/map-parity/`, `mods/`, the desktop launcher, and the three named
  Windows start scripts.
- Restored app modules `map/`, `questguide/`, `groupmark.rs`, `mapsuggest.rs`,
  `statbox.rs`, and `tracker.rs`; map UI font assets and their OFL notice; map data catalog
  (crates/eqoa/resources/map-data/**) and its FrontiersForge MIT notice.
- `crates/eqoa/src/fork_ext/` adapters and implementations; reporting policy and
  its transport tests; browser reporting policy and its tests.
- The restored render map module/shader and map-circle/terrain test modules.
- The named map generators, map-bundle helper, parser launcher helper/tests,
  and footprint checker/tests. Ownership does not extend to every `tools/` file.

A new file outside these patterns requires explicit review. A binary edit in a
shared file always fails; fork-owned binary map assets remain permitted.

Versioned delivery is fork-owned too: VERSION, .github/workflows/enhanced-release.yml,
and the named player package/verifier/remote-release guard tools and tests.
The Windows ZIP contains runtime files only; release tooling stays in source.

## Exact shared-file allowlist

Each budget is an independent maximum of added and deleted lines relative to
upstream. A replacement consumes both budgets. This list includes inherited
restoration changes, subsequent map work, the approved animation fix, and current
integration seams. No directory-wide shared allowlist exists.

| Shared upstream path | Added max | Deleted max | Reviewed purpose |
| --- | ---: | ---: | --- |
| `.gitignore` | 6 | 0 | Fork-local generated output and scratch configuration exclusions. |
| `Cargo.lock` | 2 | 0 | Fork dependency resolution; license metadata stays unchanged. |
| `crates/eqoa/Cargo.toml` | 4 | 0 | Fork dependency and release build settings; license metadata stays unchanged. |
| `crates/eqoa/src/bindpage.rs` | 16 | 16 | Restored and map input settings rows. |
| `crates/eqoa/src/binds.rs` | 93 | 80 | Restored/map controls and saved binding defaults. |
| `crates/eqoa/src/client_image.rs` | 10 | 0 | Map theme font/image resources. |
| `crates/eqoa/src/hideui.rs` | 3 | 3 | Restored map and tracker visibility. |
| `crates/eqoa/src/hud.rs` | 294 | 7 | Existing restored map/quest/group/Stats Box integrations plus one fork HUD adapter. |
| `crates/eqoa/src/input.rs` | 32 | 3 | Map controls and focus/input behavior. |
| `crates/eqoa/src/lod.rs` | 9 | 8 | Restored map terrain observation. |
| `crates/eqoa/src/main.rs` | 30 | 0 | Restored module registration, fork_ext boundary, and report policy module. |
| `crates/eqoa/src/options.rs` | 196 | 43 | Existing restored option entries plus one adapter reference per fork option hook. |
| `crates/eqoa/src/play.rs` | 745 | 47 | Restored/map state and observations plus one combat event adapter. |
| `crates/eqoa/src/report.rs` | 18 | 9 | Fork reporting policy transport integration. |
| `crates/eqoa/src/settings.rs` | 333 | 22 | Restored map/quest settings plus one adapter reference per settings hook. |
| `crates/eqoa/src/touch.rs` | 79 | 2 | Restored map and tracker touch input. |
| `crates/eqoa/src/ui.rs` | 9 | 5 | Map UI state and input integration. |
| `crates/eqoa/src/update.rs` | 3 | 3 | Fork release routing and fork-owned notes; updater mechanics unchanged. |
| `crates/eqoa/src/updatewnd.rs` | 3 | 2 | Fork release page link and regression assertions. |
| `crates/eqoa/src/viewer.rs` | 200 | 11 | Restored map/quest dialogs and focus integration. |
| `crates/esf/src/mesh.rs` | 9 | 0 | Terrain mesh lookup for restored map. |
| `crates/net/src/session.rs` | 418 | 30 | Restored group/map/quest protocol state. |
| `crates/render/src/fans.rs` | 1 | 1 | Restored UI render behavior. |
| `crates/render/src/lib.rs` | 50 | 4 | Restored map rendering pipeline and exports. |
| `crates/render/src/ui.rs` | 97 | 8 | Map circle clipping and map font rendering. |
| `crates/render/src/ui.wgsl` | 16 | 7 | Circle clipping in UI shader. |
| `crates/vi/src/csprite.rs` | 85 | 11 | Animation bounds guards and negative-control regression tests. |
| `crates/vi/src/hsprite.rs` | 69 | 3 | Animation bounds guards and negative-control regression tests. |
| `crates/vi/src/ui.rs` | 3 | 2 | Restored player-facing UI behavior. |
| `release/README.txt` | 1 | 1 | Enhanced Windows bundle startup instructions. |
| `web/build.sh` | 1 | 1 | Include the fork reporting policy module in browser assets. |
| `web/main.js` | 48 | 0 | Restored browser input/map bindings. |
| `web/report.js` | 31 | 7 | Fork reporting policy transport integration. |

`CLAUDE.md` is a separate exact-content exception: upstream text, one blank line,
then `On this enhanced fork, FORK.md defines scope and overrides the Vanilla rules above.`
No other fork edits are accepted. `UPDATE-NOTES.md` and `LICENSE` must match
upstream. The checker also compares `license` and `license-file` values in both
package/workspace-package sections of every upstream Cargo manifest; dependency
or feature budgets cannot authorize a license change. Third-party license notices
remain intact in the fork-owned resource and documentation trees.

## Named extension seams

The manifest requires exactly one textual occurrence per named hook in its shared
parent file. This is a targeted guard, not a Rust parser; normal build/test review
still verifies execution and placement. Do not add duplicate hook names in comments.

| Shared file | Checked seams |
| --- | --- |
| `crates/eqoa/src/main.rs` | `mod fork_ext;`, `mod report_policy;` |
| `crates/eqoa/src/play.rs` | `self.fork_combat_event(...)` |
| `crates/eqoa/src/hud.rs` | `fork_hud::draw(...)` |
| `crates/eqoa/src/options.rs` | `fork_options::{CATEGORY, ITEMS, change, activate, value_text}`, once each |
| `crates/eqoa/src/settings.rs` | `fork_settings::{write, parse, description}`, once each |

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
fork notes and update-window release page exactly once each (15 named seams total).

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
