# The eqoa mod API: plan

Start here for the mod API work. Written 2026-11-08 by the Opus lead from the owner's decisions. Each
round below is one task for a fresh lead and fresh workers (`~/Paseo/Agent_Learning_Instructions.md`).
Record each round's result at the bottom of this file and keep it under about 350 lines.
Rounds from 2.5 on were reordered on 2026-11-09 (owner-approved; see "Goal update" below).

## Goal (owner, 2026-11-08)

A good way to make mods for EQOA Enhanced. Mod authors can do most or all of what FrontiersForge
allows, built correctly on the native client. Cheap to sync with upstream.

- **The API is `eqoa`**: the Lua global `eqoa`, the modules `eqoa.<area>`, and `"api": "eqoa/1"` in
  mod.json, which discovery already requires. Upstream has no Lua or mod system, so the name does not
  conflict.
- **`eqoa/1` is a stability promise, made at the finish round (round 7; it was round 5 before the
  2026-11-09 reorder):** nothing under `eqoa/1` is renamed, removed or
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

## Goal update and owner decisions (2026-11-09)

Source: `docs/plans/2026-11-09-direction-review-fable.md` (the evidence, the table of every fork
change in upstream's files, and the three pull-request descriptions). The owner agreed with the
report and approved all ten recommendations on 2026-11-09.

The fork should become upstream plus mods. Neutral hooks are offered upstream; the fork's own
features (Stats Box, Target of Target, Mana Bars, HUD numbers, map search, journal, round minimap,
theme) move out of upstream's files into mods; mods never go upstream. A round is judged by two
numbers as well as its tests: lines of fork code left in upstream's files, and conflicts in a dry-run
merge of origin/master (`git merge-tree --write-tree --name-only develop origin/master`).

Owner decisions, approved 2026-11-09:

1. **Working goal.** "Upstream plus mods" comes ahead of FrontiersForge parity. Forge compatibility
   stays; it no longer sets the order of the rounds.
2. **HUD extras become mods.** Stats Box, Target of Target, Mana Bars and HUD numbers become bundled
   mods that are on by default (round 3).
3. **Round minimap and the fork's map look are mods** (round 4), and nothing is pushed upstream for
   them. The owner may mention them to Winfro, but plan on the assumption that vanilla won't take
   them (owner's correction, 2026-11-09, to the review's "ask first" recommendation).
4. **Fix pull requests go out now**, the animation crash fix first. The owner opens each one under
   their own name.
5. **First-run defaults (NPC display 64, distance 650) are kept**, applied from the fork's own
   settings at first run. Upstream's constants and tests stay untouched.
6. **Live-server client check.** The owner asks Winfro directly whether vanilla wants it. It does
   not go into a pull request unasked; until he answers it stays fork-only.
7. **Take upstream's version where the two are now the same:** group waypoint, quest toggles, the
   map suggestion form (upstream's edit-box flow), and upstream's new touch controls.
8. **Branch build before Nightly.** A round that changes the per-frame or per-packet path is played
   by the owner from a branch build before it merges into `develop`.
9. **The fork's own mods get their own repository** once round 3 has produced the first four. Until
   then they live in `bundled-mods/`.
10. **Full automation (botting) stays in the API, but last** (round 6). Small player-style actions
    come early, in round 3.

Owner update, 2026-11-10: upstream (Winfro) will look into a mod API, and group mana bars are to become
an upstream default later. The final step of this plan (round 8, after round 7) is a pull request that
offers upstream the mod API itself (runtime, hooks, `eqoa` API), something upstream can take and
implement. Individual mods stay out of upstream unless the owner decides otherwise. The Mana Bars mod is
built in round 3 as planned; when upstream ships its own group bars, the next sync retires the mod's
group part (target and ToT power stay in the mod unless upstream has them too).

Clarified the same day: vanilla has the Assist Target key (F) but not the Target of Target bar.
Upstream's notes of 2026-11-09 say the Stats Box, other players' mana bars and Target of Target
"are not coming back". The bar is therefore a mod, and it uses upstream's existing assist action.

Old and new round numbers: the spec (`docs/modding/EQOA-API-SPEC.md`), `GETTING-STARTED.md` and the
running client's capability `round` values still use the numbering of 2026-11-08. Old round 3
(items) is now round 5. Old round 4 (the rest) is split across rounds 3, 4 and 6. Old round 5
(finish) is now round 7. Those documents are renumbered at round 7, not before.

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
- Use the shared `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared` (never a per-task folder; see CLAUDE.md). Take the shared lock `%USERPROFILE%/Paseo/cargo-build.lock` (mkdir)
  for every cargo build or test, with a `timeout`, and remove it when done. One build at a time.
- Tests: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` with a scratch
  `EQOA_CONFIG`. Write behaviour tests with a negative control (show the test fails without the
  change). Count SKIP lines. Also `cargo check --locked -p eqoa --target wasm32-unknown-unknown`,
  then a release build after the final commit.
- Bundled Wereoxxs, ARPG and Modern UI must still load and pass their existing tests.
- Run gates with `python tools/run_gates.py` (`docs/DEV-TOOLS.md`); it takes the shared lock, waits for it
  and clears a lock whose holder died. Run long gate runs in the background with a long timeout and wait
  for them **in the same turn**: nobody resumes an agent that ends its turn while waiting.
- A negative control is executed, not described: break the code, run the test, record the command and
  the failing line, restore. Checkers rerun named tests themselves.
- Name the exact path of any reference that lives outside the worktree (the pinned Forge reference is
  `<main checkout>/run/forge-compat-reference/`); a builder that cannot find it must stop and say so.
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

Rounds 1 and 2 are done. The rounds from 2.5 on are in the second table, after round 2.

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

### Gates for every round from 2.5 on

1. **No-mods speed gate.** The optimized zero-addon tests (frame, menu frame, entity packet, chat
   line) allocate nothing and build no full view. This runs in CI on every develop push.
2. **Game-shaped tests.** Every new API value has a test that starts from Play/protocol-shaped data,
   not a hand-built mod snapshot. Each bundled UI has one picture-comparison test.
3. **Owner check before develop** for any round that changes the per-frame or per-packet path: the
   owner plays a branch build first. Develop publishes Nightly to players (decision 8).
4. **Footprint goes down or stays level.** A round that raises a shared budget names what it removes
   later, or it is the upstream-seam round.
5. **Never reformat or reorder upstream's lines.**

| Round | Work | Ends with |
|---|---|---|
| 2.5 Harden and tidy | Gates 1-2 built; options pages back to upstream's layout; footprint records corrected; fix PRs prepared for upstream | Owner live test of round 2 passes; dry-run merge conflicts counted |
| 3 HUD extras as mods | Small API additions; Stats Box, Target of Target, Mana Bars, HUD numbers as bundled mods; inline code removed | The four mods on by default; nothing lost for players |
| 3u Upstream seams | Folded into round 8 (owner, 2026-11-10): no separate seam PRs | — |
| 4 Map API and map mods | Map data API; journal mod, then search and guidance, then round minimap and skin; fork map code removed from upstream's map files | Upstream's map files unchanged or behind one seam |
| 5 Items | inventory, item, bank, loot, merchant, gems | A bag viewer with a loot log |
| 6 Actions and the rest | Full automation, quests, camera, dialogs | Owner approves the new seams first |
| 7 Finish | Audit, generated reference, support table, freeze eqoa/1 | Owner approves the release |
| 8 Offer the mod API upstream | A branch from origin/master with the hooks (the old round 3u seams), runtime and `eqoa` API (no fork mods), upstream's style and tests | The owner sends the PR |

Leads and builders are chosen per round from the roster (`~/Paseo/Agent_Learning_Instructions.md`);
the reviewer is a different model family from the builder.

### Round 2.5: harden and tidy (no new API)

1. Make the zero-addon allocation tests a required CI gate. Add a Play-shaped fixture helper and move
   the three bundled-UI fixtures onto it (round 2 review item L6). Add one picture comparison per
   bundled UI.
2. Options: restore upstream's Extended pages exactly (UI, Map, Textures and the rest: rows, order,
   ids 41-45). Fork rows live only on fork pages placed after upstream's last page. Keep every saved
   key. Upstream's walk tests return to upstream's text; fork pages get their own walk test in
   fork_ext/options.rs. Target: options.rs differs from upstream by hook lines only.
3. Footprint records: list the map family (map/**, tracker.rs, groupmark.rs, mapsuggest.rs,
   render/map.rs, render/map.wgsl) as shared with measured budgets; remove budgets for files that
   no longer differ; tighten every budget to its measured value.
4. Restore upstream's order and formatting where the fork only moved things (the Pick list in hud.rs,
   mini_view in play.rs, the reflowed lines in binds.rs).
5. Take upstream's version where the two are now the same (decision 7): group waypoint, quest
   toggles, the map suggestion form, and upstream's new touch controls. Re-test the D-pad in quick
   chat and the map's long press on upstream's touch code; report anything still missing.
6. First-run defaults (decision 5): apply NPC display 64 and distance 650 from the fork's own
   settings at first run. Restore upstream's default constants and the upstream tests that name them.
7. Prepare the fix PRs from origin/master, one branch each: refused login shows
   its reason; no phantom controller; texpack test path on Windows; .gitattributes LF for shaders.
   Write the note about the two AO tests on NVIDIA. The owner sends them (decision 4).
8. Report the before and after of both numbers: fork lines in upstream files, dry-run conflicts.

### Round 3: the HUD extras become mods

API additions (small; each tested from game-shaped data):
- `eqoa.player.stats()`: each stat's base and current value and the effect change on it; armour and
  resists as the Summary window shows them; Class Mastery bonuses when received.
- `power` on group members and entities when the server sends it (nil otherwise); `target_id` on
  entities.
- `eqoa.actions.assist()` and `eqoa.windows.open("summary")`: player-style actions only, through the
  same path as the key or click (assist is upstream's own Assist Target action).
- `eqoa.input.bind{id, label, default_key}`: a key the mod owns, shown on the Controls page.

Mods (bundled, on by default, each in its own folder, no client build needed to change them):
`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`, `eqoa_hud_numbers`.

Then remove from upstream's files: the Stats Box, Target of Target and Mana Bars drawing, their
HudState and Play fields, their option rows, settings and key rows. Saved settings migrate into each
mod's settings once. The network reader for others' power stays (offered upstream separately).
Owner approves each mod's five-line description first (UI rule). Acceptance: the owner sees the same
four features; hud.rs, play.rs, settings.rs and binds.rs lose the matching lines. After this round
the four mods move to their own repository (decision 9).

### Round 3u: offer the seams upstream

Folded into round 8 on 2026-11-10 (owner: one combined offer). The text below now describes round 8's hook part.


One branch from origin/master per PR, no Lua, no mod names, each with a use inside vanilla, tests
with a negative control, and the vanilla test suite passing:
1. Option rows declared in one table; ids 1000+ reserved for other builds.
2. Each HUD part can be switched off by itself (one question at the draw and at the click).
3. Game-event observer and a layer over the finished frame (chat log and creature watch moved onto
   it as first users).
Descriptions the owner can send are in the direction review, section 3. The owner opens each PR
under their own name. When one is accepted, the next sync deletes the matching fork lines and
lowers the budget. One question goes to Winfro directly, not as a PR: the live-server client check
(decision 6). The round minimap and map look are not offered; they are mods (decision 3).

### Round 4: map API and map mods

Upstream's map is the base. Add to the API:
- `eqoa.map.markers(world_id)`: camps, coaches, towns, quest marks, community pins, personal and
  group waypoints.
- `eqoa.map.view{center, zoom, size}`: the client renders the current map view to a texture; the mod
  places it, round or square. No per-frame map drawing in Lua.
- `eqoa.hud.hide("minimap")`, and hide switches for parts of the map's frame.
- `eqoa.map.set_guide(point or entity)`: a guide the mod owns that can follow a moving creature.
- Packaged read-only data above 1 MiB (the creature catalogue), read through the client.
Mods, in this order, each with the owner's five-line approval: the creature journal; search and
guidance (F2/F4); the round minimap with hover cards; the map skin (only if a theme seam exists).
The round minimap is an alternative mod: it hides upstream's minimap and draws its own (decision 3).
Then take upstream's map files unchanged. Anything that can't move behind the API is
listed for the owner with its sync cost. Measure frame time with the map mods on before deciding
(gate 1 covers them off).

### Round 5: items

Inventory and item objects (with tabs, selection and change events), bank, loot window, merchant,
and gem helpers. Each one has empty, closed and zoning cases tested. Add a matching Forge adapter
where a Forge module exists. Add an example mod: a bag viewer with a loot log. Start only when a
bundled mod needs it.

### Round 6: actions and the rest

**Owner decision 2026-11-15:** round 6 builds everything below except autonomous automation (a mod acting
on its own logic: auto-loot, auto-sell, bots). Click-driven actions (a mod button doing what the game's
button does, with the same checks), quests, camera, dialogs and the remaining Forge pieces go in and into
the round 8 PR (easier maintenance, a base for other mod authors). Autonomous automation comes after the
PR and may stay in Enhanced only.

**Owner decisions 2026-11-16 (round 6 start):** a mod action runs only inside the handling of the
player's own gesture (its mod key, a press in its claimed pointer area, or its ImGui button clicked),
one game action per gesture; other calls get a visible `needs_player_action` diagnostic. Batches ("sell
all junk"): see Part C below. Continuous controls (`move`, `look`, held `buttons`) also come after the
PR; autorun/follow/stop stay (one-press toggles). Two parts, each tested by the owner: **A** reads
(quests, mastery, camera, windows and dialogs, class/disposition icons, remaining Forge reads); **B**
click-driven actions and one example mod (5-line spec approved first). Part A is a trial of Gemini 4
Hydrogen as builder, with a checker that reruns its tests.
Part B's example mod (owner-approved 2026-11-16): `eqoa_bag_viewer` gains click actions (item menu with
Use, Equip/Unequip, Quick slot; Sell 1/stack at a merchant; Deposit/Withdraw at the bank; Take/Take all
at a corpse; refused choices greyed with the game's reason; result line under the grid; still off by
default). `actions.destroy` stays unsupported until after the PR.
**Owner 2026-11-16 (later):** click-started batches come before the PR after all, as **Part C** after Part B:
one click may run a short queue (one action at a time, each waiting for the server, stopping on error,
zoning or window close); bag viewer gains Mark as junk, Sell junk, Take all but junk. Event-triggered
automation (auto-sell, auto-loot) and continuous controls stay after the PR. Whether batches go into the
upstream PR or stay Enhanced-only is decided at round 8.
Part C spec approved by the owner 2026-11-16: (1) item menu Mark as junk/Unmark, junk icon, mark per item
type; (2) merchant: "Sell junk (N items, X tunar)", one click sells one at a time with progress; (3) corpse:
"Take all but junk"; (4) a batch stops on any refusal and shows what got done, Stop button; (5) nothing
without the click; optional "also sell grey items" off by default. Owner wants to test A+B+C together.
**Owner 2026-11-16:** the lead decides round 6's upstream seams (owner: "can you act on that"). Rule: only
small read-only accessors or one-line hook calls that expose data or call paths upstream already has; no
behaviour change; the cross-family reviewer checks each line; budgets rise by the measured lines with the
reason recorded; each approval is listed in `round-6-seams.md` for the owner to read.
**Owner 2026-11-17 (after the round 6 test): the one-gesture rule is dropped.** Enabling a mod is the
player's permission: a mod may act whenever it wants, including on its own (auto-loot near a corpse). Mods &
Addons labels every mod that can act on its own, so the player sees it before enabling. The game's own limits
stay (the server decides range and session). Next: the API gains "loot the nearest corpse" and "talk to the
nearest merchant/banker" (ported from the game's own requests), so the bag viewer loots, sells and banks
without the native windows; an auto-loot option in the mod. Selling or banking from anywhere is a bonus if
the server accepts the game's own requests at range (no position spoofing). Supersedes the 2026-11-15/16
lines above on autonomous automation and `needs_player_action`; the 5-line spec goes to the owner first.
Spec APPROVED by the owner 2026-11-17 (the mechanics are what is tested; the mods may later be split or
revamped): (1) Mods & Addons labels mods that can act on their own; enabling is the permission; the click-only
rule is gone for every mod; (2) Corpse tab "Loot" near a corpse loots the nearest corpse (all, or all but junk)
without the game's corpse window; (3) Sell junk near a merchant and Deposit/Withdraw near a banker work without
their windows; from farther away only if the server accepts the game's own requests (no position faking);
(4) mod option "Auto-loot" (off by default): walking up to a corpse you killed takes all but junk; (5) progress
line, stop on refusal, Stop button and no destroy stay. Order: owner-test fixes and retest close round 6, merge
and push on go-ahead, then this as round 6.5 (small upstream hook calls approved by the lead under delegation).

Quest log and quest objects, camera, windows/dialogs (busy state), Class Mastery values not already
done, class and disposition icons, the remaining ImGui pieces, and the full actions API from the
spec (cast, use an item, target, loot, move, follow), callable from a click or from the mod's own
logic. The map and the small actions are already done in rounds 3 and 4. Full automation needs new
seams in controls, dialogs, bank and merchant code: the lead shows the owner the exact lines before
building (decision 10).

### Round 7: finish

- Audit every Forge and `eqoa` function against the spec. Each is real, or gives a visible
  "unsupported" diagnostic; nothing reports a fake success.
- Generate the reference and `docs/modding/SUPPORT-TABLE.md` from the running client
  (`eqoa.capabilities`) instead of writing them by hand. Renumber the spec's and the capabilities'
  round labels to this plan.
- Finish the author docs: spec, getting started, examples, versioning promise.
- Retire the `docs/mod-author-api` branch (with the owner's OK) and update FORGE-COMPATIBILITY-HANDOFF
  to point here.
- Freeze `eqoa/1` only after the round 4 mods have shipped on it. The owner approves the release.

### Tools for mod authors (can run beside any round; no API change)

`eqoa mod-test <folder>` (run a mod against a saved snapshot: errors and a screenshot, no login);
`eqoa mod-api` (print the reference from the capability list); a starter template; reload on save;
docs/modding/AGENT-GUIDE.md (one page for agents building mods from an owner brief).

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
- **Direction review, 2026-11-09.** Fable, read-only: `docs/plans/2026-11-09-direction-review-fable.md`
  (develop 23d6469). The owner approved all ten recommendations the same day; this plan was
  reordered to match (rounds 2.5 to 7 above). No code changed. Still pending from round 2: the
  owner's live test. Next: round 2.5, when the owner starts it.
- **Round 2 live test and fixes, 2026-11-09.** The owner's first Nightly ran under 1 fps in the world
  (a full native view rebuilt every frame and packet, zero addons enabled): Sol replaced it with typed
  deltas, hook cost 14.7 ms to 5 microseconds per frame, merged ddd1c48 and fixed after MiniMax review
  (5346797). Further live findings fixed: Wereoxxs Enter (focus cleanup erased the key claim), Avoids
  Modern UI icon stacking (signed icon IDs dropped), a missing optional file shown as an error, an
  unclaimed Tab reaching ImGui (all on develop, 294b65f). Nightly release notes and version label
  fixed (`## Unreleased` section, `0.4.14+nightly (commit)`). Ability icons dark/red was only
  out of range (no defect). **Owner: round 2 live test passed** (2026-11-09). Logged, not done:
  server-added extra ability bars in the bundled UIs; native capture's position fallback; duplicate
  CI jobs per push; the Stable program name (final task before the Stable release).
- **Upstream sync 2026-11-09 (develop 13ce6ad, CI skip 78e5221).** 134 upstream commits (Light Shadows,
  Shadow Distance, camera, races, GM travel); map resolved as "combine": fork map files kept, upstream's
  Map/Quest Helper switches, GM Ctrl+click travel, cache path, quest rim and O/G keys ported. MiniMax
  review: 3 HIGH, 4 MEDIUM, all fixed (776fe2c). Upstream's AO test
  `the_foot_of_a_wall_darkened_and_open_ground_not` fails on untouched upstream (RTX 3070 Ti and the CI
  runner): skipped in all three workflows with the owner's OK. Round 2.5 inherits the options and map
  tidy listed above; the sync resolved options.rs/binds.rs by hand (budgets rose: options +46/-142).
- **Round 2.5 result, 2026-11-10.** Branches chore/r25-options-tidy and chore/r25-gates (a853297), not yet on develop. Items 1-6 and 8 done: zero-addon gate in CI, Play-shaped fixtures, picture tests (golden PNGs absent, comparison skips), options/touch/map files restored to reviewed upstream, 64/650 first-run migration, footprint records re-measured (fork lines in upstream files 7166 to about 6500; options-family 1286 to 1082; dry-run conflicts 8 to 7). Item 5's newer upstream touch controls (e8d7c7d) wait for the next sync. Item 7 dropped by the owner. Open: golden PNGs, M3/M4 notes in docs/plans/r25-options-tidy-report.md. Next: owner plays a branch build, then merge to develop, then a sync, then round 3.
- **Round 2.5 picture tests, owner review 2026-11-10.** The owner looked at the three captures (r25-*.png): they look fine but are hard to read and do not show every UI element. They are not approved as references; the three golden comparisons stay skipped. Before approving, the fixtures would need to show each UI's full set of elements at a readable size (backlog; low priority, the owner's live test remains the real check).
- **Upstream sync 2026-11-10 (develop dde6ecc).** 95 upstream commits to 5d54c0c (raids, Number Colors, Pet Effects, DPS meter, Macros, touch controls) by Sol high; MiniMax review: 1 HIGH (duplicated GM legend), 1 MEDIUM, fixed by free DeepSeek. Owner live test passed (Controls, map, ToT). Known GPU-only failures on the owner's PC, green on CI: two AO tests and upstream's particles16.
- **Round 3 (HUD extras as mods), 2026-11-11.** Branch feat/eqoa-api-r3, merged into develop e31d280. Four bundled mods on by default (Stats Box and ToT start off, as before); new API: player.stats, power/target_id, actions.assist, windows.open("summary"), mod keys on Controls (own file), mod option rows, hud.frame/layout/visible and a native HUD layer (text/gauge/image in the game's HUD pass, for an identical look and layering). Route: Gemini Flash built (Argon REJECT: 12 HIGH; its fix round invented proving tests), DeepSeek Pro/Flash and Argon cut off by limits, Codex Sol proof round (native PNG references, real tests), Claude Sonnet final review (1 HIGH: mod keys lost on save) and final fixes; Sonnet found the benchmark measured nothing and fixed the real cost (1.2-1.9 ms to 0.38-0.40 ms for four mods). Gates: client 1987 passed, addons 374, net 87, tools 97, zero-addon gate, wasm, footprint PASS, dry-run clean. Owner live test passed 2026-11-11 (frame rate fine). Backlog: key-conflict handling (round 7). Owner: round 3u folded into round 8. Next: round 4 (map).

- **Round 4 (map API and map mods), 2026-11-11/13.** Branch `feat/eqoa-api-r4` (6ff7992 + docs), not yet on develop. Preceded by two upstream syncs (a3cedec, 11aa5f0 Move HUD) and releases 0.5.2/0.5.3. API: map state/markers, upstream's own minimap rendered to a texture (no mini.rs change), mod-owned guides, minimap hide, pointer events/claims, input over the open map, packaged/async JSON, shared files between mods, mod commands, player-click import/export. Mods (bundled, on by default): Creature Journal, Map Search (F2/F4 now mod keys), Round Minimap; no skin mod (M map takes upstream's look; skinnable map in the backlog). Upstream's map family equals 11aa5f0 apart from 3 small seams. Builders: Sol (A, B), Claude Sonnet (C, owner's trial); reviews Sonnet (A, B) and Sol (C); Terra checks. Clean idle-PC release frame cost: none 0.28 ms, all seven mods about 1.4 ms. Known limits: clicking the round minimap does not open the map; diagonal quest steps cropped by the circle; no disc here, so real terrain/indoor pictures only in the owner's test. Owner branch-build test pending (gate 3). Owner live test 2026-11-15 (busy spot, fps): 0.5.3 39-43 mods off; round 4 39 mods off, 37 with the journal, 34-35 with Modern UI, round minimap and the other mods; Wereoxxs 26-31 on both builds (not maintained for now). Merged into develop.
- **Round 5 (items), 2026-11-15/16.** Branch `feat/eqoa-api-r5`, merged into develop e909d8a7 (with the dev
  tools 3830fc95). API: `eqoa.inventory/bank/loot/merchant/items/gems`, four `*.changed` events (native
  causes, open/close/reset; per-frame check scalars only), `Item.tabs` from the game's own tab rules, loot
  `Container.source`, six Forge adapters (transactions deferred to round 6). Owner-approved seams: play.rs +1
  (loot request names its corpse), itemmenu.rs +4 and merchant.rs +5 (read-only state), npcw.rs +1/-1.
  Example mod `eqoa_bag_viewer` (off by default). Route: Opus lead, Claude Sonnet builders (A; B + seams),
  Terra reviews (A REJECT then fixed; B APPROVE WITH CHANGES; seams REJECT twice, fixed against the pinned
  Forge reference), Haiku checker/packager. Gates on merged develop (tools/run_gates.py): client 2318 passed,
  0 failed, 50 SKIP lines; addons 471; wasm, footprint, paths, player-release, release PASS. Owner test of
  the branch build passed 2026-11-15. Known limit: if the server ignores a loot request, corpse names stay
  off until the next zone (unverified on live traffic). Claude about 20 points of the month (36% to 56%).
  Next: round 6 (scope above), fresh lead.
- **Round 6 (actions and the rest), 2026-11-16/17.** Branch `feat/eqoa-api-r6` (2e4fb5fe), merged into develop;
  owner test passed (package EQOA-Enhanced-Round6-Test at 2e4fb5fe). One game action per player gesture
  (mod key, claimed pointer press, ImGui click), else `needs_player_action`. Part A reads (windows, dialogs,
  quests, masteries); Part B click actions through an action broker plus bag viewer item actions; Part C
  click-started batches (Mark as junk, Sell junk, Take all but junk; stop on any refusal, Stop button); 11
  window/quest actions; row/dialog/close actions refuse on 5 windows whose rows are not readable (accessors in
  the backlog). Destroy unsupported. Upstream: 3 one-line hook calls (npcw.rs, bankplay.rs, play.rs camera),
  approved by the lead under the owner's delegation. Route: Gemini Hydrogen built A and B+C (trial item 12),
  Terra reviewed (A REJECT 2H; B+C REJECT 3H, then 2 rechecks), Claude Sonnet fixed A, fix 3 of B+C, merged
  A with B+C and built the window actions; Hydrogen reviewed those (8 of 8 findings valid, all fixed). Gates
  `--release` on the branch: full 2365 passed, 0 failed, 30 SKIP; addons 484; release PASS. Not verified: row,
  dialog and close actions end to end (native key presses; needs the live game). Tools merged into develop
  meanwhile: run_gates stale-lock fix and progress lines, make_owner_test_package `--build`.
  **Owner test 2026-11-17:** first pass found the live server closes the corpse after every single take
  (never 0x0019/0x001A; read from the package's eqoa.log), breaking Take all but junk and the loot log; plus
  icons, window size, a hidden Take all. Hydrogen fixed all in 3 rounds (free DeepSeek review: 0H/4M/2L, fixed),
  0 upstream lines. **Owner retest PASSED** (51bd3bf3); merged into develop 0d664fd4; gates `--release` ALL PASS.
  Owner then dropped the one-gesture rule and approved round 6.5 as a general window takeover (above).
