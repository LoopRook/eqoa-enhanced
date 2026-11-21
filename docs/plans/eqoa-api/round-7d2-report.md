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
  `permission_denied`, chat against command, `action.result` states). Eight Lua examples and two manifests,
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
- 16 mods at once; ImGui calls, geometry and scope budgets; texture size, count and bytes; media and font
  and sound byte sizes; `require` depth; settings depth and size; the 1,024-entry listing cap; four map
  views; how a large JSON value is sliced over frames.

Kept as contract because a test asserts them: 8 MiB / 128 MiB memory; 4 million instructions a frame;
1 MiB a file and 16 MiB a mod; 16 open files; 256 HUD calls a frame; the action queue (32 waiting, 8 a step,
one a step refill, 64 a step for all, `rate_limited` after a second, `unknown` after 10 seconds, 256 results
kept); 1,024 events (2 MiB) and 256 a frame; JSON 16 MiB / 32 deep / two million values / two loads, 14 MiB
to save; four commands per sender; 16 fonts and 16 sounds; chat lines under 128 UTF-16 units.

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

Results: see "Gates" below.

TEST_RESULTS_PLACEHOLDER

## 6. Not done, not verified

- Nothing was played. The examples run in the test host, not in a live client.
- Function signatures and error codes came from reading the code (by the checking passes, with the top
  findings re-read by hand), not from a test per function. The docs tests check names, not every described
  argument or code.
- The spec's freeze date and the plan's Results entry are Part G's.
- The support table and surface file were not regenerated (no API change).
- The decomp and eqoa-port oracle are not on this machine; nothing here depends on them.
