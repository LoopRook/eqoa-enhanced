# Round 7 Part D2: author documentation as built

Branch `docs/eqoa-api-r7-d2` (from `develop` at 02cf7824, with `docs/eqoa-api-r7-d1` merged in). Docs and one
test file; no game code changed, no upstream file touched, the API surface is unchanged (no regeneration
commit).

Method: the old spec was checked against the code section by section (five read-only passes, each reading
the implementing files and the tests; the top findings were then re-read in the code by hand), and against
the generated `docs/modding/eqoa-1-surface.json`. Where the old text and the code disagreed, the code won
and the text changed. Where the code looks wrong, the item is listed in section 1 below and the spec either
says "declared and never filled" or carries the item under "Under review, not part of the promise".

## 1. Suspected code defects (not fixed; for the lead)

Each was read in the code unless marked "(reported by a checking pass, not re-read)". Paths are under
`crates/eqoa/src/fork_ext/`.

### Worth a decision before the freeze

| # | What | Where | In the spec as |
|---|---|---|---|
| 1 | `Windows.active` is declared `supported` and is always nil. | `r6a_capture.rs:748` (`"active": Json::Null`) | "declared and never filled: find the focused window in `windows`" |
| 2 | `eqoa.actions.follow(nil)` stops a follow, but when nothing is followed it starts following the current target (it calls the game's toggle). | `play.rs` `ActionKind::Follow` (about 1997-2006) | under review |
| 3 | `eqoa.actions.stop()` ends auto-attack, auto-run and follow whoever started them (the player's own too). The old text said "if this VM started them". | `play.rs` `ActionKind::Stop` (about 2049) | under review |
| 4 | `eqoa.actions.equip(item)` is the game's toggle: it wears an unworn item and takes off a worn one. There is no "wear"/"take off" form; the old text's `equipped` argument and "already in that state: no packet" do not exist. | `addons/action_broker.rs:4141`, `play.rs` equip dispatch | under review |
| 5 | `eqoa.actions.cycle_target` accepts `"hostile"` and `"friendly"` and treats them as no filter (only `"group"` and `"corpse"` change anything). | `play.rs` `ActionKind::CycleTarget` (about 1491-1521) | under review |
| 6 | An `ActionResult` in state `dispatched` carries `ok = true` before the outcome is known. | `addons/action_broker.rs:1589` | under review; the spec says to read `state` |
| 7 | **Owner rule against the code.** The owner's answer (2026-11-19) is that an irreversible step keeps a confirmation a mod may skin and never skip. As built, `drop_quest`, `mastery_buy`, `mastery_refund` and `unmemorize_all` are direct: the call is the decision. Whether `mastery_buy` bypasses the Class Mastery window's own confirmation was not determined. The spec states the rule only for the game's own Yes/No boxes (they are raised and must be answered) and says plainly that `drop_quest` asks nothing. | `play.rs` dispatch of those kinds | stated as built; the rule's reach is the lead's call |
| 8 | `eqoa.entities.status` and `eqoa.abilities.scope` (the constant tables) have no descriptor of their own: `eqoa.capabilities.get("eqoa.entities.status")` is `unknown`, while `eqoa.items.slots` is described. Their keys are described. | surface file | named in the spec; the docs test accepts a name that has described children |
| 9 | Draw-list methods (`AddLine`, `AddText`, ...) have no capability descriptors at all, so the support table cannot list them (the old text promised `ImDrawList.<Name>` paths). | surface file; `addons/bridge_drawlist.rs` | the spec lists the 14 built methods and says they are not in the registry |
| 10 | `eqoa.options.button` returns a record of type `eqoa.options.Button` that has no rows in the surface. | `addons/option_buttons.rs:109` | return shape given inline |
| 11 | `ChatMessage.channel` is only ever `group`, `unknown` or `system`: say, shout, guild and tells all read `unknown`. A chat mod cannot colour or filter by channel. (`sender`, `channel_code`, `outgoing` are already declared `unsupported`.) (reported by a checking pass, not re-read) | `api_capture.rs:411-418` | stated as built, "more values may be added" |
| 12 | `resource.failed` errors name capabilities that do not exist (`eqoa.texture.load`, `eqoa.font.load`, `eqoa.sound.load`; the modules are `textures`, `fonts`, `sound`). `resource.ready.data.path` is declared for the event but set only for import and export. | `addons/native_events.rs:852` | fields listed as optional |
| 13 | A core-side event loss reports `events.dropped.first_seq/last_seq` as `seq+1 .. seq+count`, not the numbers of the lost events. | `addons/native_core.rs:918-919` | fields named, not explained further |

### Low

- `addons/native.rs:815-816`: the "takes no arguments" list names `eqoa.chat.channel` and `eqoa.combat.counters`; the functions are `get_channel` and `get_counters`, so those two accept and ignore arguments.
- `eqoa.input.bind` with a callback adds an `input.key` listener on every call; a second call makes the callback run twice per press (reported, not re-read: `addons/native_state.rs:1320-1389`). The spec says to call it once per enable.
- `eqoa.abilities.in_range` outside the world answers `stale_handle`, not `not_in_world` (reported: `addons/native_state.rs:752-758`).
- `Player.hp_fraction` can read 0.0 where `Entity.hp_fraction` of the same entity is nil (reported: `api_capture.rs:181,191`).
- `Group.raid` is filled also when `in_group` is false; a member the client does not see has a signed `world_id` byte (reported: `api_capture.rs:199-204`, `group.rs:118`).
- Calling `takeover` again with another scope changes the claim's scope but not a hidden session already running (reported: `addons/window_takeover.rs:318-325`).
- The Forge quest log treats more than 8 titles as unavailable; `eqoa.quests.list` cuts to 8 (reported: `addons/quest_log.rs:29`, `r6a_capture.rs:808`).
- `eqoa.events.after` with an absurd delay answers `failed`, not `out_of_range` (reported: `addons/native_events.rs:337`).
- `chat` and `command` validate the text before the capability check; every other action checks the capability first. `files.import/export` check the permission and the player's act before the arguments.
- Depth limits are off by one against their wording: command payloads allow 9 levels ("8"), JSON loads 33 ("32") (reported: `addons/exchange.rs:106`, `addons/async_json.rs:51`).
- `eqoa.text.localized` reads language column 0 (reported: `addons/native_state.rs` about 545).
- `world.leaving` never gives the reason `logout` (the old text listed it); the spec lists `zone`, `respawn`, `disconnect`.
- Stale texts in code: `addons/action_broker.rs:2` still says "gesture-gated"; `eqoa.windows.open`'s `busy` message says "already queued this tick" for a 16-deep queue.
- Already in the backlog and unchanged: ordinary mod keys fire on the loading screen and over the full map; HUD tokens in `replaces` are labelled "Replaces game windows" when the mod holds the takeover grant and "Replaces HUD" only without it (round 7C's noted split).

## 2. What changed, per document

- **`docs/modding/EQOA-API-SPEC.md`**: rewritten (1,008 lines of design text to about 1,070 as built).
  - New header: version and revision, the freeze (with the Stable release that first carries rounds 4 to 7),
    the generated support table as the authority, how to feature-test, and what "design intent" means. The
    freeze date is left for Part G.
  - Section 1 conventions rewritten around what a call returns; the error-code list is the codes the code
    produces (see section 3).
  - Section 2: the manifest's fields in one table, taken from `addons/discovery.rs` (`schemaVersion`,
    `runtime`, `api`, `id`, `entry`, `name`, `entryMode`, `enabled`, `capabilities`, `replaces`, `shares`,
    `commands`, `permissions`; `large_data` is a `capabilities` entry). The hand index of modules and record
    types is gone: the page links the generated `SUPPORT-TABLE.md`. New: what the player is shown (the exact
    label strings) and the disclosure rule.
  - Section 3: life cycle and the event list as built; the addenda about events folded in.
  - Sections 4 to 8: every function and record against the code and the surface file; the "Open questions"
    addenda (JSON files, shared files, commands, import/export, option buttons, pointer, map views, journal
    fields) folded into their sections. Section 7 gets the takeover rules as built (scopes, the order of
    checks and their error codes, `replaces`, the labels, the switch, hidden sessions, the re-list rule).
    Section 8 gets the permission rules, `ActionResult` as built, the action list with the real arguments,
    `open_window` kinds and aliases, the edit-box table, batches.
  - Section 9: the sandbox as built, with one table of tested limits and one sentence of design-intent ones.
  - Section 10 is new: **Keys** (the round 7E rule). The old section 10 (native integration and neutral
    hooks) and the old section 12 (delivery gates) were design-time text for client builders, full of line
    references to a base that no longer exists; they are removed. They remain in history at
    `git show 02cf7824:docs/modding/EQOA-API-SPEC.md`.
  - Section 11 (Forge) is now what an author needs; the per-module mapping table is replaced by the generated
    table's per-name status. Section 12 is the round-label table, renumbered to the plan (2 core, 3 HUD,
    4 map, 5 items, 6 actions/quests/camera/windows/mastery with 6.5, 7). Section 13 kept, shortened.
  - No design-time line references remain.
- **`docs/modding/GETTING-STARTED.md`**: rewritten: where mods live, the manifest, a first mod, Mods & Addons
  and its F10 key, Rescan/Enable/Reload, where errors show, the rules that prevent bugs, capability probes,
  settings and files, reading the game, keys, actions as built (declaration, `not_declared` /
  `permission_denied`, chat against command, `action.result` states). Nine Lua examples and two manifests,
  all run by the test.
- **D1 pages (reviewed and corrected):**
  - `README.md`: the "still being brought up to date" status removed; the support table added to the index;
    the freeze stated.
  - `VERSIONING.md`: the promise about error codes narrowed to what a code means and the shape of a result
    (the code list can grow); new "What is not promised" (design-intent numbers, the under-review list,
    performance, Forge); the `capabilities` paragraph corrected (declaring is required for acting, as built);
    the downgrade policy of plan risk 9 written out (owner's approval, the surface file changed in the open,
    a release note).
  - `EXAMPLES.md`: each row checked against the bundled mods' code (Mana Bars no longer reads the group; the
    minimap uses `pick_dot`; Map Search's and Bag Viewer's rows now say what they show since rounds 6.5 and 7).
  - `AGENT-GUIDE.md`: the placeholder `eqoa.x.y`, "round 6.5 is not on every build", "200,000 instructions
    per callback" (no such limit: 4 million per frame) and "Addon Manager" corrected; key and permission
    rules added.
  - `template/main.lua`: comment only (Mods & Addons, F10). The template loads clean (tested).
- **`docs/FORGE-COMPATIBILITY-HANDOFF.md`**: "Next" points to the mod API plan and `docs/modding/` (owner
  answer 5b).
- **`FORK-UPDATE-NOTES.md`**: one "Unreleased" entry (author docs for the API).
- **`crates/eqoa/src/fork_ext/addons/docs_tests.rs`** (new, fork-owned) and one `#[cfg(test)] mod` line in
  `addons/mod.rs` (fork-owned).

## 3. Where the old spec disagreed with the code (the code won everywhere)

The whole list would repeat the spec; these are the ones that change what an author writes.

| Area | Old text | As built |
|---|---|---|
| Stubs | planned names answer `not_yet` | every unbuilt name answers `unsupported`; `not_yet` is only `actions.move/look/buttons` (and a quiet "owner still starting" from `load_json`) |
| Error codes | list included `cooldown`, `out_of_range_target`, `not_usable` | never produced. Produced and missing from the list: `not_ready` (the usual action refusal), `window_closed`, `refused`, `requires_player`, `limit_exceeded` |
| `Error` | `retry_after` | declared `unsupported`; `capability` is the function's own name |
| `Capability.revision` | additive contract revision | 1 if supported or partial, else 0 |
| Diagnostics UI | "Unsupported/Failed capability badge and expandable message" | `[Warnings: N]` and a flat list; "Failed — reload to retry" |
| `player.stats()` | 16 `StatItem`s with name/base/current/change | also `hp_max`, `power` (maximum power), `columns`, `resists`; `StatItem` also has `short`, `word`, `stat_id`, `min`, `max`, `without_min`, `without_max`, `effect_change`, `shift` |
| `Entity` | positions optional; no journal fields in the record | `position` and `actor_position` always present; `world_id`, `entity_type`, `is_creature`, `corpse`, `search_flags`, `disposition_name` |
| `Hotbar.kind`, `HotbarSlot` | `commands`, `other`; `command`, `icon` | only `abilities`, `items`, `weapon_sets`; `command` and `icon` `unsupported` |
| `chat.history` | max 1,000 | a limit over 1,000 is refused, not cut |
| `input.claim` | "physical key names" | only Enter, Tab, Backspace, Escape, Space or one printable character |
| `input.capture_text` | renewed while the editor is active | lapses about a second after the last call; the mod must keep calling |
| Mod keys | "conflicts remain a backlog item" | the round 7E rule (section 10 of the spec) |
| HUD | no `minimap` element; `hud.image` string ids are plain blits | `minimap` is an element (not in `all`); `Layout.minimap`; a string `ui_id` other than `"panel"` is refused; texture mode with `shape`; 256 calls a frame (`limit_exceeded`) |
| ImGui "round 4 additions" | tooltips, mouse queries, style vars, item width, window focus | all 22 are refusals (`unsupported`) |
| Draw list | quads, paths, texture stack, wrap width, foreground list | not built; 14 methods exist |
| Textures | JPEG in round 4; `unsupported` for a missing decoder | PNG only; every failure is `not_found` |
| Geometry budget | 196,608 | 524,288 (pinned by a test) |
| Icons | unknown class or disposition `nil, nil` | `invalid_argument` / `not_found`; round 6 |
| `files.list` | "fails" over the cap | `quota`; Windows only |
| `mods.send` | "nil, Error" | `quota` for payload problems, `unavailable` for a missing target, `invalid_argument` for an undeclared command |
| Items | `button_hint` `not_yet`; `CanUseResult`; no `wrong_window` | `button_hint` filled; the type is `CanUse`; `merchant.price` answers `wrong_window`; causes also `fishing`, `loot_refused`, `refresh` |
| `Window` | no tabs | `tab`, `tabs` for Abilities, Class Mastery and the item page |
| Takeover | prose | the order of checks and codes as in the spec's table; idle timeout applies to scope `own` only; `release()` with no argument releases all; `taken()` with no argument returns a table |
| Map | `images`, `load_image`, `MapImage`, 8 pending | `unsupported` (replaced by `map.view`); views: size 16..1024, four per mod |
| Waypoint | errors from the call | world and ground checks come back as the request's result (`invalid_argument`, `not_ready`); the call checks world, Map setting, capability, shape |
| `ActionResult` | `{request, action, phase, status, error}` with applied/sent/confirmed | `{request_id, kind, state, phase, ok, code, reason, queued_tick, dispatched_tick, completed_tick, detail}`; states queued/dispatched/completed/refused/cancelled/unknown; an event at dispatch and at the end |
| Queue full | `quota` | `busy` |
| `cancel` | only unsent requests | a sent request is also cancelled (stops waiting; the server may still act) |
| `assist`, `target_target` | return a bool, no capability | capability-gated requests like any action |
| Arguments | `cycle_target(group:bool)`, `attack(down)`, `cast(ability, {target})`, `equip(item, equipped)`, `assign_ability(ability, slot)`, `bank_move(item, destination, quantity)`, `talk` kind `"any"`, `menu_command(ref, command, arg)` | `cycle_target(filter?)`, `attack(on?)`, `cast(ability)`, `equip(item)`, `assign_ability(slot, ability)`, `bank_move(item, count?)` or `(from, item, count?)`, `talk` kinds `merchant`/`bank` (omit for any), `menu_command(ref, command)` |
| Not built | default-target capture, explicit target options, movement leases and arbitration, `destroy` | `destroy` `unsupported`; `move`/`look`/`buttons` `not_yet`; the rest removed from the text |
| Batches | any action as a step | 23 step kinds; a `target_target` step needs the `assist` capability |
| Report guard | `#bug`, `/bug` | also `/feature`, `#feature`, behind chat wrappers |
| `input_text` | no length | at most 512 characters |
| Sandbox | "200,000 per callback" (D1) | 4 million per frame; `io` has `open` only; private data lives in the player's configuration folder, not in `mods` |
| Events | catalog lacked several | `guide.lost`, `pointer.pressed`, `pointer.wheel`, `shared.changed`, `mod.command`, `batch.progress`, `batch.result`, `player.stats_changed`; `resource` kinds are texture, font, sound, json, json_saved, import, export |
| Cross-mod order | "addon-ID order" | not promised in the new text (the manager's order is by entry path, then id) |

## 4. What is marked "design intent, not part of the promise" (plan risk 7)

No test pins these numbers (or the test needs a graphics adapter and skips without one):

- Diagnostics: a repeat written at most once a minute; 256 distinct diagnostics per mod, 16 new ones a
  second after a burst of 32; the window's 128 entries; log rotation at 5 MiB.
- 128 subscriptions and timers per mod; 32 option buttons; 32 pointer areas per frame; the one-second text
  capture hold; 64 log lines between frames; batch length 1 to 64.
- Hidden window sessions: 25 units walk-away, 15 seconds idle, 10 seconds unanswered, the 3-second re-list
  grace.
- Texture size, count and bytes; media and font and sound byte sizes; font size; `require` depth and source
  size; settings depth and size; the 1,024-entry listing cap; the 1,000 chat lines; 20 wheel steps; four map
  views and their size range; 32 entries per manifest list; 512 characters into a text box; a command's
  depth and value count; how a large JSON value is sliced over frames.

(Corrected in review round 1, below: the ImGui call, geometry and scope budgets and the 16-mod limit were
listed here and are tested; they moved to the spec's table of tested limits.)

Kept as contract because a test covers them (the spec's section 9 table; since review round 1 a docs test
ties every row to its constant and its test): 8 MiB / 128 MiB memory; 4 million instructions a frame; 1 MiB a
file and 16 MiB a mod; 16 open files; 256 HUD calls a frame; ImGui 131,072 calls a run, 524,288 geometry
units a frame, 64 open scopes; 16 mods; the action queue (32 waiting, 8 a step, one a step refill, 64 a step
for all, `rate_limited` after a second, `unknown` after 10 seconds, 256 results kept); chat lines under 128
UTF-16 units; a 128-byte waypoint label; 1,024 events (2 MiB) and 256 a frame; JSON 16 MiB / 32 deep / two
million values / two loads, 14 MiB to save; four commands per sender of at most 4 KiB; 16 fonts and 16
sounds; three imported copies.

Also outside the promise: the short "under review" list in the spec's actions section (defects 2 to 6
above).

## 5. Tests

`crates/eqoa/src/fork_ext/addons/docs_tests.rs`, five tests. The pages are read from disk when the test
runs.

- `docs_getting_started_and_template_lua_runs_clean`: every fenced `lua` block of GETTING-STARTED and the
  template (a copy of its folder, discovered like any package) is loaded through the production install
  order in a world published from a `Play` (`Play::bundled_ui_fixture`), inside an ImGui frame, for three
  frames (entry, `enabled`, `world.entered`, `state.reset`, `render`). No graphics adapter is needed, so it
  does not skip on a machine without one. It fails on a Lua error, on any diagnostic (a mod's own `info`
  and `debug` log lines are not diagnostics, as in Mods & Addons), on an `eqoa` name the surface file does
  not have, on a called name that only refuses, and on an `eqoa.events.on` name that is not a supported
  event. Every manifest the guide shows must pass discovery.
- `docs_a_misspelt_name_in_an_example_is_caught`: the standing negative control (a misspelt name gives the
  name error and the client's `unknown_symbol` diagnostic).
- `docs_every_eqoa_name_in_the_pages_exists_in_the_surface_file`: every `eqoa.a.b` in the six hand-written
  pages is a surface path (or the table above described paths).
- `docs_spec_names_everything_the_surface_file_calls_supported`: every supported module, function, constant
  table, event, record type, record field and ImGui name of the surface file is named in the spec
  (functions by their full path; events in backticks; ImGui names inside the ImGui section; record types
  and fields by name).
- `docs_readers_find_names_and_blocks`: the test's own readers.

### Results

- Local, with the disc linked (`run/eqoa.iso`, `EQOA_ISO`), through the shared build lock:
  `python tools/run_gates.py --only round,wasm,footprint,paths --filter docs_`:
  **round 5 passed, 0 failed, 0 ignored, 0 SKIP lines**; wasm PASS; footprint PASS (no upstream file
  touched, no budget change); paths PASS. Ten examples ran (the nine Lua blocks of the guide and the template; the
  guide's two manifests were checked by discovery).
- The first run of the tests, against the pages as they stood before the rewrite, failed as it should:
  `AGENT-GUIDE.md:11: eqoa.x.y`, `EQOA-API-SPEC.md: eqoa.entities.status` and `eqoa.abilities.scope`
  (defect 8), and a guide example that called a name the client only refuses.

### Negative control (executed)

Two names misspelt in the pages (no rebuild needed: the pages are read at run time), the same command, then
`git checkout -- docs/modding/EQOA-API-SPEC.md docs/modding/GETTING-STARTED.md` (status clean afterwards):

- spec: the one mention of `eqoa.player.experience_for_level` changed to `...experience_for_levl`;
- guide: `eqoa.player.get()` in the first example changed to `eqoa.player.gett()`.

Result: **2 passed, 3 failed**, each for its reason:

- `docs_every_eqoa_name_in_the_pages_exists_in_the_surface_file` (`docs_tests.rs:379`):
  `EQOA-API-SPEC.md:237: eqoa.player.experience_for_levl` and `GETTING-STARTED.md:50: eqoa.player.gett`;
- `docs_spec_names_everything_the_surface_file_calls_supported` (`docs_tests.rs:471`):
  `1 supported name(s) the spec does not name: eqoa.player.experience_for_level`;
- `docs_getting_started_and_template_lua_runs_clean` (`docs_tests.rs:329`), `3 problem(s) in 10 example(s)`:
  `GETTING-STARTED.md:49: eqoa.player.gett is not in the surface file`; the client's own diagnostic
  `"code":"unknown_symbol" ... "symbol":"eqoa.player.gett"`; and the Lua error
  `attempt to call field 'gett' (a nil value)`.

### CI

GitHub run **44873198507 at 2a2d056f: success** (`python tools/ci_gates.py push|status|cleanup`; `cleanup`
run). addons 680 passed / 0 failed / 25 ignored / 17 SKIP; full 3323 passed / 0 failed / 37 ignored /
378 SKIP; wasm, footprint, paths success; player-release 20 passed; tools 201 passed / 1 SKIP. CI has no
disc and no GPU: its SKIPs are those tests. The five `docs_` tests need neither and ran there.

Commits after that run change no code and no Lua example: the guide's actions section lost its numbered
list (prose only) and this report got its results. The docs tests were run again locally on the final
pages: `--only round,paths --filter docs_`: 5 passed, 0 failed, 0 SKIP lines; paths PASS.

## 6. Not done, not verified

- Nothing was played. The examples run in the test host, not in a live client.
- Function signatures and error codes came from reading the code (by the checking passes, with the top
  findings re-read by hand), not from a test per function. The docs tests check names, not every described
  argument or code.
- The spec's freeze date and the plan's Results entry are Part G's.
- The support table and surface file were not regenerated (no API change).
- The decomp and eqoa-port oracle are not on this machine; nothing here depends on them.

## Review round 1 (cross-family review of b30b5bd7, `round-7d2-review.md`: APPROVE WITH CHANGES)

Every finding was checked in the code before the pages changed. No finding was wrong in substance; one was
overstated (H2, below). Round 7 H's rows were left as they were (`Windows.active`, `follow`, `cycle_target`,
`ActionResult.ok`, the four direct actions, `entities.status`, `abilities.scope`, the draw-list paragraph,
`options.button`, `resource.ready`/`resource.failed`, `events.dropped`, `input.bind`).

### The reviewer's findings

| Finding | Verdict, with the code | What changed |
|---|---|---|
| H1 `bank_move`: omitting `count` moves 1; `"bags"` accepted | **Accepted.** `addons/action_broker.rs`: `let count = count.unwrap_or(1);` in both forms of the binding, and `"inventory" \| "bags" \| "deposit"`. (`loot` differs: `play.rs` `count.unwrap_or(it.quantity)`, the whole stack.) | The row gives both call forms, `count:uint=1`, "omitting `count` moves 1, not the stack", `"bags"`, and that `from` must agree with the item's list. `loot`'s row now says the whole stack is the default. |
| H2 three limits wrongly under "no test pins the number" | **Accepted, with a correction to the reviewer's evidence.** The constants are `CALL_BUDGET = 131_072` (`addons/bridge.rs:112`), `GEOMETRY_LIMIT = 524_288` (`addons/bridge_drawlist.rs:9`) and `self.stack.len() >= 64` (`addons/bridge.rs:180`), and tests do exercise all three. But the cited tests do not pin two of the numbers: `bridge_next_tests.rs:276` only shows that 80 pushes are refused, and `wereoxxs_test.rs` asserts `measured_calls * 4 <= CALL_BUDGET` (the bundled UI fits four times over) and the refusal at an injected limit of 4,096. The geometry test brackets the limit (one batch of 312,000 units accepted, a second refused). Also found while checking: the call count is reset at every run of a mod's handlers (`addons/bridge.rs:890`, `state.calls = 0` in `invoke`), so the old "per frame" was wrong too. | The three are rows of the spec's tested table with the real numbers (the old "about 20,000 shape points" is now "524,288 index units ... about 21,000 points"); the call budget is "in one run of a mod's handlers", with a sentence saying when a run starts. Two new tests pin the two numbers exactly: `docs_limit_imgui_scopes_are_64` (64 open scopes pass, the 65th is refused) and `docs_limit_imgui_calls_are_131072_a_run` (131,072 calls pass, call 131,073 is refused with `limit: 131072`). The spec says plainly that geometry and JSON save rest on tests that do not hit the number exactly. |
| M idle timeout marked design intent while Bag Viewer relies on renewing it | **Accepted.** `bundled-mods/eqoa_bag_viewer/modules/bags.lua` `renew_loot_session`; `addons/window_takeover.rs:321-325` (renewal), `:798-802` (idle end, scope `own` only). **No test pins the number 15**: the only expiry tests use `expire_hidden_session_for_test`, which sets the last activity to `now - HIDDEN_SESSION_TIMEOUT - 1 s` (`window_takeover.rs:893`), so they pass for any value. The behaviour is tested: `round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` (`items_tests.rs`, the idle end) and `round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive` (`addons/bag_viewer_tests.rs`, the renewal). | The idle end and the renewal are now stated as contract ("covered by tests"), with what a mod must do (call `takeover` again every frame or second while it shows the session). The number stays as the code has it, worded "15 seconds in this client; no test pins that number and it may change, so renew far more often than that". **For the lead: a test that pins 15 s needs a clock seam or a constant-reading assertion in `window_takeover.rs`; none exists.** |
| M `entity.*` payloads | **Accepted.** Surface file: `entity.added.data` is `after, changed, ref`; `entity.removed.data` is `before, changed, ref`. | Three rows, one per event. |
| M `settings.on_save` / `on_load` return a `Token` | **Accepted.** `addons/native_settings.rs:97-117`. | Both rows say so. |
| M `hud.image` texture options, `hud.visible` names | **Accepted.** `addons/hud_textures.rs:27-60` (`w`/`width`, `h`/`height`, `shape`, `anchor`, `vertical`); `addons/hud_layout.rs` `hud_element_visible` (`"hud"`, `"all"`, `"stats_box"`/`"plain_play"`, `"mana_bars"`). | Both rows rewritten. The game's internal aliases (`health_bar`, `main_exp_bar`, ...) are deliberately not listed: they are not names the API promises. |
| M `cancel(batch)` against `stop_batch` | **Accepted.** `addons/action_broker.rs` `cancel`: `not_found` when no batch runs; `stop_batch`: `false`. | The `cancel` row states the difference. |
| M `replaces` without the takeover capability | **Accepted.** `addons/discovery.rs:769-776`: a diagnostic, no `return`. | The manifest row says it warns and the mod still loads. |
| M `map.open` lacks `not_in_world` | **Accepted.** `addons/native_state.rs:109-130`. | Added. |
| M what the docs tests do not verify | **Accepted.** | The spec's header, the guide's opening and the README now say the tests check names and load the examples, and that arguments, results and behaviour were read from the code. The test file's header says the same. One of the listed gaps is closed mechanically (a page naming a path that only refuses must say so on that line; below). The others stay open and are listed under "Still not checked by any test". |
| L `files.import` staged name | **Accepted.** `addons/player_files.rs` (always `imports/import-<n>.json`). | Stated; also that `extensions` is required (1 to 16 entries) and the title's default. |
| L `Save profile` / `Load profile` | **Accepted.** `addons/runtime.rs:1044-1047` (shown only with no named profile). | Spec rows and the guide. |

### My own second pass

**(a) Defaults and omitted arguments** (every optional argument in sections 4 to 8 compared with its binding):

| Where | Was | Code | Now |
|---|---|---|---|
| `hud.gauge` `lit` | no default given | `unwrap_or(true)` (`addons/native_state.rs`) | "default true" |
| `hud.text` | no defaults | colour white, `scale` 1, `align`/`anchor` left, `vertical` top, `outline`/`shadow` false; `color` is a list, not `{r=...}` | stated |
| `hud.image` game art | no size default | `w`/`h` omitted = the art's own size (`addons/hud_layer.rs`, `-1`); panel 148 by 132 | stated |
| `hud.frame(size)` | "another window size" | a missing `width`/`height` counts as 640/448 | stated |
| `options.toggle` | no defaults | `label` = `id`, `default` = false | stated |
| `map.view` `size` | no default | 112 (`addons/map_views.rs:305`) | stated |
| `files.import` | `title?` unexplained | "Import file"; `extensions` required | stated |
| batch steps | "that action's arguments by name" | the step fields are `entity_id`, `kind_filter`, `target`, `on`, `item` (or `ware`), `count`, `from`, `direction`, `amount`, `text`, `ability`, `set`, `slot` (`addons/action_broker.rs` `parse_batch_step`); `count` defaults differ per step; `require_window` also takes `"corpse"` | a table of fields per step kind with each default |
| `loot` `count` | "(or `count` of it)" | whole stack when omitted | stated |

Checked and already right: `entities.named(exact=true)`, `chat.history(limit=100)`, `input.claim(suppress=false)`,
`fonts.load(size=16)`, `sound.play` (`loop=false`, `volume=1`), `files.list("", "*")`, `merchant.price(quantity=1)`,
`items.can_use` (the player's own class, race, level), `takeover` (scope `own`), `release()` and `taken()` with no
argument, `buy`/`sell` (`count=1`), `attack()`/`autorun()` (toggle), `talk` (any friendly NPC), `open_loot`
(nearest corpse), `open_window(arg=0)`, `input_text(submit=false)`, `stop_batch()` (the running batch),
`hud.visible()` (`"hud"`). Not touched because round 7 H edits the rows: `input.bind` (its `label` defaults to
`id`, `default_key` to `"b"`), `cycle_target`, `follow`.

**(b) Every number: a test, or design intent.** The spec's section 9 table is now the only place that calls a
number tested (20 rows, 33 numbers). Numbers I had left unlabelled and that no test pins are now marked
design intent: the 1,000 chat lines (asked for and kept), 20 wheel steps, the 2,048-pixel texture side in
`textures.rgba`, font size 256, settings 32 deep and 1 MiB, the 1,024 listing cap, the `require` source size,
512 characters into a text box, map view size and count, a command's depth and value count, and "at most 32"
in the manifest lists (the manifest test checks bad entries, not the cap). Newly listed as tested: 16 mods
(`forge_manager_disabled_defaults_missing_root_and_sixteen_vm_cap`; it needs a graphics adapter), a chat line
under 128 UTF-16 units, the 128-byte waypoint label, action results kept (256), JSON save 14 MiB, a command's
4 KiB, three imported copies. Argument ranges that are the game's own shape (bars 1..4, slots 1..5 and 1..20,
quick slots 1..4, weapon sets 1..5, camera modes 0..3, class ids 0..14, world ids 0..255) are stated as
ranges, not as limits; they were read from the bindings.

The test behind each row (file under `crates/eqoa/src/`):

| Row | Test |
|---|---|
| Lua memory 8 MiB / 128 MiB | `round4_json_calls_keep_ordinary_heap_and_manifest_opt_in` (`fork_ext/addons/file_io.rs`) |
| 4 million instructions | `native_iteration_shares_instruction_budget_across_entry_and_callbacks` (`native_tests.rs`) |
| 1 MiB file; 16 MiB mod | `forge_modern_io_rejects_traversal_code_other_addons_and_oversized_files` (`file_io.rs`); `native_storage_quota_combines_private_files_default_settings_and_named_profiles` (`native_tests.rs`) |
| 16 open files | `forge_modern_io_iterator_gc_keeps_memory_budget_and_close_releases_it` (`file_io.rs`) |
| 256 HUD calls | `text_and_utf16_storage_survive_clear_and_command_limit_rejection` (`hud_layer.rs`) |
| ImGui calls 131,072 | `docs_limit_imgui_calls_are_131072_a_run` (`docs_tests.rs`, new) |
| Geometry 524,288 | `forge_drawlist_geometry_budget_is_shared_across_invocations_in_one_frame` (`bridge_drawlist_tests.rs`; brackets the number) |
| 64 scopes | `docs_limit_imgui_scopes_are_64` (`docs_tests.rs`, new) |
| 16 mods | `forge_manager_disabled_defaults_missing_root_and_sixteen_vm_cap` (`manager.rs`; needs a graphics adapter) |
| Action queue 32 / 8 / one a step / 64; 1 second; 256 results | `round6b_broker_rate_limits_fairness_retention_cancel_and_reset` (`action_broker.rs`) |
| 10 seconds | `round6b_broker_server_timeout_and_subsystem_locks` (`action_broker.rs`) |
| Chat line 128 | `round7c_typed_chat_rejects_the_native_utf16_cutoff` (`fork_ext/r6d_tests.rs`) |
| Waypoint label 128 bytes | `round7c_mod_moves_and_clears_the_players_own_waypoint` (`fork_ext/api_capture_tests.rs`) |
| Events 1,024 / 2 MiB / 256 | `native_events_share_one_retention_limit_for_sources_and_resource_completions`, `native_events_enforce_two_mebibyte_payload_retention_without_losing_dispatch_backlog` (`native_tests.rs`); `an_event_nobody_listens_for_still_spends_the_delivery_budget` (`perf_delta_event_tests.rs`) |
| JSON load 16 MiB / 32 / two million / 2 | `round4_json_read_cap_is_enforced`, `round4_json_depth_is_enforced`, `round4_json_nodes_are_enforced`, `round4_json_shared_mount_and_inflight_quota` (`file_io.rs`) |
| JSON save 14 MiB | `round4_json_private_journal_flushes_on_drop_and_reloads` (`file_io.rs`; a value just under 14 MiB saves) |
| Commands 4 per sender; 4 KiB | `round4b_mod_commands_are_declared_bounded_and_delayed`; `round4b_review_commands_preserve_other_events_and_sender_fairness` (`file_io.rs`) |
| 16 fonts; 16 sounds | `forge_fonts_handle_and_size_limits` (`fonts.rs`); `forge_sound_handle_and_file_size_limits` (`sound.rs`) |
| 3 imported copies | `round4b_review_import_ids_survive_enable_and_staging_cannot_starve_saves` (`file_io.rs`) |

I read each constant in the code and each test's name; I did not re-read every test body to confirm it
asserts the exact number (I did for the ImGui three, the idle timeout, the manifest cap and JSON save).

### Tests added or changed (`docs_tests.rs`, now 9 tests)

- `docs_every_tested_limit_names_its_constant_and_its_test`: every row of the spec's tested table has an
  entry in the test; the row states the number; the named source file still contains the constant's text;
  the named test function still exists; and the word "tested" appears nowhere in the spec outside that
  section. A limit changed in code, a row added without backing, or a test renamed away, fails it.
- `docs_a_wrong_or_unbacked_tested_limit_is_caught`: its standing negative control (a changed number, a row
  with nothing behind it, a stray "(tested)").
- `docs_limit_imgui_scopes_are_64`, `docs_limit_imgui_calls_are_131072_a_run`: the two exact pins.
- `docs_every_eqoa_name_in_the_pages_exists_in_the_surface_file` now also fails when a page names a path
  that only refuses (`unsupported`, `not_yet`) on a line that does not say so.

Executed negative controls (pages read at run time, no rebuild; each restored with `git checkout`, status
clean afterwards), `python tools/run_gates.py --only round --filter docs_`:

- the spec's memory row changed to "8 MiB; 256 MiB" (at c46b830f): 8 passed, 1 failed,
  `docs_every_tested_limit_names_its_constant_and_its_test` (`docs_tests.rs:668`):
  `"Lua memory per mod": the spec says "8 MiB; 256 MiB with ...", the code's number is written "128 MiB"`;
- the line "`eqoa.actions.destroy` is `unsupported`" changed to "`eqoa.actions.destroy` destroys an item."
  (at bd74b1bf): 8 passed, 1 failed, `docs_every_eqoa_name_in_the_pages_exists_in_the_surface_file`
  (`docs_tests.rs:397`): `EQOA-API-SPEC.md:894: eqoa.actions.destroy only refuses, and the line does not
  say unsupported or not_yet`.

Found by the first of those restores: `git checkout` wrote the spec back with CRLF line endings and one new
test assumed LF. The tests now read the pages with either ending (a CI checkout can give CRLF).

### Still not checked by any test

- Defaults, argument order, result shapes and error codes as described (class (a) above): no mechanical
  check; a wrong default is caught only by reading the binding, as the reviewer did for `bank_move`.
- That a record field is described under the right record (the field check matches the name anywhere in
  the spec).
- Handlers in the guide's examples that never fire in the three fixture frames (key and result handlers).
- The number 15 of the idle timeout, the 25 units, the 10-second wait for an open and the 3-second re-list
  hold.

### Gates (review round 1)

- Local, with the disc linked (`EQOA_ISO`), `python tools/run_gates.py --only round,wasm,footprint,paths
  --filter docs_` at bd74b1bf: **round 9 passed, 0 failed, 0 ignored, 0 SKIP lines**; wasm PASS; footprint
  PASS (no upstream file touched, no budget change); paths PASS.
- GitHub run **44899435026 at 42753ca3: success**, pushed, waited for and cleaned up in one command
  (`ci_gates.py push`, `status`, `cleanup`): addons 684 passed / 0 failed / 25 ignored / 17 SKIP; full 3327
  passed / 0 failed / 37 ignored / 378 SKIP; wasm, footprint, paths success; player-release 20 passed; tools
  201 passed / 1 SKIP. The nine `docs_` tests need no disc and no graphics adapter and ran there.
- The only commit after that run adds these lines to this report.
- The API surface is unchanged: no regeneration commit.
