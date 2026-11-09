# The eqoa mod API: plan

Start here for the mod API work. Written 2026-11-08 by the Opus lead from the owner's decisions. Each
round below is one task for a fresh lead and fresh workers (`~/Paseo/Agent_Learning_Instructions.md`).
Record each round's result at the bottom of this file and keep it under about 150 lines.

## Goal (owner, 2026-11-08)

A good way to make mods for EQOA Enhanced. Mod authors can do most or all of what FrontiersForge
allows, built correctly on the native client. Cheap to sync with upstream.

- **The API is `eqoa`**: the Lua global `eqoa`, the modules `eqoa.<area>`, and `"api": "eqoa/1"` in
  mod.json, which discovery already requires. Upstream has no Lua or mod system, so the name does not
  conflict.
- **`eqoa/1` is a stability promise, made at round 5:** nothing under `eqoa/1` is renamed, removed or
  changed incompatibly. A breaking change goes to `eqoa/2` and `eqoa/1` keeps working.
- **Forge becomes a compatibility layer.** `frontiers_forge.*`/UiForge stay so that the bundled
  Wereoxxs UI, ARPG UI and Modern UI (and other Forge mods) keep running unchanged. They are rebuilt
  over the same native core as `eqoa`, so a fix reaches both. Running the same mods on PCSX2 is not a
  goal. Forge's functions are not copied name for name; Forge's *capabilities* are the target.
- **Built correctly:** every value comes from the client's real game state. The client is a port of
  the game, so the data Forge read from PS2 memory lives in our own structures. Changes arrive as
  events (chat, combat, cooldowns, effects, inventory changes), not by polling. An unsupported call
  shows a visible diagnostic in the Addon Manager and `mods/addons_error.log`, never a fake success.
- **Out of scope:** raw memory, FFI, EEmem, code caves, Forge's `mini_map` rasterizer. Where a mod
  needs what those gave Forge, add a proper feature instead (for example zone map data).
- **Actions (owner, 2026-11-08):** mods may perform game actions, including automation: auto loot,
  targeting, casting, movement, up to full botting. Upstream is fine with this too. The limits are
  only there to protect the player who installs a mod: files only in the mod's own folder, no
  network, no launching programs.
- **Hooks may go upstream; mods never will.** Design each hook in an upstream file as a neutral,
  general seam (game state and events out, actions in) that upstream could accept on its own merits,
  with nothing Lua- or mod-specific in it. The owner decides when to offer one upstream.
- **Not doing:** outreach to Forge's author, and the PCSX2 profile converter.

## What exists (0.4.14, develop e48f0a6)

- Runtime: `crates/eqoa/src/fork_ext/addons/` (about 38k lines, all fork-owned). Upstream's files
  carry 4 hook lines (play.rs 1, viewer.rs 3). The owner approved more hooks as needed (2026-11-08):
  the spec's §10 lists them (about 10 call lines, plus read-only dialog accessors in round 4). Each
  stays a neutral seam, measured by the footprint check; nothing beyond §10 without lead review.
- Native core: `host::Snapshot`, filled each frame by `fork_ext/play.rs` (`forge_snapshot`, :127),
  plus event queues for chat, combat and cooldowns. The Forge modules are adapters over this core.
- Forge modules registered: ability, ability_bar, ability_list, chat, combat, effects, entity,
  entity_list, group, icon, input, player, quest_log, ui, util, window, plus UiForge, ImGui and
  DrawList. Not built: bank, camera, gems, inventory, item, loot, merchant, quest (objects). Missing
  map pieces: tooltip and mouse APIs, `AddImageQuad`, JPEG, `GetWorldId`, file listing.
- Existing `eqoa` extensions: weapon-set bar (`weapon_sets.rs`), chat history scroll (`chat_history.rs`).
- Known placeholders (2026-10-31 audit; re-check): Class Mastery fields are zero, `ui` class and
  disposition icon helpers are stubs, the controller snapshot is default, hook-lifecycle calls report
  success, and `combat.ReadForeignCave` returns nil quietly.
- Older drafts to mine, then retire: branch `docs/mod-author-api` (1be52fb: API-CONTRACT,
  GETTING-STARTED, SUPPORT-TABLE, ROADMAP, at 0.2.1). Reference: `run/forge-compat-reference/`
  (pinned Forge 4026d5c; 25 modules under `scripts/modules/frontiers_forge/`).

## Rules for every worker brief

Paste these into each brief (they come from CLAUDE.md):
- Work only in your own Paseo worktree and branch, branched from `develop`. Commit there; never push,
  merge or tag. Managed worktrees need `git -c safe.directory=*`.
- New code goes in fork-owned files (`fork_ext/addons/`, new `fork_ext/...` files). Shared upstream
  files get hook calls only. `python tools/check_fork_footprint.py --base <reviewed_base from
  docs/fork-footprint.json>` must PASS; report any budget change with its reason.
- Game-logic floats go through `ee::*`; game counters step on `frames.rs`'s 30 Hz clock.
- Use your own `CARGO_TARGET_DIR`. Take the shared lock `C:/Users/hocka/Paseo/cargo-build.lock` (mkdir)
  for every cargo build or test, with a `timeout`, and remove it when done. One build at a time.
- Tests: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` with a scratch
  `EQOA_CONFIG`. Write behaviour tests with a negative control (show the test fails without the
  change). Count SKIP lines. Also `cargo check --locked -p eqoa --target wasm32-unknown-unknown`,
  then a release build after the final commit.
- Bundled Wereoxxs, ARPG and Modern UI must still load and pass their existing tests.
- Write the report to `docs/plans/eqoa-api/round-N-report.md`: what changed, tests (pass/fail/skip
  counts), footprint, anything not done. Reviewers write to `docs/plans/eqoa-api/round-N-review.md`:
  verdict first, then findings by severity with file:line and a failure scenario.

## Rounds

The owner starts each round in a new session with the model in the Lead column. A Sonnet lead is a
top-level Sonnet session, not a helper under Opus. Each round runs on its own branch `feat/eqoa-api-rN`. It merges into `develop` after tests,
footprint and review; the develop push publishes Nightly, which is the owner's test build. Reviewer:
a different model family from the builder (the roster's "who reviews whom").

| Round | Work | Lead | Builder | Owner's part |
|---|---|---|---|---|
| 1 Design | The `eqoa/1` spec | Opus | Scout (free) for data sources, then Sol high | Approve the one-page summary |
| 2 Core | `eqoa` over the existing core, Forge rewired onto it | Sonnet | Sol high | Live test: three bundled UIs unchanged, plus the example mod |
| 3 Items | inventory, item, bank, loot, merchant, gems | Sonnet | Sol medium (DeepSeek Pro trial) | Live test of a small bag/loot example mod |
| 4 The rest | quests, camera, windows/dialogs, map data, CM, icons, player-initiated actions | Sonnet | DeepSeek Go (small), Sol (coupled) | Live test of the examples |
| 5 Finish | Audit, docs, support table, freeze `eqoa/1` | Opus | Terra/DeepSeek Go for docs | Approve the release |

### Round 1: design (no game code)

1. **Scout** (read-only): for each of the 25 Forge modules plus the UiForge/ImGui surface, find where
   the native client holds that data or performs that action (file:line in `crates/vi`,
   `crates/eqoa`), and whether the client receives it at all (for example the bank only after
   opening it). Output: `docs/plans/eqoa-api/data-sources.md`.
2. **Sol high** writes `docs/modding/EQOA-API-SPEC.md`. It contains:
   - every module, function, field and event, with argument and return types, units, index base,
     the nil/error cases, and the native data source;
   - the event model, the capability query (`eqoa.capabilities`), the diagnostic rules, versioning,
     and the actions API (every game action a player can take, usable by mods without player input;
     files only in the mod's own folder, no network or processes);
   - each hook needed in upstream files, written as a neutral seam (see Goal);
   - a mapping table from each Forge module to its `eqoa` source, with Forge-only quirks noted;
   - a list of what the client cannot provide, and why.

   Reuse what still holds from `docs/mod-author-api`. Also write a one-page summary for the owner in
   player terms.
3. **Review** by MiniMax, then Opus triage. Opus shows the owner the summary and asks for approval.
   Nothing is built before approval.

### Round 2: core

Build the `eqoa` modules that exist today in Forge form: player, target/entity, entity list, group,
abilities and cooldowns, effects, chat, combat, input, HUD hides, drawing (ImGui stays the drawing
API), textures, sound, fonts, settings save/load, logging, capabilities. Then point the Forge adapters
at the same core, so there is one source of truth and Forge-specific shapes stay in the Forge layer.
Fill the known placeholders that fall in this area (effects from real data, controller snapshot)
or report them through capabilities. Add the example mod `bundled-mods/eqoa_example_hearts` (hearts
written against `eqoa`) and `docs/modding/GETTING-STARTED.md`. The risk is breaking the bundled UIs:
the existing addon tests are the regression gate, and the owner's live test decides.

### Round 3: items

Inventory and item objects (with tabs, selection and change events), bank, loot window, merchant,
and gem helpers. Each one has empty, closed and zoning cases tested. Add a matching Forge adapter
where a Forge module exists. Add an example mod: a bag viewer with a loot log.

### Round 4: the rest

Quest log and quest objects, camera, windows/dialogs (busy state), map data for map mods (zone, world
id, map images, waypoint to the native map), Class Mastery values, class and disposition icons, the
tooltip and mouse ImGui APIs and `AddImageQuad`, and the actions API from the spec (cast, use an
item, target, loot, move, follow), callable from a click or from the mod's own logic. Split it into independent worker tasks where the files
do not overlap.

### Round 5: finish

- Audit every Forge and `eqoa` function against the spec. Each is real, or gives a visible
  "unsupported" diagnostic; nothing reports a fake success.
- Write `docs/modding/SUPPORT-TABLE.md` from that audit.
- Finish the author docs: spec, getting started, examples, versioning promise.
- Retire the `docs/mod-author-api` branch (with the owner's OK) and update FORGE-COMPATIBILITY-HANDOFF
  to point here.
- The owner approves the release that freezes `eqoa/1`.

### Optional: offer the hooks upstream (owner's call)

After round 2, when the core hooks have settled: a worker makes a branch from `origin/master` with
only the hooks (no Lua, no mods) and their tests. It writes a short PR description that gives each
hook's use inside the vanilla client, then runs the vanilla test suite. The owner decides whether to
open the PR (it goes out under their name). Each hook upstream accepts drops out of our footprint
and out of future sync conflicts.

## Results

(One short entry per round: date, branch and merge commit, verdict, owner test result.)

- **Round 1 (design), 2026-11-08.** Branch `feat/eqoa-api-r1`. Scouts (DeepSeek Go; the free tier hit
  its limit at once) wrote `docs/plans/eqoa-api/data-sources.md`; Sol high wrote
  `docs/modding/EQOA-API-SPEC.md`, the owner summary and the report. MiniMax review: APPROVE WITH
  CHANGES (4 HIGH, 10 MEDIUM, 11 LOW; lead verified H1-H3); Sol fixed all 25 (71b80a6). Owner
  approved the design and the §10 hooks. Owner decision on files: every mod may read the shared,
  bundled `mods/resources` tree read-only; all writes go to the mod's own private area (overlay with
  tombstones, spec §9); other mods' files stay unreachable. Next: round 2 under a Sonnet lead, working
  from spec §§1-5, 9-12 and the §9 bundled-UI regression cases.
- **Round 2 (core), 2026-11-08.** Branch `feat/eqoa-api-r2`, merged into `develop` 3794d3c. Sol high built
  (stopped once at the Codex limit, resumed); play.rs budget raised 755 to 759 (owner-approved hooks).
  MiniMax review: APPROVE WITH CHANGES (3 HIGH, 6 MEDIUM, 8 LOW); lead verified H1/H2; Sol fixed H1-H3,
  M1-M4, M6, L2-L3, and showed M5, L1, L4, L5 not defects. Spec §9 now states the 131,072 ImGui call
  budget. Gates: addon suite, installer, pinned Forge, GPU opt-ins, wasm, footprint, release build.
  Not verified: four symlink branches, oracle tests, live game. Owner live test pending: the three
  bundled UIs unchanged plus `eqoa_example_hearts`. Next: round 3 (items), Sonnet lead.
