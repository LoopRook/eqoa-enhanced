**EQOA Enhanced upstream sync ? 2026-11-10**

**Batch 1: pinned 77-commit checkpoint**

Merged `82e64c86df07fcdac9cba39e4cc6c5020032953d` into `sync/2026-11-10` from develop `abd47db`.
Merge: `724e7fa`; follow-ups: `ef3c265`, tested source `b237316`.
Worktree: `<repo:eqoa-sync-2026-11-10>`.
No push, tag or merge into develop/enhanced/master.

Client/net, addon, WASM, tools, notes, optimized allocation, release and packaging
checks pass. **The workspace has two GPU failures, both reproduced exactly on
untouched pinned upstream; it is not claimed green.**

Hand resolutions:

| File | Kept from upstream | Kept from Enhanced / reason |
|---|---|---|
| `hud.rs` | DPS meter state/picks/draw, tracker room, pet-effects strip and tests. | Stats Box state/pick/draw, ToT, mana gauges, numeric labels, addon claims and local Report hook. Existing HUD features remain without redesign. |
| `map/chrome.rs` | GM travel legend, inserted after waypoints. | Rebound/wrapped keyboard hints and combined map chrome; the keyboard already supplies its quest hint, the pad retains upstream's hint. |
| `map/mini.rs` | Current quest-offer/step rules, rectangle-placement helper, upstream quest and far-waypoint tests verbatim. | Circular mask/border/hits/guidance/search/journal. Circle geometry moved to `fork_ext/map.rs`; rendered rim marks are clamped after upstream placement, preserving both coordinate contracts. |
| `options.rs` | Menu order including Macros; every Extended page, row, ID and order; touch notes; layout; upstream walk text/tests. | Existing hooks append Enhanced HUD and Mods & Addons after upstream's last page, Combat. No upstream row moves. |
| `play.rs` | Pet/number/DPS agreements, meter state/interactions, macro capture, GM travel, new HUD payloads and initialization order. | Stats Box, ToT, mana bars, numeric labels, map-search hold release, guide/report/addon hooks. Field-wise literal merge combines both initializers; redundant fork GM-travel method removed from `fork_ext/play.rs`. |
| `settings.rs` | New particles/colors/pet/DPS/touch fields, persistence, defaults and tests, including non-ASCII fixtures. | Existing fields and write/parse hooks; legacy keys and first-run 64/650 migration. |
| `viewer.rs` | Adjustable `touch.opts` and new macro input/capture. | Map search still owns touch-window input; existing map hold, report and addon hooks. |
| `net/session.rs` | Map-travel reset and following NPC-tint reset, new protocol handling/tests. | Power/quest/live-shard behavior. Removed one byte-identical automatically combined duplicate GM-travel test; upstream's test remains verbatim. |

Non-conflicted follow-up: upstream's new bindpage macro fixture needs F2 free;
Enhanced uses F2 for map search. One setup hook in `bindpage.rs` calls the owned
`fork_ext/bindpage.rs` helper, moving only that fixture's map-search key to F24.
All upstream assertions remain, and a separate test proves production stays F2.
Restoring upstream's header comments keeps this below the existing budget.
Fork-only fixtures gained `PetState.effects`, `Report.triggers` and the new
`touch::layout(size, &Opts)` argument.

Upstream `touch.rs`, `macropage.rs`, `numcolor.rs`, `dpsmeter.rs` are identical to
the pinned versions after line-ending normalization. No newly lost/weakened
upstream tests. An inherited exception remains: vanilla's
`the_servers_power_offer_goes_unanswered` was already absent on develop, whose
Mana Bars consumes those opcodes; this sync did not remove it. Existing fork
power-offer tests remain. Other inherited fork fixture/layout adjustments stay.

Rework before the final runs: duplicate net test and Play initializer/fixture
compile errors; a Windows default-decoding pass temporarily damaged settings
non-ASCII text, reconstructed from Git inputs with explicit UTF-8. Two intermediate
client runs each had 1,900 passed / 1 failed / 31 ignored (the F2 fixture).
The final runs below supersede these. Four rerere postimages exactly matching the
initial committed HUD/Play/settings/session resolutions were refreshed to the
final tested files; other records/preimages were untouched.

Native Extended retains upstream's 12 pages then two fork pages; browser retains
11 (no Textures) then two. Upstream rows include Particles 63, Pet Effects 58,
touch 59?62, Number Colors 64?69 and Combat 70?72. The separate Macros page is
unchanged. The owned fork walker follows Combat; upstream walkers stay upstream's.
`fork_ext/settings.rs` and enhanced.opt's one-time 64/650 migration are unchanged;
explicit settings remain explicit. Upstream 24/100 constants/default tests stay
upstream's. Legacy touch_size still reads into upstream's separate sizes.

Touch was verified **by tests**, not just reading: `fork_ext/touch_tests.rs:8`
passes all four D-pad directions through touch/libpad into quick chat, releases
and checks no map tap. `:94` verifies one map group-waypoint after 600 ms, no
repeat, no tap on lift, and the ordinary-tap negative control when holds are off.
Both passed in sync and workspace runs. Upstream layout/opacity/edge tests stay.
Physical-device and live browser touch were not tested.

**Batch 1 feature overlaps for round 3**

- Number Colors: `numcolor.rs:14` describes six floating combat-number categories;
  IDs are at `:33`. `play.rs:624` chooses upstream RGB
  with Sandstorm patch precedence; `play.rs:6504`
  reads the tagged color; `play.rs:6524` forwards
  it through the fork hook at `fork_ext/play.rs:998`.
  Addon hit colors therefore include upstream's selected color. Fork HUD numbers
  are fixed player HP/power labels (`fork_ext/hud.rs:208`),
  XP details (`:229`), and target percentage
  (`:239`). They do not recolor floating
  damage. Both displays/settings remain independent; combat color observation is
  the shared seam.
- Pet Effects: upstream builds the pet strip at `hud.rs:175`
  and draws it from `hud.rs:586`, offset `[10,48]`
  (`hud.rs:144`). It reads the server's pet state via
  `play.rs:9167`. The fork's player active-effects
  wrapper at `fork_ext/hud.rs:123`
  honors addon active_effects claims; **that claim does not hide the new native
  pet strip**. A future mod drawing pet buffs needs to coordinate this. Stats Box
  shows stat changes at `statbox.rs:47`
  (`[200,54,348,186]`), not pet icons. The fixed ToT is `[20,300]`
  (`hud.rs:284`); group bars begin `[20,93]`
  (`hud.rs:288`). At the base 640?448 frame, four group
  members put the pet at y=237 and its effect rows at 285/305, meeting/overlapping
  ToT y=300. Upstream reserves pet-effects height for its raid list, while the
  fork's fixed ToT does not reserve space. This is retained for the mod conversion,
  not redesigned here.
- DPS: upstream's server Fight/Overall meter draws at
  `hud.rs:503` with layout
  at `dpsmeter.rs:318` in the tracker's right column
  (`tracker.rs:26`, `[380,168,586,336]`). It
  reduces tracker room and sits below it. Stats Box/numeric labels remain left.
  The fork's separate Combat Parser action at
  `fork_ext/options.rs:119`
  and local combat/telemetry hook at `fork_ext/play.rs:998`
  remain. A separate parser and server meter can both run through different data
  paths; the meter does not replace addon combat events. There is no separate
  native DPS-meter addon claim yet, so future mods must coordinate placement.

**Batch 1 verification and footprint**

Every cargo command used `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared`,
an atomic mkdir lock at `%USERPROFILE%/Paseo/cargo-build.lock`, 30-second retries,
a bounded lock wait, per-command timeout (1,200?3,600 seconds) and scratch
EQOA_CONFIG. The sync script uses its own scratch target/sync-config, one thread.
CI-style Windows workspace/optimized/release use Rust 1.99.1, explicit
x86_64-pc-windows-msvc and `RUSTFLAGS=-C target-feature=+crt-static`.

| Gate | Result | Passed | Failed | Ignored | SKIP lines |
|---|---|---:|---:|---:|---:|
| Sync script --continue --tests-only (eqoa/net/integrations/doc tests) | PASS | 1,984 | 0 | 31 | 205 |
| CI tools: python -X utf8 -m unittest discover -s tools -p test_*.py | PASS | 97 | 0 | 0 | 0 |
| Required fork_ext::addons:: suite, one thread, --nocapture | PASS | 338 | 0 | 24 | 9 |
| cargo check --locked -p eqoa --target wasm32-unknown-unknown | PASS | ? | ? | ? | 0 |
| CI-style full workspace, --no-fail-fast, one thread, --nocapture | FAIL | 2,273 | 2 | 34 | 297 |
| this_builds_own_notes_read | PASS | 1 | 0 | 0 | 0 |
| CI exact optimized zero-addon allocation gate | PASS | 1 | 0 | 0 | 0 |
| Untouched pinned upstream AO/particles16 control | Same failures | 1 | 2 | 2 | 0 |
| Static-CRT enhanced-dev release build at clean b237316 | PASS | ? | ? | ? | 0 |
| CI packaging command + independent verify_release(channel=enhanced-dev) | PASS | 1 package | 0 | ? | 0 |

Per-invocation counts overlap; they are not unique test totals. Rust passes include
SKIP returns. Addon SKIPs: four symlink-privilege failures, two optional missing
Forge spike fixtures, three absent approved image goldens. Other suites lack
ISO/modules/oracles/captures. Opt-in tests remain ignored; no goldens invented.

The known `the_foot_of_a_wall_darkened_and_open_ground_not` was filtered exactly
as CI does. Executed failures:

- `render/tests/ao.rs:281`, `the_same_at_freeport_as_at_the_origin`: factor differs
  by 54 (56 at half radius).
- `render/tests/particles16.rs:101`,
  `particles_are_blended_as_the_gs_blends_them_into_a_16_bit_frame`: fifty puffs of
  a tenth, pixel 3 `[163,102,73]`, expected `[155,102,73]`.

Both reproduce exactly on an untouched git archive of `82e64c86df07fcdac9cba39e4cc6c5020032953d`, same RTX 3070 Ti
Vulkan, static CRT, shared target and one thread. AO source/test/shader are also
unchanged from 91f9e72. No renderer assertion, test or CI exclusion was altered.
The optimized gate reports zero allocations/bytes for menu frame, busy frame,
combat and chat. Packaging ran tools/build_player_release.py with the shared
x64 release executable, an external temporary output and --channel enhanced-dev,
then checked runtime members, PE/static CRT, source SHA, manifests and checksums.
No package was published.

Footprint: `python tools/check_fork_footprint.py --base 82e64c86df07fcdac9cba39e4cc6c5020032953d` PASS;
reviewed_base moved deliberately to that SHA. **No budget changed/increased.**
Before: develop abd47db vs reviewed 91f9e72, +6,149/-349 = **6,498** changed lines.
After: b237316 vs 82e64c8, +5,932/-310 = **6,242**. Reduction **256**. This measures
additions plus deletions in policy-shared paths (including new map files in full),
not fork-owned adapters, using each snapshot's reviewed base.

| Shared file | Before | After |
|---|---:|---:|
| `crates/eqoa/src/hud.rs` | +264/-43 | +264/-42 |
| `crates/eqoa/src/map/chrome.rs` | +327/-19 | +309/-18 |
| `crates/eqoa/src/map/mini.rs` | +313/-44 | +269/-14 |
| `crates/eqoa/src/options.rs` | +21/-13 | +21/-13 |
| `crates/eqoa/src/play.rs` | +381/-62 | +377/-62 |
| `crates/eqoa/src/settings.rs` | +52/-1 | +51/-1 |
| `crates/eqoa/src/viewer.rs` | +123/-13 | +107/-12 |
| `crates/eqoa/src/bindpage.rs` | +7/-7 | +5/-4 |
| `crates/net/src/session.rs` | +281/-27 | +218/-27 |

Full 42-path measurements and unchanged-budget assertion: footprint-metrics.json.
No budget-increase reasons are needed because none increased.
Pinned dry run (`git merge-tree --write-tree --name-only HEAD 82e64c86df07fcdac9cba39e4cc6c5020032953d`): exit 0,
zero conflicts. An earlier current-ref dry run at d9b1d59 (six later commits) also
had zero conflicts. origin/master then advanced to `5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` (18 later commits):
current-ref dry run now has four conflicts in hud/play/viewer/net session. These
are the next batch, not unresolved conflicts in the pinned merge. The owner
instructed that batch be merged on top only after this report checkpoint commits.

The upstream CLAUDE copy equals the pinned SHA's file. Adopted rule in fork
CLAUDE.md: native/browser server-world composition streams one zone through
Disc/Sink at a time and preserves parity with server Compose/CRC.

Evidence and temporary local packages:
`%LOCALAPPDATA%/Temp/eqoa-sync-2026-11-10-gates`.
Includes command/exit JSON, detailed logs, source audit, rerere record, footprint
metrics, untouched-upstream control and both dry runs. The worktree target folder
contains only sync logs/config, no Cargo cache; it will be archived and removed
at final cleanup, along with disposable control sources. All Cargo output stays
in the shared target. A release rebuild after the final report commit is recorded
separately in release-final.json/log; its package provenance in packaging-final.json.

Still not done at this checkpoint: the newer upstream batch (covered next),
independent lead review, owner live QA, physical touch/browser play, live server
feature/world checks and optional Forge/oracle/golden fixtures. The reproduced
upstream GPU failures remain unfixed. The four HUD extras stay as existing code
until the separately authorized mod-conversion round.


**Batch 2: the newly authorized 18-commit merge**

Checkpoint `bb04abe` was committed first, as requested. On top of it, merge
`38c930f` retains all 18 additional upstream commits through
`5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` (95 incoming commits across both batches). The batch adds raid
windows/leadership marks/loot controls, hat D-pad fixes and Firefox/WebGL2
context-loss/lighting guards. This completes the batch listed as pending in the
first checkpoint; no third batch was fetched or silently folded in.

The four manual conflicts were resolved by taking upstream's code as written
and attaching existing fork behavior:

| File | Upstream retained | Fork retained / reason |
|---|---|---|
| `hud.rs` | MemberState.marks, role marks on group bars, all new raid drawing/tests. | Existing MemberState.power remains alongside marks; mana/ToT/Stats Box/numeric hooks unchanged. Neutral marks=0 added only to existing fork fixtures. |
| `play.rs` | Loot-roll key capture runs first and returns when the roll owns the key; new raid state/menus/events and two field-wise merged initializers. | Existing chat-history hook runs after the upstream ownership check, so it sees unclaimed keys without stealing loot-roll controls. All HUD/API hooks stay. |
| `viewer.rs` | Browser exits immediately on graphics_lost, before further event-loop uploads. | Native report and Forge event hooks follow the guard. Their native cfg and browser guard are mutually exclusive; native pointer bookkeeping remains. |
| `net/session.rs` | Full the_raid_windows_are_agreed_read_and_sent test and raid offer/state/roll/master-loot/command handling. | Existing fork power-offer test and its comments remain immediately after the upstream raid test. Only the obsolete vanilla comment describing its already-absent unanswered-power test is omitted. No upstream test/assertion was removed. |

The script automatically merged two one-line Play literals field by field.
No resolution rerun or implementation fix was needed after this batch's gates.
New raid modules (`raidplay.rs`, `raidlist.rs`, `rollwnd.rs`, `masterloot.rs`,
`net/raid.rs`) are identical to this upstream SHA after line-ending normalization.
The incoming test-name audit loses no tests except the same inherited
unanswered-power exception documented in Batch 1.

**Raid overlap audit (current source at 38c930f; Batch 1 line references are at b237316)**

- **Group/target power:** the new batch adds no group-power or target-power
  display. Upstream's MemberState now adds marks, not power; retained fork power
  is at `hud.rs:121`. The fork's
  `member_gauges` at `hud.rs:308` and
  `target_gauges` at `hud.rs:325` still implement
  those Mana Bars. Upstream already had raid-roster power before this batch:
  `raidlist.rs:30` and the combined HP/power
  draw loop at `raidlist.rs:226`.
  The 82e64c8 version has that same power field/loop; the new change adds role
  marks and reserves name width for them. Upstream's raid power remains
  independent of the fork's Mana Bars toggle. Fork member gauges run only in
  the non-raid branch, so this merge adds no second power strip to raid rows.
- **Target of Target:** upstream has no new ToT readout/bar. Main-assist is a raid
  role (`raidplay.rs:27`), and Assist
  Main Assist is a server command, not a continuously drawn target's-target
  bar. The fork's ToT draw remains at
  `hud.rs:540`. Its existing raid/pet
  layout limit from Batch 1 remains; no replacement was designed.
- **Stats Box / HUD numbers:** raid State contains leader/helper/assist IDs and
  loot rules/flags (`crates/net/src/raid.rs:36`), not player stat summaries or
  HP/power/XP readout formatting. These fork features remain independent. Native
  role marks draw at `hud.rs:569`;
  they decorate names without replacing numeric labels or stat summaries.
- **Placement and input overlap, by code reading:** loot strips start at
  `[436,164]`, width 196, height 52, up to three (`rollwnd.rs:30` and `:37`).
  They cover the right-hand quest-tracker/DPS region as upstream describes,
  rather than the left-hand HUD readouts. A focused item description uses
  center `[310,240]`, width 250 and up to 14 text rows
  (`raidplay.rs:291`);
  that window can extend over the central Stats Box area depending on item
  height. It draws after the HUD via
  `play.rs:9001`. This is a potential
  visual overlap from code reading, not a claimed pixel/live play-test. The
  roll's R2/right/d-pad ownership at `raidplay.rs:207`
  takes precedence over the fork history observer, preserving upstream's flow.
  Normal corpse/master-looter windows remain ordinary game windows. No special
  new addon claim for the roll panel or item tooltip was added in this sync.

**Batch 2 gates (rerun once on 38c930f)**

Same shared target, mkdir lock, timeouts, scratch configs and CI static-CRT
settings as Batch 1. Detailed command/exit logs are named batch2-* beside the
first logs. The sync script's plain --continue performed the second merge
commit and the required serial tests.

| Gate | Result | Passed | Failed | Ignored | SKIP lines |
|---|---|---:|---:|---:|---:|
| Sync client/net/integrations/doc tests | PASS | 2,027 | 0 | 31 | 206 |
| CI Python tools tests | PASS | 97 | 0 | 0 | 0 |
| Required addon suite | PASS | 338 | 0 | 24 | 9 |
| WASM check | PASS | ? | ? | ? | 0 |
| CI-style full workspace, --no-fail-fast, known foot AO test filtered | FAIL | 2,319 | 2 | 34 | 298 |
| Notes test | PASS | 1 | 0 | 0 | 0 |
| Exact optimized allocation gate | PASS | 1 | 0 | 0 | 0 |
| Untouched 5d54c0c AO/particles16 control | Same two failures | 1 | 2 | 2 | 0 |

The executed GPU failures have exactly the same values as Batch 1, now freshly
reproduced on an untouched archive of **5d54c0c**: AO factor 54 and particle
pixel `[163,102,73]` versus `[155,102,73]`. Both tests and the known filtered foot
AO test remain unchanged. No additional failing test or CI exclusion was added.
The allocation gate again reports 0 allocations/bytes for menu, frame, event
and chat. Both fork touch integration tests also remain passing. SKIP categories
are unchanged (including four symlink privileges, optional pinned Forge data,
three absent approved goldens, and missing ISO/module/oracle/capture fixtures).
Counts are per invocation and include prerequisite-return passes as before.

**Batch 2 footprint, rules, final build receipt and remaining work**

reviewed_base is now **5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd**; the pinned-base checker passes.
No budgets changed: +5,932/-310 = **6,242** changed shared lines, unchanged from
the first checkpoint, versus 6,498 before this task. Full metrics:
batch2-footprint-metrics.json; source audit: batch2-source-audit.txt.

`git merge-tree --write-tree --name-only sync/2026-11-10 origin/master` against
that same SHA returns exit 0, **zero conflicts**, tree `6fbca9c922ca4f76f8a1bd7a4d9a1ed97a563a91`.
Both upstream targets are ancestors of this branch, in two separate merge commits.

The upstream CLAUDE copy equals 5d54c0c. Additional rules adopted in the fork's
CLAUDE.md: a lost WebGL2 context stops the event loop before uploads accumulate;
unsupported WebGL2 lighting stays off through the renderer guard; preserve the
combined mapped/axis/raw-hat D-pad paths and upstream controller diagnostics.

The final report-only commit is followed by the required static-CRT release
build, with EQOA_UPDATE_CHANNEL=enhanced-dev and EQOA_BUILD_COMMIT equal to its
full HEAD. Because that gate must postdate this file's commit, its authoritative
receipt is **release-final.json/log**, and the final package's independent
allowlist/PE/source-SHA/checksum verification is **packaging-final.json/log** in
`%LOCALAPPDATA%/Temp/eqoa-sync-2026-11-10-gates`. The final response states their actual outcome. The package
is local only; no publication/push/tag/protected-branch merge is authorized.

After that gate, sync logs/config are archived as target-archive, and the
worktree target directory plus both disposable upstream-control sources are
removed. No Cargo cache was created in the worktree. Final status/cleanup
receipt is retained as final-state.json beside the logs.

Remaining acceptance work: independent lead review; owner live QA; physical
hat/touch and Firefox/WebGL2 play; live raid/loot/new server features/world
composition; optional Forge/oracle/golden fixtures. The two upstream GPU-test
failures remain. The HUD extras still await the separately planned mod conversion.
No further implementation, merge into develop/enhanced, push or tag was done.

## Review fixes

The round-3 review's two defects were fixed on `sync/2026-11-10`:

- **Duplicate GM travel legend (HIGH).** `crates/eqoa/src/map/chrome.rs` had the
  `if c.travel { ... }` block twice after the merge. The second copy was deleted so
  the block matches upstream `5d54c0c` exactly. A new fork test,
  `fork_ext::map::tests::a_gms_travel_click_is_named_once`, draws the legend for a
  GM and counts `LEGEND_TRAVEL` exactly once (keyboard and pad), with the control
  that a non-GM legend has none. Negative control: with the duplicate restored the
  test failed (`left: 2, right: 1`); without it, it passes. Upstream's own test was
  left untouched.
- **Dropped quest-off assertions (MEDIUM).** `crates/net/src/session.rs` lost
  upstream's checks that agreeing off before asking answers nothing in
  `quest_marks_offer_answer_and_set` and `quest_progress_offer_answer_and_set`.
  Both `agree_quest_marks(false)` / `agree_quest_progress(false)` + `assert_eq!`
  lines were restored exactly as upstream wrote them.

Validation (shared target dir, scratch `EQOA_CONFIG`):

- `cargo test --locked -p net -- --test-threads=1`: **87 passed, 0 failed, 0 ignored**.
- `cargo test --locked -p eqoa --bin eqoa map:: -- --test-threads=1`: **160 passed, 0 failed, 0 ignored**.
- New fork test: **1 passed, 0 failed**.
- `python tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd`: **PASS**.
  Budgets were only tightened to the measured divergence: `map/chrome.rs` added
  327 → 304 (deleted 19 unchanged); `net/session.rs` added 281 → 218 and deleted
  27 → 23. No budget was raised.

