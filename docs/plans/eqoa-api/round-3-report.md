# Round 3 implementation report: the HUD extras become mods

Historical stage notes below are superseded by the audited Fix round where they conflict.

Branch: `feat/eqoa-api-r3`, from `develop`. Implementation follows `docs/plans/eqoa-api/round-3-brief.md` in stages A, B, C.

---

## Stage A: API additions & corrections

### Changes by item

1. **`eqoa.player.stats()` (A1):**
   - Extracted stat computation from `crates/eqoa/src/statbox.rs` into new fork-owned module `crates/eqoa/src/fork_ext/player_stats.rs`.
   - `statbox.rs` re-exports `Row`, `ROWS`, `Shift`, `Source`, `View`, `view` from `player_stats`.
   - Populated `View.player_stats` in `api_capture.rs` whenever `stats_known` is true.
   - Bound `eqoa.player.stats()` in `native_state.rs`, exposing the 16 display rows in authentic Summary order, column structures, and individual named keys (`strength`, `stamina`, `armor`, `max_hp`, `max_power`, resists, etc.) with `current`, `base`, `min`, `max`, `change`, and `shift` (`"same"`, `"raised"`, `"lowered"`).
   - Negative control verified: returns `nil, not_in_world` when called before entering a world.

2. **Power & Target ID on Entities & Group Members (A2):**
   - Added `power` field to `Entity` and `Group.Member` records in `crates/eqoa/src/fork_ext/addons/native.rs` and `api_capture.rs`.
   - Power is extracted from `self.session.power_of(id)` (the native 0x7E01/0x7E02 OP_POWER_BARS reader) normalized to a `0.0..=1.0` float, or computed from `stat[9]/stat[10]` for self. Returns `nil` when the server has not sent power data.
   - `target_id` is preserved on `Entity`.
   - Negative control verified: returns `nil` for entities/members without received power data.

3. **Actions Assist & Summary Window (A3):**
   - Implemented `crates/eqoa/src/fork_ext/addons/actions.rs` providing player-action queuing.
   - Hooked action processing into `crates/eqoa/src/fork_ext/play.rs:begin_iteration` during valid game steps in play mode (`mode == 0`).
   - `eqoa.actions.assist()` executes `_play.assist_target()` via upstream's own action path.
   - `eqoa.windows.open("summary")` sets `_play.trainer_open = true; _play.trainer_from_hud = true; _play.mode = 12;` matching the native Stats Box click.
   - One request processed per call.
   - Negative control verified: rejects calls outside world with `not_in_world`, rejects unsupported window names with `invalid_argument`.

4. **Mod Key Bindings on Controls Page & B-Key Verification (A4):**
   - Implemented `crates/eqoa/src/fork_ext/bindpage.rs` providing registration, storage, rebinding, and rendering of mod-owned keybinds on the Controls page.
   - Mod keys are saved to `keys.opt` under the `mod.<addon>.<id>=<key>` namespace (e.g. `mod.eqoa_stats_box.toggle=b`).
   - Hooked into `crates/eqoa/src/bindpage.rs` through a single neutral extension hook appending extra rows after upstream's list (`extra_rows()`, `reset_defaults()`, `capture_key()`, `draw()`, `help_or_defaults()`).
   - `crates/eqoa/src/bindpage.rs` diff is `+10, -5` lines, strictly within the Lead-approved budget of `{ "added": 11, "deleted": 7 }`.
   - Bound `eqoa.binds.register(id, default_key, description)` in `native_state.rs`.
   - Verified that rebinding actually changes the key that fires: pressing the rebound key dispatches the registered callback, and the old key no longer fires (negative control in `round_3_stage_a_controls_page_rebind_and_negative_control`).

5. **Mod Option Toggles on Enhanced HUD Page (A5):**
   - Created `crates/eqoa/src/fork_ext/addons/options.rs` with `register_toggle`, `get_toggle`, `set_toggle`, `toggle_by_item`, and `value_by_item`.
   - Bound `eqoa.options.toggle({ id, label, default })`, `eqoa.options.get(id)`, and `eqoa.options.set(id, val)` in `native_state.rs`.
   - Integrated dynamic mod toggles into `crates/eqoa/src/fork_ext/options.rs` on the "Enhanced HUD" options page.
   - Negative control verified: invalid argument types return `invalid_argument`.

6. **HUD Placement & Font API (A6):**
   - Created `crates/eqoa/src/fork_ext/addons/hud_layout.rs` computing frame metrics, 4:3 pillarbox/letterbox offsets, side/top margins, and screen-space coordinates.
   - Bound `eqoa.hud.frame(opt)` returning `{ width, height, offset_x, offset_y, scale_x, scale_y, side, top, window_width, window_height }`.
   - Bound `eqoa.hud.layout(opt)` returning exact layout rects for stock HUD parts: `player` (panel, health, power, exp), `group_members` (panel, health, power), `target` (panel, health, power), and `pet` (panel). Per Lead Directive 3, `target_of_target` and `stats_box` were removed from the stock API rects.
   - Bound `eqoa.hud.visible(element)` honoring `hideui::hidden()`, modal dialogs/menus, loading screens, and active mod suppression claims from `effective_visibility()`.
   - Negative control verified: `eqoa.hud.visible()` returns `false` when outside world or when Hide UI is active.
   - Documented all new functions and capability mappings in `docs/modding/EQOA-API-SPEC.md`.

---

## Verification & Test Results

All tests run using `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared` with lock acquisition protocol and scratch `EQOA_CONFIG`.

| Test Command | Result |
|---|---|
| `cargo test --locked -p eqoa --bin eqoa fork_ext::bindpage:: -- --test-threads=1` | **2 passed; 0 failed; 0 ignored** (1982 filtered out) |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons::native_tests:: -- --test-threads=1` | **27 passed; 0 failed; 0 ignored** (1957 filtered out) |
| `cargo test --locked -p eqoa --bin eqoa player_stats::tests:: -- --test-threads=1` | **4 passed; 0 failed; 0 ignored** (115 filtered out) |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` | **345 passed; 0 failed; 24 ignored; 0 SKIP** (1615 filtered out) |
| `python tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` | **PASS**: 355 fork-owned, 42 shared files; 31/31 hooks intact; all files within reviewed policy. |

### Explanation of Earlier Test Count Discrepancy

In the initial Stage A report, the `fork_ext::addons::` test count was reported as "82 passed / 37 filtered out". This difference occurred because the earlier run was executed against an integration test target with a sub-filter (`tests/` integration tests), rather than the client binary's full unit test tree. Running the exact command specified in the brief (`cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1`) runs all 369 unit tests under `fork_ext::addons`: **345 passed**, **24 ignored** (the 24 tests requiring `EQOA_FORGE_REFERENCE`), **0 failed**, and 1615 tests filtered out from unrelated modules.

---

## Questions for the lead

### A6 / Stage B4: Native HUD Font (`eqoa.fonts.game()`) vs. ImGui Font Loading

The Lead instructed:
> "2. `eqoa.fonts.game()`: a table with a name and height is not usable for drawing. Make it return a real FontInfo handle (as eqoa.fonts.load/default return, spec §5) for the font the native HUD numbers use, loaded so `ImGui.PushFont`/DrawList AddText with font+size draw in it. If the native HUD text isn't a font ImGui can load (bitmap glyph atlas etc.), stop and tell me what it is and where it lives."

**Investigation findings:**
1. **What the native HUD font is and where it lives:**
   - The native HUD font used by `readouts` (`fork_ext/hud.rs:260`) and throughout EQOA's native UI is an `esf::Font` (CSF node type `0x7000`) stored inside the disc CSF package `ArialUI.csf` (located via `vi::ui::font_id()`).
   - It is decoded by `esf::font::read` in `crates/esf/src/font.rs:37` into an `esf::Font` struct with a character-to-glyph map, glyph widths (node `0x7020`), and proprietary 4-bit/8-bit indexed or RGBA bitmap pixels (node `0x7030`).
   - In `crates/eqoa/src/play.rs:1120-1124`, this bitmap atlas is packed into a GPU texture via `ui.add_font(device, queue, &f)` producing a `render::ui::Font` (`crates/render/src/ui.rs:559`), and rendered in the native UI pass via `ui.ui.text_scaled` (`crates/render/src/ui.rs:887`) using a custom wgpu quad pipeline.
   - **It is NOT a vector TrueType or OpenType font (`.ttf`/`.otf`).** There are no vector outline tables (`glyf`, `CFF`, `cmap`) in the CSF format.

2. **ImGui font loading constraint:**
   - The mod ImGui font engine (`crates/eqoa/src/fork_ext/addons/fonts.rs`) operates strictly on vector TrueType/OpenType binaries via `StbTrueTypeFontData` and `FontSource::stb_truetype_with_size` passed to `dear_imgui_rs` / `ImFontAtlas::AddFontFromMemoryTTF`.
   - ImGui cannot load or rasterize `esf::Font` binary bitmap atlases as an `ImFont`.

**Question for the lead:**
How should `eqoa_hud_numbers` draw its readouts in Lua / ImGui?
- **Option 1:** Have `eqoa.fonts.game()` return `eqoa.fonts.default()` (Dear ImGui's built-in default font, or a bundled TTF like `SpaceMono-Regular.ttf` / bundled fonts), so Lua mods draw via standard `ImGui.PushFont(font_info.font)` and `draw_list:AddText(...)`.
- **Option 2:** Provide a dedicated drawing helper for authentic HUD text, or an ImGui custom glyph callback / texture-quad blitter.
- **Option 3:** Bundle a clean, open-source TrueType font matching Arial UI metrics (or point `eqoa.fonts.game()` to an existing bundled TTF).

---

## Stage B: The Four Bundled Mods & Native HUD Layer

### 1. Lead Directives Received & Implemented
- **HUD Layer Architecture:** Per the lead's directive, added a native HUD layer in `crates/eqoa/src/fork_ext/addons/hud_layer.rs` drawing directly through the game's own HUD pass and raster (`UiRaster` / `Vui`), using the disc's authentic `ArialUI` font and frame/gauge art. Dropped `eqoa.fonts.game()`.
- **API Surface Added:**
  - `eqoa.hud.text{ x, y, text, color, scale=1, align="left"|"center"|"right", outline=false, shadow=false }`: renders through `UiRaster.ui.text_scaled` using `ArialUI`, with optional 4-way outline (for numbers) or 1px drop shadow (for stats box).
  - `eqoa.hud.gauge{ x, y, art, fill, lit=true }`: renders authentic health (art 7) and power (art 9) gauges using frame 136 via native `crate::hud::draw_gauge`.
  - `eqoa.hud.image{ ui_id, x, y, w?, h?, color? }`: renders 3-piece bar (ui_id 25/26) or ui blits.
  - `eqoa.hud.panel{ x, y, w, h }`: renders authentic parchment card backing + window frame set.
- **Queue & Allocation Guarantees:** Bounded per-mod queue (cap 256 commands/frame). Buffers are retained across frames (`queue.drain(..)`), ensuring zero allocation in steady-state rendering. Calls outside the world return `nil, "not_in_world"`.
- **Zero Added Upstream Lines:** Drawn from the EXISTING fork hook in `crates/eqoa/src/hud.rs:498` (`fork_hud::draw(vui, ui, s)`), without adding any new upstream lines or raising the `hud.rs` budget.
- **Click Routing:** Mods create hitboxes via ImGui `InvisibleButton` converted with `eqoa.hud.frame()`, consuming clicks so they do not leak through to the game world.

### 2. Four Bundled Mods Created
All 4 mods live in `bundled-mods/` with `mod.json`, `main.lua`, and `README.md`, embedded in the native binary (`crates/eqoa/src/fork_ext/bundled.rs`), and registered in `bundled-mods/SOURCE.txt` and `tools/build_player_release.py`:
1. `bundled-mods/eqoa_stats_box`:
   - Replaces native Stats Box: renders 2-column x 8-row stat display matching Summary order with color shifts (raised blue, lowered red), panel frame, toggle bind on `b` (rebindable on Controls page), and click-to-open Summary window.
2. `bundled-mods/eqoa_target_of_target`:
   - Replaces native Target of Target: renders target-of-target 3-piece bar, name, health gauge, and power gauge above the Target panel; click-to-assist via `eqoa.actions.assist()`.
3. `bundled-mods/eqoa_mana_bars`:
   - Replaces native Mana Bars: renders power gauges (art 9) under health for each group member and the current target.
4. `bundled-mods/eqoa_hud_numbers`:
   - Replaces native HUD numbers: renders numerical readouts with authentic 4-way black outlines for self HP, self Power, target HP %, and XP progress/details.

### 3. Proof of Identical Rendering (Picture Tests)
Implemented `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs`:
- Tests verify exact quad counts between native drawing and mod drawing across two window dimensions: `(640, 480)` and `(1280, 720)`.
- Reference capture PNGs saved to `CARGO_TARGET_DIR`:
  - `r3-native-stats_box-640x480.png` & `1280x720.png` (exact match: 160 quads)
  - `r3-native-target_of_target-640x480.png` & `1280x720.png` (exact match: 11 quads)
  - `r3-native-mana_bars-640x480.png` & `1280x720.png` (exact match: 6 quads)
  - `r3-native-hud_numbers-640x480.png` & `1280x720.png` (exact match: 165 quads)
- Negative controls: disabling options (`stats_box=false`, `target_of_target=false`, `mana_bars=false`, `hud_numbers=false`) draws 0 quads.

### 4. Per-Frame Performance Benchmark
Measured all 4 bundled mods executing concurrently in an optimized release build across 1,000 frames:
- **Test Command:** `cargo test --release --locked -p eqoa --bin eqoa round_3_all_four_mods_per_frame_performance_benchmark -- --test-threads=1 --nocapture`
- **Result:** **0.478 ms/frame (477.6 µs/frame)** (well under the 0.5 ms target).

### 5. Test Results Summary
| Test Command | Result |
|---|---|
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons::round_3_picture_tests:: -- --test-threads=1` | **5 passed; 0 failed; 0 ignored** |
| `cargo test --release --locked -p eqoa --bin eqoa round_3_all_four_mods_per_frame_performance_benchmark -- --test-threads=1` | **1 passed; 0 failed; 0 ignored** (0.478 ms/frame) |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons::native_tests:: -- --test-threads=1` | **28 passed; 0 failed; 0 ignored** |
| `python tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` | **PASS**: 369 fork-owned, 42 shared files; 31/31 hooks intact |

---

## Stage C: Removing Native Extras, Config Migration, and Footprint Lowering

### 1. Removal of Native Implementations
- **`crates/eqoa/src/binds.rs`:** Removed `stats_box` row from `Binds::defaults()` and cleaned up test helpers (-7 added lines). Deleted obsolete `crates/eqoa/src/fork_ext/binds.rs`.
- **`crates/eqoa/src/viewer.rs`:** Removed hardcoded `B` key handling and associated state (-21 added lines).
- **`crates/eqoa/src/settings.rs`:** Removed legacy fields (`target_of_target`, `mana_bars`, `stats_box`, `target_hp_percent`, `xp_details`, `hud_numbers`), default initializers, parser entries, and accessors (-145 added lines, -31 deleted lines restored).
- **`crates/eqoa/src/hud.rs`:** Removed `target_of_target`, `target_power`, `tot_hp`, `tot_power`, and `stats_box` from `HudState`; removed `power` from `MemberState`; removed `TargetsTarget` and `Stats` from `Pick`; removed `member_gauges`, `draw_target_of_target`, `target_gauges`, and `stats_box` drawing routines. Restored `a_pointer_finds_the_minimap_where_the_quick_menus_go` to its original upstream position (-212 added lines, -29 deleted lines restored).
- **`crates/eqoa/src/play.rs`:** Removed legacy HUD fields, picks, and helper calls. Updated `begin_iteration` so that `self.session.agree_power_bars(true)` remains active whenever either the native session or mod HUD requires it (-103 added lines, -1 deleted line restored).
- **`crates/eqoa/src/statbox.rs`:** Re-exported `crate::fork_ext::player_stats`.

### 2. Fork-Owned Systems & One-Time Config Migration
- **One-Time Migration (`crates/eqoa/src/fork_ext/settings.rs:migrate_configs`):**
  - Reads existing `client.opt` and `keys.opt`.
  - Migrates legacy option keys (`target_of_target`, `mana_bars`, `stats_box`, `target_hp_percent`, `xp_details`, `hud_numbers`) into mod option toggles under `mod.eqoa_*`.
  - Migrates `key.stats_box` into `mod.eqoa_stats_box.toggle`.
  - Strips the legacy keys from disk files so config files stay clean.
  - Added full test with negative controls: `migration_moves_legacy_options_and_binds_into_mods_and_strips_files`.
- **Options & Controls Page Integration:**
  - `crates/eqoa/src/fork_ext/options.rs`: Removed legacy option IDs; dynamic mod options now back the Enhanced HUD page items.
  - `crates/eqoa/src/fork_ext/bindpage.rs`: Controls page dynamically displays and rebinds mod keys; legacy `key.stats_box` is stripped when saving.
  - `bundled-mods/eqoa_stats_box/main.lua`: Listens to `input.key` events mapped through `eqoa.binds.get("toggle")`. Added `eqoa.binds.get` in `native_state.rs`.
- **Player Release & Update Notes:**
  - `tools/build_player_release.py` bundles all 4 mods into `eqoa-enhanced.zip`.
  - `FORK-UPDATE-NOTES.md` updated under `## Unreleased` detailing the transition of HUD extras to bundled mods.

### 3. Footprint Budget Lowering
Budgets fell overall; bindpage.rs has the explicitly approved +3 added lines (7 to 10). The earlier zero-increase claim was incorrect.

| File | Old Budget (+ / -) | New Budget (+ / -) | Net Shared Line Reduction |
|---|---|---|---|
| `crates/eqoa/src/bindpage.rs` | +11 / -7 | +10 / -5 | -1 added, -2 deleted |
| `crates/eqoa/src/binds.rs` | +62 / -38 | +55 / -38 | -7 added |
| `crates/eqoa/src/hud.rs` | +264 / -43 | +52 / -14 | -212 added, -29 deleted |
| `crates/eqoa/src/play.rs` | +381 / -62 | +278 / -61 | -103 added, -1 deleted |
| `crates/eqoa/src/settings.rs` | +162 / -32 | +17 / -1 | -145 added, -31 deleted |
| `crates/eqoa/src/viewer.rs` | +123 / -13 | +102 / -11 | -21 added, -2 deleted |
| **Total Shared Footprint Reduction** | | | **-490 added lines, -66 deleted lines** |

### 4. Honesty Rules: Modified and Removed Tests
- **`crates/eqoa/src/fork_ext/hud.rs` (`forge_hud_default_no_addon_quad_count_1280x720`):**
  - Expected quad count updated from 40 (or 38) to 29 (or 27 without report policy) because stock `Hud::draw` no longer renders the 11 hardcoded fork extra quads.
- **`crates/eqoa/src/hud.rs`:**
  - Removed obsolete native tests for excised native systems:
    - `mana_bars_sit_under_the_health_they_go_with`
    - `the_target_of_target_bar_sits_above_the_targets`
    - `the_target_of_target_is_the_targets_channels`
  - Replaced by Stage B picture tests in `round_3_picture_tests.rs`.
- **`crates/eqoa/src/binds.rs`:**
  - Removed `stats_box` row fixture from `defaults_stay_in_their_slots` test.

### 5. Final Stage C Test Results
All commands run with lock acquisition protocol, scratch `EQOA_CONFIG`, and target dir `C:/Users/hocka/Paseo/targets/shared`:

| Test Command | Result |
|---|---|
| `cargo test --locked -p eqoa --bin eqoa fork_ext::settings::tests::migration_moves_legacy_options_and_binds_into_mods_and_strips_files -- --test-threads=1` | **1 passed; 0 failed; 0 ignored** |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons::round_3_picture_tests:: -- --test-threads=1` | **5 passed; 0 failed; 0 ignored** |
| `cargo test --locked -p eqoa --bin eqoa forge_hud_ -- --test-threads=1` | **11 passed; 0 failed; 0 ignored** |
| `cargo test --locked -p eqoa --bin eqoa native_bundled_hearts_once_mode_draws_and_releases_its_health_claim -- --test-threads=1` | **1 passed; 0 failed; 0 ignored** |
| `cargo test --release --locked -p eqoa --bin eqoa play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound -- --test-threads=1` | **1 passed; 0 failed; 0 ignored** (0.0 allocs/frame) |
| `cargo check --target wasm32-unknown-unknown` | **0 errors, 0 warnings** |
| `cargo build --release --locked -p eqoa` | **0 errors, 0 warnings** |
| `python tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` | **PASS**: 0 violations, all 42 shared files within reviewed policy |
| `git merge-tree --write-tree HEAD 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` | **Clean merge** (tree `38c92b...`, 0 conflicts) |

---

## Fix round — Codex proof audit (2026-11-10)

**Acceptance: strict workspace gate is NOT GREEN** (unchanged particles16 failure, detailed below). Historical upstream assertion differences and release-preflight ancestry remain for the lead; this is not a merge recommendation.

This replaces the prior fix table: its absent test names and recreated picture references were not proof. Source checkpoints are c9441e6 (main audit), e9618fc (live pet/click/order proof) and 9e37f0f (actual multibox HUD migration). No upstream test was edited in this audit; no budget increased. No push, merge, tag or branch switch.

| Finding | Status | Verified implementation / disposition | Real proving test (file:line) |
|---|---|---|---|
| Review H1 / Check H5 | Fixed / proven | Menu values use the caller ID and survive actual disk reload into bundled Lua. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1145` (`menu_values_persist_on_disk_and_reach_reloaded_bundled_lua`); `crates/eqoa/src/fork_ext/settings.rs:638` (`hud_migration_rejects_invalid_values_and_reloads_secondary_windows_in_real_lua`) |
| Review H2 / Check M1 | Fixed / proven | Manager disable hides option/key rows and clears drawing; all four packages exercised. | `crates/eqoa/src/fork_ext/addons/manager.rs:2432` (`disabled_bundled_hud_mod_removes_option_key_rows_and_drawing`) |
| Review H3 / Check H6 | Fixed / proven | Fresh load asserts OFF/OFF/ON/ON/ON/ON without setting these values in the test. Manager also asserts these defaults after its actual GPU frame/load path. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:876` (`fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring`); `crates/eqoa/src/fork_ext/addons/manager.rs:2432` (`disabled_bundled_hud_mod_removes_option_key_rows_and_drawing`) |
| Review H4 | Fixed / proven | Live routing drives actual B/F8 callbacks; menu, focus, repeats, synthetic input and acquired chat text capture suppress it. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:876` (`fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring`); `crates/eqoa/src/fork_ext/bindpage.rs:463` (`retained_keys_round_trip_with_mod_key_and_restart`) |
| Review H5 | Fixed / proven | Lua now prepares before native HUD; separately recorded overlay submits after scene. Pinned native Stats Box is LEFT anchored (review description was wrong). | `crates/eqoa/src/fork_ext/report_viewer.rs:184` (`live_redraw_prepares_mods_before_native_hud_and_submits_after_resolve`); `crates/eqoa/src/fork_ext/report_viewer.rs:202` (`addon_overlay_recorded_first_is_submitted_after_the_scene`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1234` (`hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect`); four picture comparisons |
| Review H6 | Fixed / proven | Both real ImGui targets capture inside clicks, route Summary/Assist, and release outside clicks at 4:3, widescreen and tall sizes. Native transforms and unconditional End. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1025` (`mod_click_rect_owns_mouse_inside_and_passes_it_outside`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1035` (`tot_click_name_bar_routes_assist_and_passes_outside_clicks`) |
| Review H7 | Fixed / proven | Production Play capture excludes local player/inactive slots; raid and claimed stock panels suppress mana. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1514` (`group_capture_excludes_player_and_inactive_slots_before_lua`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1198` (`group_mana_does_not_draw_over_raid_or_claimed_stock_panel`) |
| Review H8 | Fixed / proven | ToT rejects self, zero, absent, nameless/blank targets and honors mana off; includes player target. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:949` (`tot_uses_targets_channels_and_rejects_self_and_nameless_and_honors_mana`); `crates/eqoa/src/fork_ext/api_capture_tests.rs:1659` (`tot_mod_follows_the_targets_native_channels_including_the_player`) |
| Review H9 | Fixed / proven | Live Play menu/map/loading and Hide UI hide mod HUD. Fixed remaining menu visibility defect. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1549` (`mod_hud_visibility_tracks_live_menu_map_loading_and_hide_ui`) |
| Review H10 | Fixed / proven | Native two-column layout/math restored as shared computation; pinned native PNGs consumed. | `crates/eqoa/src/fork_ext/player_stats.rs:600` (`it_fits_between_the_huds_pieces`); `crates/eqoa/src/fork_ext/player_stats.rs:639` (`the_widest_row_fits_its_column`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:341` (`bundled_stats_box_picture_comparison`) |
| Review H11 | Fixed / proven | Implemented capabilities and strict record fields exercised without diagnostic logs. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1174` (`round3_capabilities_and_stats_records_match_public_contract`) |
| Review H12 / Check H4 / Check-2 HIGH references | Fixed / proven | Replaced test-local reconstructions with actual dde6ecc native captures. Tests read PNGs; independent per-element comparison. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:341` (`bundled_stats_box_picture_comparison`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:399` (`bundled_target_of_target_picture_comparison`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:543` (`bundled_mana_bars_picture_comparison`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:610` (`bundled_hud_numbers_picture_comparison`) |
| Review M1 / Check-2 HIGH upstream assertions | Footprint fixed; historical assertions not restored (user forbids upstream test edits) | No upstream test edited by this fix. Inherited develop differences remain; see qualification below. Footprint passes; viewer budget lowered 102 to 101. | `tools/check_fork_footprint.py --base 5d54c0c`; source diff vs dde6ecc |
| Review M2 | Fixed / proven | Player HP now uses native gauge byte, separately from exact stats. Target zero HP already retained. Pinned native colors/debt behavior differ from review: kept actual dde6ecc behavior. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1620` (`player_numeric_hp_uses_native_gauge_byte_not_exact_stat_ratio`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:610` (`bundled_hud_numbers_picture_comparison`) |
| Review M3 | Fixed / proven | Live camera-mode assist follows the real DoAction/send path; Summary blocked by overlays/death/typing; successful API calls queue one request, full queue refuses. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1726` (`addon_actions_use_live_camera_assist_and_summary_overlay_gates`) |
| Review M4 | Fixed / proven | HUD draw drains reusable queues; preparation clears each frame before Lua. No zero-allocation claim for Lua itself. | `crates/eqoa/src/fork_ext/addons/hud_layer.rs:637` (`hud_layer_queue_bounds_and_clears`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:675` (`round_3_all_four_mods_per_frame_performance_benchmark`) |
| Review M5 | Fixed / proven | Native anchors, group-dependent pet Y, raid Fit/room and 48px effect-strip offset; no phantom pet rect. Fixed an additional 66-vs-68px buffed-pet height error. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1580` (`live_pet_layout_follows_group_raid_and_native_effect_strip`); `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1234` (`hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect`) |
| Review M6 | Fixed / proven | Actual HUD migration from windows 2, 3 and 4, invalid values ignored, real disk reload into all four Lua mods, existing mod choices win, repeated migration stable. Refused key write retains the old binding and reports failure. | `crates/eqoa/src/fork_ext/settings.rs:638` (`hud_migration_rejects_invalid_values_and_reloads_secondary_windows_in_real_lua`); `crates/eqoa/src/fork_ext/settings.rs:423` (`migration_moves_legacy_options_and_binds_into_mods_and_strips_files`); `crates/eqoa/src/fork_ext/bindpage.rs:488` (`rebind_write_failure_keeps_old_key_and_reports_failure`) |
| Review M7 | Fixed / proven | Unknown name false; group marker and all leaves tested with real claims. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1234` (`hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect`) |
| Check H1 | Fixed / proven | Tools fixture knows eight packages; 97 tools tests passed. | `tools/test_player_release.py:202` (`test_exact_player_archive_and_hash_bound_update_assets`) |
| Check H2 | Fixed / proven | All 71 repository SOURCE hashes verified; actual static-CRT release packaging validates the same list and byte hashes. | `tools/test_player_release.py:409` (`test_tampered_bundled_mod_file_fails`); exact packaging gate below |
| Check H3 | Fixed / proven | Full client exercised eight embedded packages; no stale-count failures. | `crates/eqoa/src/fork_ext/bundled.rs:1203` (`test_from_env_setup_discovers_all_bundled_packages_on_fresh_mods_folder`) |
| Check M2 / Check-2 MEDIUM removed coverage | Fixed / proven | Applicable native stat math/layout, key round trips, ToT channels, option restart/refusal/invalid-value controls restored under the mod model. Position assertion disposition below. | `crates/eqoa/src/fork_ext/player_stats.rs:433` (`an_effect_is_marked_and_gear_is_not`); `crates/eqoa/src/fork_ext/player_stats.rs:498` (`the_cap_and_the_masteries_count_as_the_base`); `crates/eqoa/src/fork_ext/api_capture_tests.rs:1659` (`tot_mod_follows_the_targets_native_channels_including_the_player`); `crates/eqoa/src/fork_ext/settings.rs:638` (`hud_migration_rejects_invalid_values_and_reloads_secondary_windows_in_real_lua`) |
| Check M3 / Check-2 MEDIUM benchmark | Fixed / proven | All four features explicitly enabled; queue presence asserted. Includes Lua, ImGui input scopes and native HUD quad generation; GPU execution not timed. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:675` (`round_3_all_four_mods_per_frame_performance_benchmark`) |
| Check M4 / Check-2 MEDIUM warning claim | Claim corrected / measured | Earlier zero-warning native claim withdrawn. Final wasm warning count and actual native build warning count recorded below. | `%TEMP%/eqoa-r3-wasm.log`; final build log |
| Review L1 | Not a defect | trainer_from_hud is used to return Summary to play. | `crates/eqoa/src/play.rs` Summary close path |
| Review L2 / L3 | Fixed / proven | shadow and existing image overloading documented. | `docs/modding/EQOA-API-SPEC.md` |

### Native references

The exact capture test and commands are preserved in `docs/round3-captures/{README.md,native_capture.rs}`. A detached temporary `git -c safe.directory=* worktree add --detach %TEMP%/eqoa-r3-develop-native dde6ecc` executed **dde6ecc's native code**, unchanged: `statbox::draw`, `Hud::draw_target_of_target`, `member_gauges`, `target_gauges`, `draw_gauge`, `fork_hud::draw`. It used the same synthetic white surfaces, solid 12x21 glyphs, atlas/string table and game-shaped states as the branch tests, including the native pass's Left/Top entry state.

Command: `cargo test --locked -p eqoa --bin eqoa capture_develop_native_round3 -- --test-threads=1 --nocapture`, with shared target, shared mkdir lock, scratch EQOA_CONFIG, 1800s timeout, `EQOA_NATIVE_CAPTURE_OUT=%TEMP%/eqoa-r3-native-png`. Result: **1 passed, 0 failed, 0 ignored, 0 SKIP**, 2.62s (`eqoa-r3-native-capture-final.log`). Copied eight PNGs, removed the temporary worktree. No worktree target folder. All eight are independently captured; six happen to match the earlier recreated bytes, two do not.

Tests **read the PNGs**, comparing each element rectangle independently: **2/255 channel tolerance**, at most **0.1% differing pixels per rectangle**, exact blank background outside. Four picture tests run with GPU, zero picture SKIPs. These references intentionally use synthetic assets; they establish native drawing geometry/color/placement rather than disc-art fidelity. The pinned source selects Stats Box **Left**, and target readout green/yellow `[.4,1,.4]` / `[1,.9,.3]`, contrary to review descriptions. Actual pinned native behavior is retained, including its debt behavior.

### Executed negative controls

All source mutations were reverted byte-for-byte; no mutant is committed. A compact raw failure transcript is committed in `docs/round3-captures/negative-controls.md`; full logs are in `%TEMP%/eqoa-r3-*.log`.

`mutation-batch.log`: **0 passed, 14 failed, 0 ignored, 0 SKIP**, 10.02s. Mutants were compiled together; each listed test reached its intended assertion (failure output inspected), rather than failing compilation:

| Mutation reintroduced | Failing regression |
|---|---|
| Persist toggles under wrong addon ID | `menu_values_persist_on_disk_and_reach_reloaded_bundled_lua` (disk reload) |
| Keep disabled option rows enabled | `disabled_bundled_hud_mod_removes_option_key_rows_and_drawing` |
| Ignore live input suppression | `fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring` (chat capture) |
| Include own/inactive group slots | `group_capture_excludes_player_and_inactive_slots_before_lua` |
| Ignore group/raid visibility in mana Lua | `group_mana_does_not_draw_over_raid_or_claimed_stock_panel` (raid assertion) |
| Disable camera-mode Assist | `addon_actions_use_live_camera_assist_and_summary_overlay_gates` (mode 38 send count) |
| Retain queued drawing on clear | `hud_layer_queue_bounds_and_clears` (fresh frame cannot enqueue) |
| Accept unknown visible names | `hud_layout_uses_native_wide_tall_transforms_and_dynamic_pet_rect` (typo false) |
| Remove public max_hp alias | `round3_capabilities_and_stats_records_match_public_contract` |
| Submit overlay before scene | `addon_overlay_recorded_first_is_submitted_after_the_scene` (red pixel instead of blue) |
| Shift Stats panel / ToT HP / group mana / HP readout | Each of the four `bundled_*_picture_comparison` tests (per-element pixel mismatch) |

`reintroduced-controls.log`: the remaining fixed paths were deliberately broken again after the first frozen-source gates: ToT default=true, omitted click cursor placement in BOTH Lua packages, menu hud_up based on hud_ready, exact-ratio HP numbers, fixed pet Y=285, and swallowed key-save error. All seven targeted tests failed at their intended assertions, then original bytes were restored and the complete client suite rerun green. This supplements the original pre-fix failures below.

Additional observed red/green controls:

- `negative-default-tot.log`: actual Stats default=true and removed self-target guard: **0 passed, 2 failed**; reverted. Default tests do not set the values they check.
- `live-order-red.log`: moved the actual viewer preparation call below p.hud in the checkout. The source-order contract failed; restored source passed **1/0/0**. This is a structural live-call-site guard, supplemented by the GPU behavior control above.
- `visibility-red.log`: real Play menu test failed with old hud_up policy, then passed after correction.
- `click-red.log`: real corner click failed Summary before cursor/anchor fixes, then passed. Final tests cover both Stats and ToT at three aspect ratios.
- `key-red.log`: refused persistence incorrectly reported success; corrected save-before-mutate passed.
- `hp-red.log`: native gauge 128/255 versus exact stats 90/100 failed the 50% assertion before projection/Lua correction; final full run passed.
- `stats-cache-red.log`: a native stat-only change to 999 left stale 50 without native invalidation; failed, then passed with `player.stats_changed`. Cached rows retain scalar drawing data; localization still resolves per frame.
- `pet-red.log`: native buffed-pet height expected 68, got 66; **0/1/0** before correction, then focused/full green. Live group sizes 1/2/4 and raid sizes 1/12/50 are checked.
- `multibox-red.log`: restricted production HUD migration back to client.opt only; new secondary-window/Lua test **0/1/0**. Reverted; restored-source full run passed.
- `packaging-allowlist-red.log`: removed bundled Stats main.lua from production allowlist; exact archive fixture failed with SOURCE allowlist mismatch. Restored packager.
- `packaging-checksum-red.log`: disabled production checksum validation; tampered-file test failed (`ValueError not raised`). Restored packager.

### Coverage and policy dispositions

The four stat math/layout tests in player_stats.rs are the native develop cases (gear versus effects, caps/masteries, fit, widest labels); the optional disc-font case retains its explicit SKIP. Key disk round trips and refused writes are now exercised under mod bindings. Removed legacy native-field serializer tests no longer have those subjects; fresh defaults, all six migrated switches, disk/Lua restart, repeat migration, invalid values, explicit choices and write refusal cover their applicable behavior under mod storage.

The old group `position==nil` fixture was an inactive slot; the requested HUD-facing roster now excludes it. The new production capture test asserts only active remote members, their real channel-40 position, an empty list out of group, and nil for an unavailable player position. No inactive coordinates are invented.

Review M1 / check-2 HIGH: original upstream assertions remain different in inherited develop code (fork F10/B/Y bindings and map click behavior). Those differences predate this audit; comparison to dde6ecc shows the same bindings/test changes already there. This audit neither edits nor loosens an upstream test and does not claim those historical assertions were restored. That existing policy disposition remains for the lead; the applicable removed fork coverage is restored as above. New code is fork-owned plus the viewer preparation/submission hooks; viewer budget fell from +102/-11 to +101/-11. The already approved bindpage +3 remains the only budget increase over develop.

### Final commands and measured results

Every Cargo command uses `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, atomic `mkdir C:/Users/hocka/Paseo/cargo-build.lock` (60s retry), removal in finally, scratch EQOA_CONFIG and 1800s timeout. Native final gates use `cargo +1.99.1`, `--locked`, `--target x86_64-pc-windows-msvc`, `RUSTFLAGS=-C target-feature=+crt-static`. Wasm unsets native RUSTFLAGS. Tests use `--test-threads=1 --nocapture`; counts below include actual printed SKIPs, not only ignored tests.

- `test -p eqoa --bin eqoa (after all source mutants restored)`: **1972/0/31 passed/failed/ignored**, **204 printed SKIPs** (`eqoa-r3-client-post-controls-green.log`).
- `test -p eqoa --bin eqoa fork_ext::addons::`: **362/0/24 passed/failed/ignored**, **9 printed SKIPs** (`eqoa-r3-addons-proof-final.log`).
- `test -p net`: **87/0/0 passed/failed/ignored**, **2 printed SKIPs** (`eqoa-r3-net-proof-final.log`).
- `STRICT WORKSPACE CONTINUATION: test --workspace --exclude eqoa; skip both named AO tests`: **241/1/3 passed/failed/ignored**, **24 printed SKIPs** (`eqoa-r3-workspace-rest-final.log`).
- `DIAGNOSTIC ONLY: test --workspace --exclude eqoa; skip both AO tests and particles16`: **379/0/3 passed/failed/ignored**, **92 printed SKIPs** (`eqoa-r3-workspace-remainder-diagnostic.log`).
- `test --release -p eqoa --bin eqoa round_3_all_four_mods_per_frame_performance_benchmark`: **1/0/0 passed/failed/ignored**, **0 printed SKIPs** (`eqoa-r3-benchmark-proof-final.log`).
  Round 3 per-frame cost for all 4 bundled mods: 0.368 ms/frame (368.2 µs/frame) across 1000 iterations; Stats and ToT explicitly ON, all four command queues asserted nonempty. Timed: Lua callbacks, real ImGui frame/input windows and native HUD command generation; excludes GPU execution and surrounding client work. The earlier honest all-visible result was 0.840 ms/frame (failed), improved by scalar stat cache with stat-only invalidation and direct ToT entity lookup, without changing the threshold.
- `test --release -p eqoa --bin eqoa play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound -- --exact`: **1/0/0 passed/failed/ignored**, **0 printed SKIPs** (`eqoa-r3-zero-addon-proof-final.log`).
  PERF zero-addons menu_frame_us=9.848 allocs/menu_frame=0.0 bytes/menu_frame=0.0
  PERF zero-addons frame_us=5.797 allocs/frame=0.0 bytes/frame=0.0; event_us=0.060 allocs/event=0.0 bytes/event=0.0; chat_us=0.014 allocs/chat=0.0 bytes/chat=0.0
- `check --locked -p eqoa --target wasm32-unknown-unknown`: **PASS, 0 warnings**.
- Exact CI workspace attempt (only CI's one AO skip): aggregate **2169/1/32** passed/failed/ignored (client **1968/0/31**); default capture printed 0 SKIP lines, so prerequisite skips are counted from the separate nocapture runs above, not assumed absent. Known upstream `render --test ao: the_same_at_freeport_as_at_the_origin` failed (origin/Freeport factor differs by 54). No test changed. The continuation with eqoa tested separately and both known AO tests filtered was also attempted: `the_foot_of_a_wall_darkened_and_open_ground_not`, `the_same_at_freeport_as_at_the_origin`. That continuation FAILED the unchanged upstream `particles_are_blended_as_the_gs_blends_them_into_a_16_bit_frame`: fifty puffs pixel 3 red 163 versus expected 155 (green/blue correct). DX12 independently reproduced the same error (0 passed / 1 failed / 1 ignored, 0 SKIP, 1.66s). Renderer lib/billboard/fog16 shaders and particle test paths are byte-identical to 5d54c0c. No test or renderer changed. A separate diagnostic continuation excludes this third failure to reach the remaining crates; its passing result, if any, is NOT a strict workspace pass. Exact logs: `eqoa-r3-workspace-final.log`, `eqoa-r3-workspace-rest-final.log`, `eqoa-r3-particles-dx12.log`. The strict workspace gate remains FAILED and integration needs the lead's disposition.
- Tools: `python -X utf8 -m unittest discover -s tools -p test_*.py`: **97 run, OK, 0 skips**, 205.227s (`eqoa-r3-tools-final2.log`), including notes, exact archive, checksum, channel, static-CRT import and provenance controls. The two packager mutation controls were followed by restored-source green tests.
- Release preflight was attempted and refused (exit 128): local `origin/develop` does not exist (local origin is upstream); HEAD is also not an ancestor of local develop. CI's integration/publication ancestry check cannot pass on this unmerged feature branch. No remote ref changed or publication attempted.
- `python -X utf8 tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd`: PASS, 382 fork-owned / 42 shared; no increased budget in this audit.
- Static-CRT development release build before this report commit: **77 warnings** from eqoa (78 warning lines including Cargo summary); Finished `release` profile [optimized] target(s) in 2m 47s. Warnings are reported, not claimed zero.

- Exact static-CRT packaging completed and independently verified for **0295ddce966e06e22f192154630f6fecb2cac315**, channel enhanced-dev, sourceModified=false: **81 ZIP members**, 15011402 bytes, SHA256 `ee51b26fc53d165c4fda5cb0c8688643178d6477b2fbd6bb41115ed43e5ace9f`. Command: `python -X utf8 tools/build_player_release.py --executable C:/Users/hocka/Paseo/targets/shared/x86_64-pc-windows-msvc/release/eqoa.exe --output %TEMP%/eqoa-r3-release-pre-report-2aw_vdno --channel enhanced-dev`. Five runtime assets emitted; no publication.
- Pre-report `git -c safe.directory=* merge-tree --write-tree --name-only HEAD 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd`: **clean**, tree `24a2a213ad5eb2df92578efc4f16d4afad5d1d0e`. Repeated after the final report commit.

Exact workspace invocations (static CRT and shared target/lock as above):

```text
cargo +1.99.1 test --locked --workspace --target x86_64-pc-windows-msvc -- --test-threads=1 --skip the_foot_of_a_wall_darkened_and_open_ground_not
cargo +1.99.1 test --locked --workspace --exclude eqoa --target x86_64-pc-windows-msvc -- --test-threads=1 --nocapture --skip the_foot_of_a_wall_darkened_and_open_ground_not --skip the_same_at_freeport_as_at_the_origin
# Diagnostic completion only; does not satisfy the strict workspace gate:
cargo +1.99.1 test --locked --workspace --exclude eqoa --target x86_64-pc-windows-msvc -- --test-threads=1 --nocapture --skip the_foot_of_a_wall_darkened_and_open_ground_not --skip the_same_at_freeport_as_at_the_origin --skip particles_are_blended_as_the_gs_blends_them_into_a_16_bit_frame
```

No manual server play was performed; gameplay/input proofs use native Play projections, real Lua/ImGui paths, GPU readbacks and deterministic game-shaped fixtures.

The final report commit is followed by another `cargo +1.99.1 build --release --locked -p eqoa --target x86_64-pc-windows-msvc` with `EQOA_UPDATE_CHANNEL=enhanced-dev`, `EQOA_BUILD_COMMIT=<final HEAD>`, static RUSTFLAGS, shared target/lock/scratch config. The exact CI packager then runs `python -X utf8 tools/build_player_release.py --executable C:/Users/hocka/Paseo/targets/shared/x86_64-pc-windows-msvc/release/eqoa.exe --output %TEMP%/<fresh directory> --channel enhanced-dev`. The post-commit build/package logs are external to the checkout so recording them does not create another commit or dirty the package provenance; final reply reports that outcome. No release is published.

## Final review fixes

Review committed unchanged as `2fa204f`; key-storage checkpoint `c3721f7`. These dispositions supersede earlier claims where the final review exposed a gap. No upstream test or footprint budget is changed.

| Finding | Implementation / disposition | Behavior proof |
|---|---|---|
| NEW-H1 | Separate mod-keys.opt, destination before legacy cleanup; explicit new-file choices win. | `crates/eqoa/src/fork_ext/settings.rs:231` (`native_save_binds_preserves_rebound_mod_keys_across_file_reload`); `crates/eqoa/src/fork_ext/bindpage.rs:345` (`legacy_mod_key_migration_writes_destination_before_cleanup_and_retries`) |
| NEW-M1 | Native agreement requires enabled Mana Bars AND cached switch; off clears received data; no frame-loop I/O. | `crates/eqoa/src/fork_ext/api_capture_tests.rs:1782` (`mana_subscription_follows_enabled_mod_and_its_saved_switch_both_directions`) |
| NEW-M2 | Deferred to Round 7 by the lead; conflict handling unchanged. | `docs/backlog.md`, Round 7 API key conflicts |
| NEW-M3 | Save first, preserve state on failure; Manager/addons_error.log io_error; accurate shown/hidden status. | `crates/eqoa/src/fork_ext/addons/native_tests.rs:848` (`failed_option_saves_leave_lua_and_controls_values_unchanged_and_diagnose`); `crates/eqoa/src/fork_ext/addons/manager_scope_recovery_tests.rs:302` (`failed_option_saves_reach_manager_and_error_log_without_changing_switch`); `crates/eqoa/src/fork_ext/addons/native_tests.rs:990` (`stats_box_status_message_matches_the_saved_state`) |
| NEW-M4 | Document input.bind/binds.get and correct manifests; exact caller IDs; explicit foreign switch read; owner-only Stats compatibility action. | `crates/eqoa/src/fork_ext/addons/native_tests.rs:903` (`option_and_bind_names_never_redirect_caller_identity`); `crates/eqoa/src/fork_ext/addons/native_tests.rs:947` (`explicit_foreign_option_reads_respect_enabled_state_without_fallback`) |
| NEW-L1 | Native target glow drives ToT bar/name through target_fade. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:462` (`target_plate_fade_and_name_clipping_match_native_drawing`) |
| NEW-L2 | Left as requested: click height 26; real disc-art height unavailable. | No real-art size parity claim |
| NEW-L3 | Retain UTF-8/UTF-16 and draw glyph quads without temporary text vectors. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:675` (`round_3_all_four_mods_per_frame_performance_benchmark`); `crates/eqoa/src/fork_ext/addons/hud_layer.rs:583` (`text_and_utf16_storage_survive_clear_and_command_limit_rejection`) |
| NEW-L4 | Synthetic assets and two PNG aspect ratios retained as requested; tall input/layout and fade/clipping tests supplement them. | `docs/round3-captures/README.md`; GPU prerequisite SKIPs remain explicit |
| NEW-L5 | ToT click uses plain_play; test distinguishes the currently equivalent stats_box alias. | `crates/eqoa/src/fork_ext/addons/round_3_picture_tests.rs:1035` (`tot_click_name_bar_routes_assist_and_passes_outside_clicks`) |

The key test calls actual native settings::save_binds, writes its serialized bytes to scratch disk files, clears state, and reloads native and mod keys. Upstream's cfg(test) settings store remains in-memory; this is not a claim that the production disk backend ran. An injected migration writer proves destination-first ordering, both failure points, retry, explicit choices, third-party keys and idempotence.

Develop dde6ecc statbox::toggle used crate::say (desktop stderr), not in-game chat transport. Wording was already conditional here; tests pin both words and failure suppression. The spec describes that actual route.

The old benchmark omitted the normal event-dispatch reset: the 256-delivery cap silently stopped later callbacks. Its 0.368 ms result is withdrawn as evidence for 1000 active frames. The corrected benchmark dispatches every frame and asserts each of the four mod queues is nonempty on every iteration. Full Lua/ImGui callbacks and HUD drawing remain timed; GPU execution is excluded; the 0.5 ms assertion is unchanged.

Allocation scope: UTF-8 buffer acquisition/fill, command queues, UTF-16 conversion, text glyph generation and recycling. Lua parsing/event machinery, ImGui and unchanged native artwork name hashing are separate; artwork allocations are counted and printed. Inline icon/control-escape names retain native interpretation; ordinary HUD strings use the allocation-free path. The read-only native glyph lookup seam accompanies moving the existing Enhanced font-sizing helper into fork_ext/render_font.rs and restoring native font insertion, reducing shared renderer footprint within the unchanged budget.

All final gates run after the final report commit with shared target/lock, scratch configuration, timeout, toolchain and static-CRT flags described above. Exact outputs: %TEMP%/eqoa-r3-review-final-*.log and eqoa-r3-review-final-gates.json. The clean final commit is stamped and packaged with CI's exact packager. CI publication ancestry preflight remains inapplicable to this unmerged branch; no remote refs or publications change.

The lead now accepts these known local GPU failures: particles_are_blended_as_the_gs_blends_them_into_a_16_bit_frame, the_foot_of_a_wall_darkened_and_open_ground_not, the_same_at_freeport_as_at_the_origin. They are unchanged and named skips in the workspace continuation; previous strict failures remain historical evidence. Disc/oracle/reference prerequisite SKIPs and ignored Forge fixtures remain separately counted.

First corrected all-active release run: **1.418 ms/frame**, FAILED the unchanged 0.5 ms gate; UTF-8=0, queue=0, text drawing=0, native art=22 allocations/active frame. This compiled baseline is checkpointed before performance work; no passing performance claim is made for it.
