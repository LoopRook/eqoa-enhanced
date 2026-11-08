# Round 1 review: the `eqoa/1` mod API spec

Reviewer: MiniMax (read-only). Reviewed at `5a97cbe` (branch `feat/eqoa-api-r1`, base `11e036f`).
Documents: `docs/modding/EQOA-API-SPEC.md`, `docs/plans/eqoa-api/round-1-summary.md`,
`docs/plans/eqoa-api/round-1-report.md`. Requirements: `docs/plans/EQOA-MOD-API-PLAN.md`
("Goal", "Round 1"), `CLAUDE.md`/`FORK.md`, `docs/plans/eqoa-api/data-sources.md`.

**Verdict: APPROVE WITH CHANGES** — the design is complete, matches the owner's decisions and is
buildable; four HIGH items (three wrong native sources and one sandbox/compat break) must be fixed
in the spec before round 2 starts, because implementing them as written would ship dead automation
and break the bundled UIs.

Overall quality notes: 470 file:line citations were checked mechanically (every one resolves to an
existing file and in-range line); the great majority point at the right symbol, the event model,
capability query, diagnostics, versioning, actions, sandbox, hooks, the 25-row Forge mapping and
the round plan are all present and internally consistent. The owner summary is accurate: I found no
statement in `round-1-summary.md` that contradicts the spec. The defects below are concentrated in
the "native data source" column, which is where a round 2–4 builder looks first.

---

## HIGH

### H1. The input seam and the pad snapshot cite the offline playshot path, not the live one
Spec §4 `eqoa.input.get()`, §8 `eqoa.actions.buttons`, §10 hook row "prepare_controls".

- Spec: §4 "`eqoa.input.get()` … `viewer.rs:462`, `input.rs:82`, `A/runtime.rs:192`";
  §8 "`eqoa.actions.buttons` … Merged into a virtual Report **inside the client**, then VI
  pad/Play::set_key (`viewer.rs:462..483`, `P:5220`)";
  §10 "`viewer.rs:462..483`, native merged Report before console.read/PadKeys".
- Evidence: `crates/eqoa/src/viewer.rs:429` — the cited block sits inside `fn shot(...)`
  (`while t < script.ms { … }`, scripted `pad_script`/`touch_script`), i.e. the deterministic
  offline playshot renderer. The **live** path is `App::poll_pad`, `crates/eqoa/src/viewer.rs:2710-2772`:
  report from `self.pads.poll()` (`:2711`), binds/capture rewrite (`:2726-2734`), player's macros
  `report.buttons |= pressed` (`:2747`), touch pad merge (`:2750`), `console.poll` (`:2754`),
  `p.set_key(k, down, w, aspect)` (`:2768`). `A/runtime.rs:192` is `let consumed = match event {`
  in the winit key/mouse capture path — not the pad.
- Failure: a builder puts the virtual-control merge at `viewer.rs:462..483`, i.e. inside the
  playshot only. `eqoa.actions.buttons(...)`, `eqoa.actions.move(...)` and `eqoa.actions.look(...)`
  then return `sent`/`applied` and do **nothing in the live game** — exactly the "fake success" the
  plan forbids. It also misses that the live path merges macros and touch *after* the point the
  spec names, so a virtual button injected there would be clobbered by `report.buttons |= pressed`
  (`viewer.rs:2747`), and that the live path has a capture branch (`capturing`, `:2726`) that
  swallows buttons before `set_key`.
- Fix: cite `viewer.rs:2710-2772` for the live seam (and say explicitly that the seam sits after
  the macro/touch merges and that virtual presses carry their own source tag so the capture branch
  cannot steal them); state that the playshot path is not the game.

### H2. The sandbox rewrite as specified breaks all three bundled UIs (shared `resources` tree)
Spec §9 "Imported Forge paths" and "Private storage".

- Spec: §9 "'Own folder' means the package's private code/resources plus that addon's private
  writable data/profile; no sibling mod, **shared config**, server patch, disc path or arbitrary
  host directory"; "Trusted discovery/import **can** copy/deduplicate approved shared support files
  into **private read-only** mounts"; the row names only `modules_path` and `profiles_path`.
- Evidence: `crates/eqoa/src/fork_ext/addons/host.rs:361-368` sets `UiForge.resources_path` to
  `addon.script_root.join("resources")`, and for a package `script_root` is the **mods root**
  (`crates/eqoa/src/fork_ext/addons/discovery.rs:703-710` adds `script_root/resources` as the
  "Shared resources" dir; `:716` sets `script_root` = mods root). The shared tree is installed as
  `mods/resources/…` (`crates/eqoa/src/fork_ext/bundled.rs:253-291`, test `:836`). The bundled UIs
  depend on it: `bundled-mods/wereoxxs_ui/wereoxxs_ui.lua:159-161` (fonts under
  `resources/fonts/…`), `:759` (`UiForge.LoadTexture(UiForge.resources_path .. "\\gear-icon.png")`),
  `bundled-mods/avoids_modern_ui/avoids_modern_ui.lua:119-122` (legacy `.cfg` read),
  `bundled-mods/avoids_modern_ui/modules/avoids_quest_log.lua:147`
  (`io.open(GetQuestNotesFilePath(), "w")` → `resources_path`),
  `modules/avoids_persist.lua:82` (legacy read-then-delete), and eight more `LEGACY_SETTINGS_FILE`
  paths under `UiForge.resources_path`.
- Failure: round 2 narrows file access to the addon's own folder and provisions a *read-only*
  private mount. Wereoxxs loses its three fonts (silent fallback to the default font) and the gear
  icon; the Modern UI's quest notes and per-module `.cfg` files become unwritable or land in a
  different place, and its legacy-file migration silently no-ops. That is exactly the owner's round-2
  live test ("three bundled UIs unchanged") failing.
- Fix: make provisioning mandatory and explicit: the shared `resources` tree must be materialised
  into each addon's private mount (including a writable area), `UiForge.resources_path` must be
  defined (it is not named in §9 at all), and the spec must state whether legacy `.cfg` reads and
  writes keep hitting the shared tree (with the permission that implies) or are migrated once.

### H3. `CombatHit.color` is documented as a u32 trailer; the native value is a 3-byte RGB
Spec §4 "Chat, combat and input".

- Spec: "Color is optional nonzero u32 trailer parsed at `P:5829`; preserve it through `F:889`
  rather than hardcoding 0 (`A/combat.rs:44`)."
- Evidence: `crates/eqoa/src/play.rs:5827-5830` — "a server's patch (`s2`…): **three more bytes**,
  when there, are the number's colour"; the parse is `rest[0], rest[1], rest[2]` into `[f32;3]`, and
  it only exists when `crate::s2::get().damage_rgb` is on. `P:5829` is only
  `let rest = &r.data[r.pos..];`. `crates/eqoa/src/fork_ext/addons/combat.rs:44` is `color: 0,`
  (the hardcoded zero the spec wants removed).
- Failure: a builder reads a **u32** at the trailer position, i.e. one byte past the packet, and
  then has to invent a packing for `color:uint?` (0x00RRGGBB? 0xFFRRGGBB? ImGui u32?). The Forge
  row (§11 combat) then carries the wrong value into the legacy damage payload, and the Wereoxxs
  damage meter's number colours change.
- Fix: define `color` as the native `[r,g,b]` triple (or state the exact packing), note the
  `s2.damage_rgb` gate, and say that the colour is absent (nil) unless the patch is on.

### H4. `eqoa.windows.get().mode` names are cited to a source that contains no names
Spec §7 "Camera and windows".

- Spec: "mode enum is the native-mode name map in `F:745` (normal, loot, input, text_dialog,
  merchant, bank, trainer, start_menu, confirm, trade, …)"; "native_mode keeps actual number".
- Evidence: `crates/eqoa/src/fork_ext/play.rs:745` is `fn is_menu_mode(mode: i32) -> bool` — a
  predicate over native mode IDs (1 loot, 4 GM goto, 5 spellbook, 6 recipe book, 10 merchant,
  11 bank, 12 trainer, 13 start menu, 14 MBox, 15 trade, 16 trade ask, 17 buddy request,
  18 buddy list, …). There is no name map anywhere in the fork; the spec's 36 names are not tied to
  any ID.
- Failure: the string↔ID mapping is the actual contract. A builder who assumes the listed order is
  ascending mode ID maps native 10 (merchant) to "input", 11 (bank) to "text_dialog", 13 (start
  menu) to "trainer", 14 (confirm box) to "start_menu". Every `windows.mode == "bank"` check in a
  mod then silently fails, and `eqoa.actions.close_window`/`select_row` act on the wrong window.
- Fix: publish the native-mode ID → name table as a normative list, and drop the `F:745` citation
  or replace it with the per-window mode constants.

---

## MEDIUM

### M1. Frame mode + the `render` event: double drawing is not excluded
Spec §3 ("In default frame mode execute the entry once per rendered addon frame
(`A/manager.rs:269`), with drawing legal in that entry") vs the event catalogue row
"`render`, `settings.render` | `{dt:number}` … each drawn addon frame".
The spec never says the `render` event is suppressed in frame mode. A once-mode author who
switches to frame mode, or a frame-mode author who adds `eqoa.events.on("render", fn)` for a
`dt`, draws every window twice per frame (double HUD text, double input handling).
Fix: state that in frame mode the `render` event is not delivered (the entry *is* the render
callback), or that it is delivered and frame mode authors must not subscribe.

### M2. `eqoa.client.display()` promises two numbers with no native source
Spec §5: "`eqoa.client.display()` returns `{width:uint,height:uint,ui_scale:number,render_scale:number}`
(A), from runtime's final-target metrics `A/runtime.rs:259`".
`crates/eqoa/src/fork_ext/addons/runtime.rs:259` is the `snapshot: Snapshot,` parameter of `draw()`;
the real metrics are `runtime.rs:659-663` (`FramebufferExtent::from_texture(final_target.texture())`
→ `set_display_size`, and `set_display_framebuffer_scale([1.0, 1.0])`). `render_scale` is a client
setting (`crates/eqoa/src/settings.rs:740`, values 1/2/4), and **no `ui_scale` exists anywhere in
the tree**. A builder must either invent `ui_scale` or return a value that does not describe
anything, contradicting §1's "no invented values".

### M3. The HUD element enum has no mapping to the native claim registry
Spec §5: "HUD element string enum: `health`, `power`, `experience_main`, `experience_secondary`,
`compass`, `target`, `group_panel`, `group_markers`, `pet`, `effects`, `hotbar`, `chat`, `all`.
`all` expands to the same leaves in the native claim registry".
`crates/eqoa/src/fork_ext/addons/visibility.rs:14-29` has **14** leaves with different names:
`health_bar`, `power_bar`, `main_exp_bar`, `secondary_exp_bar`, `compass_face`, `compass_back`,
`target_nameplate`, `group_member_panel`, `group_compass_marks`, `pet_panel`, `chat_window`,
`hotbar_draw_compact`, `hotbar_draw_expanded`, `active_effects`. The new enum has 12 names, so
`compass` and `hotbar` each cover two leaves. Without the mapping, `eqoa.hud.hide("health")` and
`eqoa.hud.hidden_by("all")` can disagree, and `hidden_by` has no defined result for a leaf the
public enum does not name.

### M4. `eqoa.chat()` can still make the client submit a server bug report
Spec §8 `chat`: "same `chat_line` (`P:4602`) … Text including prefix <128 UTF-16 units". The native
`chat_line` has a branch for a "#bug" prefix that calls `send_bug_note()` →
`session.send_bug_note(&note)` (`crates/eqoa/src/play.rs:4608-4612`, `send_bug_note`). §9's
"Network" row forbids "community submission/update/report-upload API", but the "Processes/host
integration" row only refuses routes that leave the game session (`F3 report export`, browser
open), and the §8 `command` row says to block branches "outside game session" — a server-side bug
note is *inside* the session. A mod can therefore spam the server with bug reports through a
"normal" chat call.

### M5. `eqoa.hud.hide` has contradictory availability
Spec §5 marks it **W** ("In a connected, loaded world") but also says "display projection only
while W". Under §1's rule a W-labelled call outside a world returns `nil, not_in_world`; under the
other sentence it returns a token that hides nothing. One of the two is a fake success the other
is an error, and the spec does not say which.

### M6. The §10 footprint estimate for the dialog accessors is not credible
Spec §10: "`PrivChat::text(&self)->&[u16]`; `Response::view(&self)->ResponseView<'_>`;
`MBox::view(&self)->MessageBoxView<'_>`; views contain actual text/options/selection/style …
At most three trivial read-only accessors, **12 added lines total**".
The fields are private today (`crates/eqoa/src/dialog.rs:10-16` `PrivChat.text`, `:63-70`
`Response.dialog/text/options/sel/first`, `:241-256` `MBox.sel`), so this needs two new public
view structs plus three accessors plus the private-field plumbing — well over 12 lines — in
`crates/eqoa/src/dialog.rs`, an upstream file that currently has **no** fork diff
(`docs/fork-footprint.json` "shared" list). That is new upstream API surface, not a hook call, so
it also strains FORK.md's "shared upstream files carry hook calls only". The lead should re-measure
this row before round 2 asks for budget.

### M7. `not_yet` is a capability status with no matching error code
Spec §2: "Every declared module is registered from round 2, even when its round has not landed.
Before implementation its entries fail visibly with capabilities `not_yet`." But §1's Error code
list has no `not_yet` code, and §1 says unsupported callables return `nil,Error{code="unsupported"}`.
A builder must choose between `unsupported` (indistinguishable from "will never exist") and
inventing a code, which is a breaking change for `eqoa/1`.

### M8. Several index bases are left to the builder
Spec §1 says everything is 1-based, but the following are never tied to a base: `ContainerView.tab`
and `Tab.index` (native tabs are 0..3 and the Forge layer keeps 0..3, §11 inventory), `Effect.index`
(native slots 0..7, §11 effects), `Hotbar.index` / `HotbarSlot.index`, `Window.selected_row`, and
the `row` argument of `eqoa.actions.select_row`/`activate_row`/`dialog_answer`. A builder that
reads §11's "0-based" rows instead of §1 will be off by one on every menu action.

### M9. No consolidated module/function index
The plan asks for "every module, function, field and event" with types, units, index base, nil/error
cases and source. The spec defines them per section but never lists the modules in one place, so
§2's `eqoa.capabilities.list()` contract ("includes modules, functions, record fields, events,
ImGui members and Forge compatibility entries") cannot be checked against anything, and a
capability-path typo cannot be spotted.

### M10. Forge capabilities with neither an `eqoa` equivalent nor a "cannot provide" entry
Checked all 25 reference modules plus the UiForge core. These have no home in §4–§9 and are absent
from §13's "what the client cannot provide": `Util.IsBattleMusicPlaying` (game music state — the
native client tracks it in `crates/eqoa/src/sound.rs`), `Chat.CaptureChannelTypeFromGameWindow`,
`Chat.GetChannelTableInfo`, `Chat.GetMessageTypeString`, `UiForge.IGraphicsApi.CreateTextureFromFile`
(`UiForge/src/core/core.cpp:938-943`), `Util.utf8_to_utf16` / `Util.utf16_to_utf8`, `Util.IsBitZero`
(the `bit` global exists in `host.rs` but is not in the new API's module list). The plan requires
each Forge capability to be mapped, provided, or declared unavailable.

---

## LOW

- **L1. §6 item text sources are off by one table.** "`eqoa.items.slot_info` … native
  `infotext.rs:13`" — `crates/eqoa/src/infotext.rs:13` is `const STAT_WORDS` (stat words); the
  wear-slot table is `WEAR_WORDS` at `:16` and `wear_slot_word()` at `:32`. And
  "`eqoa.items.attack_name`/`stat_name` … `infotext.rs:32`" points at `wear_slot_word`, not
  `attack_word()` (`:38`) or the stat-word lookup.
- **L2. §9 "sound.rs:30" resolves to the wrong file.** `crates/eqoa/src/sound.rs:30` is `Ui(i32),`
  (a `Cue` variant). The budget constants are `crates/eqoa/src/fork_ext/addons/sound.rs:30-36`
  (`MAX_SOUND_FILE_BYTES`, `MAX_SOUND_HANDLES`, `MAX_AGGREGATE_BYTES`). Write `A/sound.rs:30`.
- **L3. §9 "8 MiB Lua heap (`A/host.rs:96`)"** — `:96` is the `"loadfile"` entry in the stripped
  globals list; the limit is `lua.set_memory_limit(8 * 1024 * 1024)` at `host.rs:88`.
- **L4. §5 "runtime's final-target metrics `A/runtime.rs:259`"** — `:259` is a function parameter;
  see M2 for the real lines.
- **L5. §9 "`Lua::new_with` whitelist as `A/host.rs:90`"** — `:90` is the comment "Extract original
  pcall before modifying globals"; the call is `host.rs:84-87`. "Instruction budget cannot be
  defeated with pcall (`A/host.rs:274`)" — `:274` is inside the budget hook; the protected `pcall`
  wrapper is `host.rs:280-289`.
- **L6. §5 "`eqoa.math.bearing` … `A/host.rs:190`"** — that host function (`GetBearingToPoint`)
  returns **degrees**; the new API is radians. The spec's own text is right, but a builder copying
  the cited code gets degrees.
- **L7. §10 "health_redraw … is render-timed (`F:805`)"** — `fork_ext/play.rs:805` is the closing
  brace of `weapon_sets_requests`; `health_redraw` starts at `:808`.
- **L8. §10 "`host::Snapshot` (`A/host.rs:16`)"** — `:16` is the first *field*; the struct starts at
  `host.rs:14-15`. Harmless, but a line-accurate anchor is what the spec promises.
- **L9. §2 "`A/host.rs:220` supplies a bounded Lua log queue"** — `:220` is `forge.set(`; the queue
  is `host.rs:219-230` (64 messages × 4096 chars).
- **L10. §3 `resource.ready`/`resource.failed` cite `A/manager.rs:305`**, which is
  `flush_pending_fonts` — a font flush, not a resource-completion source. Textures and sounds have
  no completion path at all today, so round 2 must build one; the row should say so instead of
  pointing at a font helper.
- **L11. §11 `entity` row**: "GetTargetId reproduces pinned **low-byte read** for compatibility"
  is right and worth keeping, but the spec should say the *eqoa* value is the full u32 and that the
  adapter's low byte is a deliberate divergence from the native target ID (it already is in the
  current adapter), otherwise a round-3 builder may "fix" it and break Forge mods.

## Checked and found sound (no action)

- The 25 Forge module rows are all present (§11), the index bases match the pinned reference
  (`ability_list` 1-based, `ability_bar` bar 0..2/slot 0..4, `effects` 0..7, `entity_list` 0..23,
  `inventory` tab 0..3, `loot`/`bank` 40 slots, `quest_log` max 8) — spot-checked against
  `run/forge-compat-reference/scripts/modules/frontiers_forge/*.lua`.
- `cast` correctly forbids sending `a.slot = -1` as `u8 = 255` (`play.rs:6786-6789`).
- `group.rs:121` s16→f32 cast and the "no /100" conclusion match `group.rs:121` and the HUD/map
  use at `play.rs:7738`/`8112`.
- Class Mastery: x0c available / x10 spent / x08 XP share / x14 raw is consistent with
  `classmastery.rs:253,266,274,323,365,762,873`.
- Abilities are a `BTreeMap<i64, …>` (`play.rs:155`), so "sorted by numeric native ID" is right.
- Every §10 seam is neutral (no Lua, addon, manifest or mod terms in the shared signatures) and
  `fork_ext/play.rs` is a child module of `play.rs` (`play.rs:34-35`), so the "private fields are
  readable" claim holds.
- Round 3 correctly refuses to smuggle transaction actions in early (§6, §12).
- `round-1-summary.md` is consistent with the spec; I found no misstatement in it.
