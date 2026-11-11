# Round 3 Code Review (`feat/eqoa-api-r3`)

**Verdict: REJECT**

Summary of verified findings: **12 HIGH**, **7 MEDIUM**, **3 LOW**.

---

## HIGH Severity Findings

### H1. Enhanced HUD option toggles and migrated settings are disconnected from all four Lua mods (`"mod"` vs `"eqoa_*"`) and never persisted to disk
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/native_state.rs:645, 661, 670`
  - `crates/eqoa/src/fork_ext/addons/options.rs:18-32, 34-55, 64-78`
  - `crates/eqoa/src/fork_ext/options.rs:120, 139-144`
  - `crates/eqoa/src/fork_ext/settings.rs:123-138, 168-205`
- **What is wrong:**
  1. `native_state.rs` hardcodes `"mod"` as the `addon_id` in `eqoa.options.toggle` (`register_toggle("mod", ...)` at line 645), `eqoa.options.get` (`get_toggle("mod", &id)` at line 661), and `eqoa.options.set` (`set_toggle("mod", &id, val)` at line 670), instead of passing the calling addon's ID (`&id` from `bind_eqoa_table`).
  2. `BUNDLED_TOGGLES` (`addons/options.rs:21-28`), the Enhanced HUD menu (`fork_ext/options.rs:120` items `2000..=2005`), and `client.opt` migration (`fork_ext/settings.rs:168-189`) all key toggles by real addon IDs (`"eqoa_stats_box"`, `"eqoa_target_of_target"`, `"eqoa_mana_bars"`, `"eqoa_hud_numbers"`). Because the Lua mods register under `"mod"`, `bundled_id("mod", id)` returns `None`, assigning them new item IDs `2006..=2011`.
  3. `TOGGLES` (`addons/options.rs:18`) is an in-memory `thread_local! RefCell<Vec<ModToggle>>` with no disk persistence to addon settings or `client.opt`. Meanwhile, `migrate_configs()` (`fork_ext/settings.rs:168-205`) populates `"eqoa_*"` in memory and rewrites `client.opt` on disk with those 6 keys stripped (`write_config("client.opt", ...)` at line 204).
- **Player failure scenario:**
  - A player opens **Options → Enhanced HUD** and flips **Stats Box**, **Target of Target**, **Mana Bars**, **Target HP %**, **XP Bar %**, or **HUD Numbers** on or off. The menu mutates items `2000..=2005` (`addon_id = "eqoa_*"`), while the four Lua mods read `get_toggle("mod", ...)` (`2006..=2011`). Changing any of the 6 Enhanced HUD rows has **zero effect** on what is drawn in game.
  - Worse, an upgrading player's existing `client.opt` choices are stripped from `client.opt` on first launch and only written to the unused `"eqoa_*"` in-memory entries; on the second launch, their saved preferences are permanently lost.

### H2. Disabling any of the four bundled mods in the Addon Manager leaves its Enhanced HUD option rows visible
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/options.rs:58-62, 118-133`
  - `crates/eqoa/src/fork_ext/options.rs:120, 139-144`
- **What is wrong:**
  - `unregister_addon_toggles` (`addons/options.rs:58`) is never called anywhere in the codebase when an addon is disabled or unloaded.
  - `HUD_CATEGORY` (`fork_ext/options.rs:120`) statically lists `items: &[2000, 2001, 2002, 2003, 2004, 2005]`, and `value_by_item` (`addons/options.rs:130-131`) falls back to `Some(true)` for every entry in `BUNDLED_TOGGLES` even when not registered.
- **Player failure scenario:**
  - A player disables `eqoa_stats_box` or `eqoa_mana_bars` in the Addon Manager and opens **Options → Enhanced HUD**. All six option rows remain permanently listed and toggleable on the Enhanced HUD page, violating the Round 3 requirement that disabling a mod removes its option rows.

### H3. Stats Box and Target of Target default to ON instead of OFF
- **Where:**
  - `bundled-mods/eqoa_stats_box/main.lua:4, 10`
  - `bundled-mods/eqoa_target_of_target/main.lua:6`
  - `crates/eqoa/src/fork_ext/addons/options.rs:71, 94, 114, 131`
- **What is wrong:**
  - On `develop` (`crates/eqoa/src/settings.rs:995, 997`) and per the Round 3 brief, both `stats_box` and `target_of_target` default to `false` (off until turned on).
  - `bundled-mods/eqoa_stats_box/main.lua` sets `local is_open = true` (line 4) and `default = true` (line 10).
  - `bundled-mods/eqoa_target_of_target/main.lua` sets `default = true` (line 6).
  - `addons/options.rs` hardcodes `true` as the fallback default for all `BUNDLED_TOGGLES` in `get_toggle`, `toggle_by_item`, `set_by_item`, and `value_by_item`.
- **Player failure scenario:**
  - A player launches the game on a fresh install and enters the world: the Stats Box and Target of Target bars are immediately drawn on screen without being turned on.

### H4. Pressing `B` (or a rebound Stats Box key) never toggles the Stats Box
- **Where:**
  - `bundled-mods/eqoa_stats_box/main.lua:14-21, 24`
  - `crates/eqoa/src/fork_ext/addons/native_events.rs:424-436, 655-665`
  - `crates/eqoa/src/fork_ext/bindpage.rs:221-235`
  - `crates/eqoa/src/fork_ext/addons/input.rs:160-163, 213-229`
- **What is wrong:**
  1. In `eqoa_stats_box/main.lua:14-21`, the `input.key` listener checks `if e.down and not e.repeat_ then` and `if e.key == bound then`. However, `native_events.rs:424-436` wraps every event in an `eqoa.events.Event` envelope `{ name, seq, time, tick, frame, generation, data }`; the key payload is at `e.data.key`, `e.data.down`, and `e.data["repeat"]`. Accessing `e.down` evaluates to `nil` (and logs an unknown-field diagnostic on `eqoa.events.Event`), so the branch never executes.
  2. Even if `e.data` were read, `main.lua:18, 24` toggles a local `is_open` boolean while requiring `if not is_open or not eqoa.options.get("stats_box") then return end`, rather than toggling the `stats_box` option itself (and never prints `"stats box shown"` / `"stats box hidden"` as `develop` did at `viewer.rs:4346`).
  3. `crates/eqoa/src/fork_ext/bindpage.rs:221` (`dispatch_keypress`) is dead code only called from unit tests; live key events go through `addons/input.rs:160-163`, which only maps ASCII characters plus 5 named keys (`enter`, `escape`, `backspace`, `tab`, `space`), dropping function keys (`f1`–`f12`), arrows, and controller buttons that a player can bind on the Controls page, and firing even while typing in chat or inside menus.
- **Player failure scenario:**
  - A player presses `B` in-game to show or hide the Stats Box (or rebinds it on the Controls page and presses the new key). Nothing happens.

### H5. HUD Layer forces `Anchor::Left` and `Vertical::Top` for all primitives and executes one frame late
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layer.rs:88-89`
  - `crates/eqoa/src/hud.rs:402, 410`
  - `crates/eqoa/src/viewer.rs:6008, 6141`
- **What is wrong:**
  1. `hud_layer::draw` (`hud_layer.rs:88-89`) hardcodes `ui.ui.anchor = render::ui::Anchor::Left; ui.ui.vertical = render::ui::Vertical::Top;` for all queued HUD commands, and `eqoa.hud.panel`/`text`/`gauge`/`image` expose no anchor or vertical alignment options.
     - On `develop` (`crates/eqoa/src/statbox.rs:1066`), the Stats Box used `Anchor::Right` and `Vertical::Top` so it stayed on the right side of widescreen windows. With `Anchor::Left`, in a 16:9 window (`1280x720`, `side = 106.67`), the Stats Box is shifted left by `2 * side = 213.3` virtual pixels (~341 physical pixels), floating near the center-left of the screen instead of under the minimap/hotbar.
     - On `develop` (`crates/eqoa/src/hud.rs:402`), `ui.ui.vertical = Vertical::Bottom` is active when `draw_target`, Target of Target, Target Mana Bar, and Target HP % are drawn. Because `hud_layer::draw` resets `ui.ui.vertical = Vertical::Top`, in any taller-than-4:3 window (`top > 0`, e.g. `798x828` where `top = 24.5`), the Target of Target bar, Target Mana Bar (`y = 408`), and Target HP % (`y = 397`) shift upward by `24.5` while the native Target bar shifts downward by `24.5`, separating them by 49 virtual pixels.
  2. In `viewer.rs`, `p.hud(...)` runs at line 6008 (draining `MOD_QUEUES` via `fork_hud::draw`), whereas `g.forge_runtime.draw(...)` (which runs Lua `render` callbacks that enqueue `MOD_QUEUES`) runs later at line 6141 after 3D/scale resolve. Every HUD command is drawn one frame late, or left stale in `MOD_QUEUES` when `Hud::draw` returns early (e.g. when `hideui::hidden()` is toggled).
- **Player failure scenario:**
  - On a standard 16:9 monitor (`1280x720` or `1920x1080`), the Stats Box renders in the middle-left of the viewport instead of the right edge; on a tall/split window (`798x828` or `1280x1024`), Target of Target, Target Mana Bar, and Target HP % float ~50+ pixels above the Target nameplate.

### H6. Clickable rects on Stats Box and Target of Target call `ImGui.InvisibleButton` outside `Begin`/`End` and use hardcoded `640x448` window metrics
- **Where:**
  - `bundled-mods/eqoa_stats_box/main.lua:65-75`
  - `bundled-mods/eqoa_target_of_target/main.lua:44-54`
  - `crates/eqoa/src/fork_ext/addons/hud_layout.rs:8-19`
- **What is wrong:**
  1. `eqoa_stats_box/main.lua:71-72` and `eqoa_target_of_target/main.lua:50-51` call `eqoa.imgui.SetCursorScreenPos(sx, sy)` and `eqoa.imgui.InvisibleButton(...)` without wrapping them in an `ImGui.Begin` / `ImGui.End` window scope. In Dear ImGui, submitting widgets at the top level places them into the default `"Debug##Default"` window, which renders a visible, titled, draggable `"Debug"` window on screen and captures mouse input across its entire window bounds.
  2. `hud_layout::set_window_size` (`hud_layout.rs:14`) is never called anywhere in the codebase outside unit tests. `get_window_size()` always returns `(640, 448)`. Consequently, `eqoa.hud.frame()` always returns `offset_x = 0, offset_y = 0, scale_x = 1, scale_y = 1` regardless of the actual window resolution, and neither mod accounts for `frame.side` or `frame.top`.
  3. `eqoa_target_of_target/main.lua:49` sets `sh = 46 * frame.scale_y` (`y = 300..346`), whereas on `develop` (`hud.rs:430`) only the 28px name bar (`[20, 300, 194, 300 + vui.entry(26).uv1[1]]`) was clickable.
- **Player failure scenario:**
  - While in the world, a visible Dear ImGui `"Debug"` window appears over the game and swallows mouse clicks/drags across the screen; at `1920x1080`, clicking the actual Stats Box or Target of Target bar does nothing because the hitbox stays at unscaled `640x448` coordinates.

### H7. `eqoa.group.get().members` includes the local player and inactive slots, corrupting Group Mana Bars and drawing over the Raid panel
- **Where:**
  - `crates/eqoa/src/fork_ext/api_capture.rs:157-166`
  - `bundled-mods/eqoa_mana_bars/main.lua:13-22`
  - `crates/eqoa/src/fork_ext/addons/hud_layout.rs:138`
- **What is wrong:**
  1. On `develop` (`crates/eqoa/src/play.rs:8810`), the HUD's group list is `self.group.members[..self.group.count()].iter().filter(|m| Some(m.id) != me && m.active)`.
  2. In `api_capture.rs:157-166`, `self.group.members.iter().map(...)` iterates over all 5 elements of the raw `self.group.members` array without slicing `[..self.group.count()]`, without filtering `m.active`, and without excluding `Some(m.id) == me`. Worse, for the player's own entry (`Some(m.id) == me`), it populates `power` from `self.stats.stat[9]` / `stat[10]`.
  3. `eqoa_mana_bars/main.lua:15-21` loops `for k, m in ipairs(grp.members)` and draws a gauge at `y = 93 + 48 * (k - 1) + 35` whenever `m.power` is non-nil.
  4. In a raid (`!s.raid.is_empty()`), `Hud::draw` (`crates/eqoa/src/hud.rs:382-399`) replaces the group panel with the raid list, which already has its own power bars, but `eqoa.hud.visible("group")` (`hud_layout.rs:138`) still returns `true`.
- **Player failure scenario:**
  - As soon as a player joins a group, `eqoa_mana_bars` draws the player's own mana bar on top of Group Member 1's frame, shifts every other member's mana bar down by 48px onto the wrong group member, and can draw a 5th bar at `y = 320` over the Pet / ToT area. In a raid, it draws group mana bars directly on top of the raid window.

### H8. Target of Target shows when an entity targets itself, shows blank bars for nameless entities, and ignores the Mana Bars option
- **Where:**
  - `bundled-mods/eqoa_target_of_target/main.lua:11-41`
  - `crates/eqoa/src/fork_ext/api_capture.rs:724`
- **What is wrong:**
  1. On `develop` (`crates/eqoa/src/play.rs:4325`), `target_of_target_id` required `t != 0 && t != u32::MAX && t != self.target` and `!n.trim().is_empty()`.
  2. `eqoa_target_of_target/main.lua:13, 25` does not check `target.target_id ~= target.id`, and checks `if not tot or not tot.name then return end`. In Lua, `""` is truthy (`api_capture.rs:724` sets `name` to `""` when absent), so an entity with an empty or whitespace name passes `not tot.name`.
  3. `eqoa_target_of_target/main.lua:39-41` unconditionally draws `tot.power` without checking `eqoa.options.get("mana_bars")` (and without checking if `eqoa_mana_bars` is enabled), whereas on `develop` (`play.rs:634, 8868`) ToT's power bar was gated by `c.mana_bars`.
  4. On `develop` (`hud.rs:431-435`), the ToT bar and name pulsed with `self.glow0` and clipped the name to `TOT_WIDTH - 14` via `Window::text_box`; `main.lua:28-31` draws at fixed alpha `1.0` with no width clipping.
- **Player failure scenario:**
  - When targeting an NPC or player that targets itself, a duplicate Target-of-Target bar appears for the target itself; when Mana Bars is turned off, ToT still draws a mana bar.

### H9. Mods remain visible and clickable over native menus, full-screen Map, Trainer/Merchant/Bank windows, and loading screens
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layout.rs:127-148`
  - `bundled-mods/eqoa_stats_box/main.lua:25-75`
  - `bundled-mods/eqoa_target_of_target/main.lua:10-54`
- **What is wrong:**
  - `hud_element_visible` (`hud_layout.rs:127-148`) only checks `crate::hideui::hidden()` and `visibility::effective_visibility()`. Unlike `Play::right_hud_up()` on `develop` (`crates/eqoa/src/play.rs:8545`), it does **not** check whether the full-screen Map (`self.map.open`), Player Search (`self.search.open`), Loading screen (`self.loading.up()`), or a native menu/dialog mode (`self.mode != 0 && self.mode != 30 && self.mode != 38`) is open.
- **Player failure scenario:**
  - A player opens the full-screen World Map (`M`) or the Start Menu / Options / Trainer / Merchant window. The Stats Box fails to hide when `right_hud_up()` is false, and the ImGui `InvisibleButton` hitboxes from Stats Box and Target of Target remain active over the Map and menus, intercepting clicks and opening the Summary window or firing Assist.

### H10. Stats Box visual, layout, and localization regressions vs `develop`
- **Where:**
  - `bundled-mods/eqoa_stats_box/main.lua:5, 34-62`
  - `crates/eqoa/src/fork_ext/player_stats.rs:95-113`
  - `crates/eqoa/src/fork_ext/addons/hud_layer.rs:142`
- **What is wrong:**
  - On `develop` (`crates/eqoa/src/statbox.rs:1032-1105`), the Stats Box dynamically measured string widths (`layout(f.height, widths(...))`) at `SCALE = 0.6`, `top = 66.22`, `step = 13.75`, using localized stat abbreviations from `vui.words.get(word, vui.language)`, label color `[0.95, 0.82, 0.5, 1.0]`, and backing color `[0.05, 0.06, 0.08, 0.85]`.
  - `eqoa_stats_box/main.lua` hardcodes English strings (`r.short` from `player_stats.rs:95-113`), `scale = 0.55`, `top = 58`, `step = 15.5`, fixed x coordinates `206, 270, 278, 342`, label color `{ 0.85, 0.72, 0.38, 1.0 }`, and panel backing `[0.08, 0.07, 0.06, 0.9]` (`hud_layer.rs:142`). Because `top = 58` starts inside the 9px top border (`54..63`) and row 8 ends at `166.5 + 11.55 = 178.05` inside the 9px bottom border (`177..186`), the top and bottom stat rows collide with the frame borders.
- **Player failure scenario:**
  - Opening the Stats Box shows text overlapping the top and bottom panel borders, wrong gold/backing colors, cramped fixed columns that clip 4-digit HP/Power values, and unlocalized English labels on French/German discs.

### H11. Public API contract violations, unimplemented spec functions, and strict-record field diagnostics
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/native_state.rs:675-713`
  - `crates/eqoa/src/fork_ext/addons/native.rs:184, 191, 515-516, 825, 865-871, 925`
  - `crates/eqoa/src/fork_ext/player_stats.rs:70-71, 114-128`
  - `docs/modding/EQOA-API-SPEC.md:305, 312-315`
- **What is wrong:**
  1. `eqoa.player.Stats` schema mismatch: `native.rs:825, 865-871` and `EQOA-API-SPEC.md:315` define `max_hp` and `max_power` on `eqoa.player.Stats`, and `name word stat_id current base min max change shift` on `eqoa.player.StatItem`. However, `player_stats.rs:70-71, 114-128` populates `"hp_max"` and `"power"` (leaving `max_hp` and `max_power` as `nil`), and adds `"short"`, `"effect_change"`, `"without_min"`, `"without_max"`, `"columns"`, and `"resists"` which are not in `native.rs:865-871`. Because `native.rs` wraps these tables in strict `record()` guards, reading `s.max_hp` or `s.max_power` returns `nil`, while reading `r.short` (`eqoa_stats_box/main.lua:46`) triggers an unknown-field diagnostic every frame.
  2. `EQOA-API-SPEC.md:313-314` and `native.rs:184, 515-516` advertise `eqoa.actions.target_target()` and `eqoa.actions.toggle_stats_box()` as Round 3 functions, but neither is bound in `native_state.rs` (calling them returns `nil, not_yet`). Conversely, `eqoa.actions.assist()` and `eqoa.windows.open("summary")` are bound in `native_state.rs:371-416` but omitted from `EQOA-API-SPEC.md`.
  3. `eqoa.input.bind` (`native_state.rs:697-713`, required by brief Stage A4) is a no-op stub that discards `spec` and `callback` and returns `true, nil`. Instead, `eqoa_stats_box` uses `eqoa.binds.register` and `eqoa.binds.get`, and `"get"` is missing from `MODULES` in `native.rs:191` so `eqoa.capabilities.has("eqoa.binds.get")` returns `false`.
  4. All new Round 3 record types in `native.rs:925` (`eqoa.player.Stats`, `eqoa.hud.Frame`, `eqoa.hud.Rect`, `eqoa.hud.Layout`, `eqoa.options.Toggle`, etc.) are tagged with `Some(2)` (Round 2) instead of `Some(3)`.
- **Player failure scenario:**
  - Any mod author following `EQOA-API-SPEC.md` calling `eqoa.player.stats().max_hp`, `eqoa.actions.target_target()`, or `eqoa.input.bind(...)` gets `nil` or `not_yet`, while `eqoa_stats_box` spams unknown-field diagnostics on `eqoa.player.StatItem.short`.

### H12. Picture comparison tests in `round_3_picture_tests.rs` are fake (compare only quad counts and write dummy bytes instead of images)
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:233-251, 255-492`
- **What is wrong:**
  1. `save_reference_capture` (`round_3_picture_tests.rs:233-251`) allocates an all-zero buffer `vec![0u8; width * height * 4]`, copies the ASCII bytes of `format!("{name}:{quad_count}")` into the first few pixels, writes that blank image to disk, and performs zero pixel or quad-geometry comparison.
  2. All four picture tests (`bundled_stats_box_picture_comparison`, `bundled_target_of_target_picture_comparison`, `bundled_mana_bars_picture_comparison`, `bundled_hud_numbers_picture_comparison`) only assert `assert_eq!(mod_quads, native_quads)` (quad count equality). Because `UiRaster::text_scaled` emits one quad per character regardless of position, scale, anchor, vertical alignment, or color, the tests pass even when elements are hundreds of pixels off-position or the wrong color.
  3. In `bundled_stats_box_picture_comparison` (`round_3_picture_tests.rs:111-123`), `test_view()` crafts stat values (`50..125`) whose digit lengths happen to match the character lengths of the 3-letter stat names so the total quad count (`160`) matches by coincidence.
  4. Lines `257, 316, 384, 453` print `SKIP: no graphics adapter` and `return`, contradicting the report's claim of `0 SKIP` without skip-free assertions.
- **Player failure scenario:**
  - Major visual regressions (wrong widescreen anchor, wrong tall-window vertical alignment, border collisions, wrong colors) pass CI undetected.

---

## MEDIUM Severity Findings

### M1. Upstream file footprint and test hygiene violations vs `5d54c0c`
- **Where:**
  - `crates/eqoa/src/bindpage.rs:48, 53, 105, 182, 221, 350, 408, 595, 657`
  - `crates/eqoa/src/binds.rs:69, 157, 1702-1903, 2207-2282`
  - `crates/eqoa/src/play.rs:3477, 4302-4316, 4738-4749`
  - `crates/eqoa/src/viewer.rs:4340-4349`
- **What is wrong:**
  - `bindpage.rs`: The lead allowed `bindpage.rs` to grow by up to 4 lines for one Controls-page hook with no upstream test touched. Instead, `bindpage.rs` has 7 separate hook sites (8 modified/added lines in production code) plus `crate::fork_ext::bindpage::free_fixture_key(&mut b)` and a comment edit inside `mod tests` (`bindpage.rs:595, 657`).
  - `binds.rs`: Left an extra blank line at line 69; left the fork's modified `"Target: the target's target"` label and help text mentioning `"A click on the Target of Target bar (the Extended page's)"` at line 157 instead of upstream `5d54c0c`'s `"Assist Target"`; and left upstream unit tests modified (`map` instead of `hide_ui`, `"y"` instead of `"b"`).
  - `play.rs`: Left a truncated doc comment from `target_of_target_id` attached to `fn self_entity` (`play.rs:4314-4316`); added `pub fn entity_name` (13 lines at `play.rs:4738-4749`, used only by `fork_ext/api_capture.rs`) directly in `play.rs` instead of `fork_ext/`; added `self.session.agree_power_bars(true)` inline at `play.rs:3477` instead of in `fork_hooks`.
  - `viewer.rs`: Altered upstream comments on `N` and `F` keys (`viewer.rs:4340-4349`).
- **Player failure scenario:**
  - Merge conflicts on upstream syncs, and the Controls page help text for `F` still tells players Target of Target is on the Extended page instead of Enhanced HUD.

### M2. HUD Numbers behavioral and color differences vs `develop`
- **Where:**
  - `bundled-mods/eqoa_hud_numbers/main.lua:7-8, 23-48, 71-88`
  - `crates/eqoa/src/fork_ext/api_capture.rs:724`
- **What is wrong:**
  1. On `develop` (`crates/eqoa/src/fork_ext/hud.rs:206`), Target HP % was suppressed when `s.self_targeted_flag` was true (when self-targeted with XP debt, the target gauge displays XP debt, not HP). `eqoa_hud_numbers/main.lua:71-88` does not check if `target.id == player.id` with XP debt, drawing Target HP % on top of the XP debt bar.
  2. In `api_capture.rs:724`, `"hp_fraction": (e.p19 != 0).then(|| ...)` returns `nil` when `e.p19 == 0`. Thus at 0% HP, `target.hp_fraction` is `nil` and Target HP % disappears instead of displaying `0%`.
  3. On `develop` (`fork_ext/hud.rs:212-218`), Target HP % green/yellow colors were `[0.3, 0.9, 0.3, 1.0]` and `[1.0, 0.85, 0.2, 1.0]`; `main.lua:7-8` uses `{ 0.4, 1.0, 0.4, 1.0 }` and `{ 1.0, 0.9, 0.3, 1.0 }`.
  4. On `develop` (`fork_ext/hud.rs:220`), player HP % was computed from the HUD gauge byte `(s.self_hp as f32 / 255.0 * 100.0).round() as i32` and gated by `health_visible(s)` (which includes `!s.forge_health_hidden`). `main.lua:23-24` uses `player.hp.current / player.hp.maximum` (which can differ or be 0 before stat channel 0x42 arrives) and hides power `0/0` when `player.power.maximum == 0`.
- **Player failure scenario:**
  - When a player targets themselves while carrying XP debt, a green `100%` HP label is drawn directly over their XP debt bar; when a target reaches 0 HP, the `0%` readout vanishes.

### M3. `eqoa.actions.assist()` and `eqoa.windows.open("summary")` drop or mis-gate requests across game modes
- **Where:**
  - `crates/eqoa/src/fork_ext/play.rs:25-35`
  - `crates/eqoa/src/fork_ext/addons/actions.rs:16-38`
  - `crates/eqoa/src/fork_ext/addons/native_state.rs:380, 399`
- **What is wrong:**
  1. `fork_ext/play.rs:25-26` unconditionally drains `ACTION_QUEUE` (`take_action_requests()`) on every step and only executes requests when `if ready && _play.mode == 0`.
  2. On `develop` (`viewer.rs:4344`), Assist (`F`) also worked in manual camera (`_play.mode == 38`); `fork_ext/play.rs:26` silently drops Assist in mode 38.
  3. Conversely, when `_play.mode == 0`, `_play.map.open`, `_play.map.search.open`, `_play.search.open`, `_play.typing.is_some()`, or `_play.is_dead()` can be true. `OpenSummary` sets `_play.trainer_open = true; _play.mode = 12;` directly without checking those states, whereas a native HUD click on `Pick::Stats` was blocked when the map, search, or death state was active.
  4. `native_state.rs:380, 399` ignores the `bool` return from `queue_assist()` / `queue_open_summary()` when the 16-slot queue is full, returning `true, nil` even when the action was dropped.
- **Player failure scenario:**
  - Clicking Target of Target while holding L1/R1 in manual camera mode drops the Assist action; triggering `eqoa.windows.open("summary")` while typing in chat or viewing the map switches `Play::mode` to `12` (Trainer/Summary) mid-overlay.

### M4. Per-frame heap allocations and undrained queues in `hud_layer.rs`
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layer.rs:58-70, 94-121`
- **What is wrong:**
  - Every frame in `hud_layer::draw` (`hud_layer.rs:94-95`), `map.keys().cloned().collect()` allocates a new `Vec<String>` and clones every addon ID string, and `HudCommand::Text` allocates a new `Vec<u16>` via `crate::ui::wide(&text)`.
  - `MOD_QUEUES` is only drained when `Hud::draw` reaches `fork_hud::draw`. When the HUD is hidden (`hideui::hidden()`) or not drawn, queued commands remain in `MOD_QUEUES` across frames.
- **Player failure scenario:**
  - Unnecessary per-frame heap allocations on the render path, and stale HUD commands flashing for one frame when un-hiding the UI.

### M5. `eqoa.hud.layout()` ignores pet/raid dynamic offsets and widescreen/tall-window anchors
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layout.rs:51-125`
- **What is wrong:**
  - `compute_layout` (`hud_layout.rs:51-82`) uses `transform_rect(frame, ...)` which applies uniform center-pillarbox `offset_x`/`offset_y`, whereas native `Render::Ui` (`crates/render/src/ui.rs`) places left-anchored HUD elements with `x - side` and `y - top` (for `Vertical::Top`) or `y + top` (for `Vertical::Bottom`).
  - `pet` panel rect is hardcoded at `(20, 285, 174, 46)` (`hud_layout.rs:78`), whereas in `Hud::draw` (`crates/eqoa/src/hud.rs:386, 401`) the pet bar `y` position is dynamic (`93 + ((group_count - 1) * 48).max(0)` in groups, or below the raid window in raids).
- **Player failure scenario:**
  - Any mod positioning elements via `eqoa.hud.layout()` misaligns with the native HUD on non-4:3 windows and places pet overlays at `y = 285` even when the player is solo (where the pet bar is actually at `y = 93`).

### M6. `fork_ext/settings.rs` ignores multibox config files (`client-2.opt`..`client-4.opt`) and discards write errors
- **Where:**
  - `crates/eqoa/src/fork_ext/settings.rs:149-234`
  - `crates/eqoa/src/fork_ext/bindpage.rs:202-219`
- **What is wrong:**
  - Unlike `migrate_defaults()` (`fork_ext/settings.rs:64`), which migrates `client.opt` and `client-2.opt`..`client-4.opt`, `migrate_configs()` only reads `"client.opt"`.
  - Both `migrate_configs()` (`fork_ext/settings.rs:204, 230`) and `save_mod_bind()` (`fork_ext/bindpage.rs:218`) ignore `write_config` errors via `let _ = crate::settings::write_config(...)`.
- **Player failure scenario:**
  - Multibox windows 2–4 never have their legacy HUD settings migrated, and a failed config write silently drops the player's keybind or migration state without error handling.

### M7. `eqoa.hud.visible(element)` accepts invalid element names and misses leaves for `"all"` and `"group_markers"`
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layout.rs:127-148`
- **What is wrong:**
  - `hud_element_visible` returns `true` for any unrecognized element string (`_ => true` at line 146) instead of rejecting invalid element names per `EQOA-API-SPEC.md:349`.
  - `Some("group_markers")` is not matched (falls into `_ => true`), and `None | Some("all")` only checks 5 of the 14 visibility leaves.
- **Player failure scenario:**
  - A mod calling `eqoa.hud.visible("group_markers")` or a typo like `eqoa.hud.visible("targets")` always receives `true` even when hidden.

---

## LOW Severity Findings

### L1. Dead `trainer_from_hud` field left in `crates/eqoa/src/play.rs`
- **Where:**
  - `crates/eqoa/src/play.rs:472, 2756, 6426, 6708`
- **What is wrong:**
  - `Pick::Stats` was removed from `crates/eqoa/src/hud.rs`, but `trainer_from_hud: bool` and its comment referencing `Pick::Stats` were left in `crates/eqoa/src/play.rs`.
- **Player failure scenario:**
  - No direct player bug, but leaves fork state in `play.rs` after Stats Box extraction.

### L2. `eqoa.hud.text` `shadow` option is undocumented in `EQOA-API-SPEC.md`
- **Where:**
  - `docs/modding/EQOA-API-SPEC.md:305`
  - `crates/eqoa/src/fork_ext/addons/native_state.rs:490`
- **What is wrong:**
  - `native_state.rs:490` implements `shadow?: bool` on `eqoa.hud.text` (and `eqoa_stats_box/main.lua:47` uses `shadow = true`), but `EQOA-API-SPEC.md:305` only lists `outline?: bool`.
- **Player failure scenario:**
  - Mod authors reading `EQOA-API-SPEC.md` cannot discover the `shadow` text option used by the Stats Box mod.

### L3. `eqoa.hud.image` magic overloading (`ui_id = 25` vs `ui_id = "panel"`) is fragile and undocumented in detail
- **Where:**
  - `crates/eqoa/src/fork_ext/addons/hud_layer.rs:121-136`
  - `docs/modding/EQOA-API-SPEC.md:307`
- **What is wrong:**
  - `hud_layer.rs:123` special-cases `if ui_id == 25` to call `vui.blit_bar(..., 25, 26, w, 3, c[3], 0, true)` instead of `vui.blit(ui, ui_id, [x, y])`, making it impossible to blit entry 25 normally or draw any other 3-piece bar, and ignoring `c[0..3]` RGB tinting on `ui_id != 25`.
- **Player failure scenario:**
  - A mod passing `color` to `eqoa.hud.image` for any icon other than `ui_id = 25` gets an untinted image because `vui.blit` ignores `color`.
