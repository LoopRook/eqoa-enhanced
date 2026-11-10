# Round 3 implementation report: the HUD extras become mods

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
