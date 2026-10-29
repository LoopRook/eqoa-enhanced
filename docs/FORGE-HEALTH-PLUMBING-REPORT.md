# Native health plumbing verification

Implemented the approved bounded native health adapter. This is source/fixture
acceptance, not a packaged preview, live server acceptance or full Forge support.

## Behavior and ownership

- Owned `addons/game_health.rs` tracks valid channel0x42 receipt per Play lifetime.
  Default state is unavailable; truncated input invalidates the previous receipt.
  The existing Reader must decode the full233-byte Stats prefix without error.
  WorldEntry (including same id/world) and disconnect clear validity. HP0 with
  positive maxHP is valid death; zero/nonpositive maxHP is unavailable. No fake
  maxHP, entity-percentage conversion or invented freshness timeout.
- Small native Play field/default and three event hooks feed that owned state.
  The existing child adapter `fork_ext/play.rs` extracts actual connected, quit,
  disconnected, HUD, loading, start-menu and health state. Native loading suppresses
  availability. No Session field or protocol/parser behavior was changed.
- Host Snapshot carries health. Manager normalizes health/in-game together before
  queued callbacks and entries, updates all retained instances, and installs the
  player/UI adapter into every fresh VM before entry execution. The standalone
  spike's separate fixture contract is preserved; it does not install health APIs.
- Owned `addons/visibility.rs` adds a managed intent layer while preserving the
  standalone health registry/installer and tests. Active pre-login initialization
  may record a desired hide label while returning false/no effective claim.
  Readiness promotes that bounded request without an extra Lua callback. Inactive
  callbacks cannot record intent. EnableHealthBar removes pending/effective intent.
  Limits remain32 distinct labels/VM and128 bytes/label, including pending requests.
- Normal disable clears pending/effective visibility after callbacks while retaining
  the VM and textures. Failure, successful replacement, removal and shutdown clear
  visibility independently of callback success; compile-failed reload preserves the
  old lease. Fresh staging is ineligible. Cleanup and OnEject errors cannot retain
  claims. Re-enable does not resurrect an explicitly released label or invoke Load.
- Runtime errors suspend effective claims and publication; recoverable reopening
  promotes only unreleased intent on the next real frame. Construction, failed
  initialization, pause, stop, Drop and manager teardown reset publication. The
  thread-local publication assumes the current single native app/render thread.
- Native post-resolve Lua publishes for the **next native HUD frame**. Owned HUD
  predicates suppress only the player's health gauge and its percent label; the
  frame, power, XP, target/group UI, maps and saved settings remain unchanged.
  A claim/readiness transition forces cached HUD quads to rebuild even without a
  game step. Shared changes are the approved Play/HudState/gauge/cache hooks and
  two viewer snapshot call sites; implementation remains fork-owned.
- Addon Manager shows unavailable-health guidance, including a specific wait for
  current-world stats when connected/HUD-ready without valid post-entry data.

## Test evidence

Meaningful RED preceded receipt/intent/manager integration: both receipt tests
failed on default-only state, both intent tests failed before installer support,
and pre-login manager initialization failed before per-instance installation.
The intent fixture initially used the standalone spike Host, which intentionally
rejects managed modules; it was corrected to use a real addon Host. The real PNG
fixture initially placed resources in the private folder; unchanged hearts uses
the documented shared UiForge.resources_path, so the fixture was corrected without
changing source or resolver behavior. The native cache fixture explicitly exits
Play's initial loading state before asserting entered-world suppression.

Actual controlled negative source changes removed the HUD suppression and cache
invalidation predicates: `forge_health_` produced **6 passed, 2 failed**, at the
intended gauge-visibility and non-game-frame cache-transition assertions. Both
production hooks were restored before GREEN checks. These tests exercise the actual
owned predicates used by native HUD generation/cache selection; they do not claim
a live atlas-backed native game HUD capture.

Commands use Cargo from the existing worktree target cache:

```powershell
cargo test --locked -p eqoa forge_game_health -- --nocapture --test-threads=1
cargo test --locked -p eqoa forge_visibility_ -- --nocapture --test-threads=1
cargo test --locked -p eqoa forge_manager_health_ -- --nocapture --test-threads=1
cargo test --locked -p eqoa forge_runtime_ -- --nocapture --test-threads=1
cargo test --locked -p eqoa hud:: -- --nocapture --test-threads=1
cargo test --locked -p eqoa play::fork_hooks:: -- --nocapture --test-threads=1
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
cargo build --locked -p eqoa
cargo check --locked -p eqoa --target wasm32-unknown-unknown
git diff --check
```

Focused GREEN: receipt2, intent2, manager health lifecycle1, runtime8, native HUD8,
and Play fork adapter3 tests passed. Coverage includes ordered pre/post-WorldEntry
receipts, same-id reentry, connect-none, short input after a valid receipt, death,
successive HP values, two independent owners, pending quotas, disabled callbacks,
fresh and compile-failed reload, failure/cleanup/eject, paused/reopened runtime,
publication lifetime resets and both cache transitions. Runtime GPU tests use real
Context/Manager/backend; overlay errors are explicitly injected boundary failures,
not a real device-loss/window-mismatch simulation.

Final serial addon suite: **80 passed, 0 failed**. Four Windows symlink-operation
cases explicitly skipped privileged operations with error1314 (reported as passing
tests, not ignored). There were **no GPU/source/PNG prerequisite skips**. Pinned
grid, unchanged hearts, manager cleanup/isolation, runtime recovery and real
post-upload allocator rollback all executed. Native development build passed
(exit0,14 warnings); WASM check passed (exit0,one warning). No warning suppression
or automatic fixes. `git diff --check` passed with line-ending notices only.
No exhaustive workspace/release build was run. Exact shared footprint review
remains owned by root; policy budgets were not changed.

## Unchanged pinned hearts and original PNG acceptance

`retro_health_hearts.lua` is copied byte-for-byte into a scratch addon; SHA256 is
asserted as `ac5c5b7ab8a245dfedfa7414d8938ef0608546c7618db17b44425c2467fafe37`.
The four original PNGs are copied to scratch shared resources without conversion:

| File | SHA256 |
| --- | --- |
| heart_accent.png | 948577e94751edb401961e6f2c6f7ca07be32eba7ae499906f1e9b39be9b4799 |
| heart_border.png | 6ea34105000957fad82cb75eb490483cd0e73a9da61961088766821c5f82a2fc |
| heart_fill.png | b3b26c6d71fe7207c28456a86c3e41e4b4dbc24ea29182f380cbb4c0464c12a0 |
| heart_highlight.png | bd93888cfc45628a82bc678ae0679dd5662197b92e30918a8373662edd0c06c3 |

Actual WGPU readback gives **1395 red fill pixels at100HP, 690 at50HP and0 at0HP**
(max100), proving real source/texture fill response rather than a substitute script.
Pre-login initialization executes once and its pending intent promotes when ready.
Disable preserves all four cached texture IDs; plain re-enable renders identical
full-health pixels with the same Lua state and handles. The native bar remains
visible because upstream OnDisable released the label and Enable adds no callback.
Explicit saved profile Load reasserts suppression through unchanged upstream Load.
Shutdown releases claims and expires all four handles. This uses synthetic,
explicitly labeled HP snapshots, not fabricated live server observations.

The final fixture writes `target/forge-hearts-{full,partial,death}.png`; pinned grid
capture is `target/forge-spike-grid.png`. These are ignored local evidence only.
Full/death captures were visually inspected. The256x192 fixture crops the right
edge of the fifth heart at the default ImGui window position; this is a bounded
pixel-response/lifecycle oracle, not native display/layout/DPI acceptance.

## Remaining gates

Actual live current-HP delivery, login/zoning/reconnect order, native HUD rendering,
input/DPI/resize and packaged player acceptance remain unverified. Conservatively,
stats received before WorldEntry are discarded at entry. A server sending no later
full0x42 could leave health addons waiting indefinitely; the manager explains that
state. No acceptance is claimed for server ordering or an expiry-based freshness
guarantee. Native and Forge health sampling paths may have different network timing
(native gauge uses entity p19, the addon uses real integer0x42 HP).

Root owns exact shared footprint review and concrete preview packaging. No policy,
dependencies, maps, Session, render crate, settings, account/server operations,
commits, pushes or release/package actions were performed in this task. Broader
ff_example/module compatibility remains separate.
