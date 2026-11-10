# Round 3 implementation report: the HUD extras become mods

Branch: `feat/eqoa-api-r3`, from `develop`. Implementation follows `docs/plans/eqoa-api/round-3-brief.md` in stages A, B, C.

---

## Stage A: API additions

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

4. **Mod Key Bindings & B-Key Verification (A4):**
   - Bound `eqoa.input.bind({ id, label, default_key }, callback)` in `native_state.rs`.
   - **B-Key Verification:** Verified in `viewer.rs:4346` that today's B key check is hardcoded:
     `(None, Key::Character(c)) if (p.mode == 0 || p.mode == 38) && c.eq_ignore_ascii_case("b") && !event.repeat => { crate::statbox::toggle(); return; }`.
     The native client never reads the `stats_box` bind from `keys.opt` or `binds.rs` during gameplay; B is hardcoded to toggle the stats box.
   - **Controls Page Footprint Barrier:** See "Questions for the lead" below.

5. **Mod Option Toggles on Enhanced HUD Page (A5):**
   - Created `crates/eqoa/src/fork_ext/addons/options.rs` with `register_toggle`, `get_toggle`, `set_toggle`, `toggle_by_item`, and `value_by_item`.
   - Bound `eqoa.options.toggle({ id, label, default })` and `eqoa.options.get(id)` in `native_state.rs`.
   - Integrated dynamic mod toggles into `crates/eqoa/src/fork_ext/options.rs` on the "Enhanced HUD" options page.
   - Negative control verified: invalid argument types return `invalid_argument`.

6. **HUD Placement & Font API (A6):**
   - Created `crates/eqoa/src/fork_ext/addons/hud_layout.rs` computing frame metrics, 4:3 pillarbox/letterbox offsets, side/top margins, and screen-space coordinates.
   - Bound `eqoa.hud.frame(opt)` returning `{ width, height, offset_x, offset_y, scale_x, scale_y, side, top, window_width, window_height }`.
   - Bound `eqoa.hud.layout(opt)` returning exact layout rects for `player` (panel, health, power, exp), `group_members` (panel, health, power), `target` (panel, health, power), `target_of_target` (panel, health, power), `pet` (panel), and `stats_box` (panel).
   - Bound `eqoa.hud.visible(element)` honoring `hideui::hidden()`, modal dialogs/menus, loading screens, and active mod suppression claims from `effective_visibility()`.
   - Bound `eqoa.fonts.game()` returning `{ name = "ArialUI", height = 21, scale = 0.6 }`.
   - Negative control verified: `eqoa.hud.visible()` returns `false` when outside world or when Hide UI is active.

---

## Verification & Test Results

All tests run using `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared` with lock acquisition protocol and scratch `EQOA_CONFIG`.

| Test Command | Result |
|---|---|
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons::native_tests:: -- --test-threads=1` | **26 passed; 0 failed; 0 ignored; 0 SKIP** (1952 filtered out) |
| `cargo test --locked -p eqoa --bin eqoa player_stats::tests:: -- --test-threads=1` | **4 passed; 0 failed; 0 ignored; 0 SKIP** (115 filtered out) |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` | **82 passed; 0 failed; 0 ignored; 0 SKIP** (37 filtered out) |
| `python tools/check_fork_footprint.py --base 5d54c0c859f6956c27bb71a1ffb0bfddbd31fddd` | **PASS**: 354 fork-owned, 42 shared files; 31/31 hooks intact; all files within reviewed policy. |

---

## Questions for the lead

### A4: Controls Page Mod Key Rebinds vs. Footprint Budget

The brief states:
> "4. `eqoa.input.bind{id, label, default_key}`: a mod-owned key shown and rebindable on the Controls page, saved in the binds file under a mod namespace, firing an event/callback. Verify first whether today's B key actually honours a rebind (the scout saw it hardcoded in viewer.rs)."
> "If something in the brief can't be done as written, or you would need to add a drawing helper other than those in A6, or raise any footprint budget, STOP and write the question in the report (section "Questions for the lead"), commit, and end your turn with the question."

**Findings from code inspection:**
1. **B Key is hardcoded today:** In `viewer.rs:4346`:
   ```rust
   (None, Key::Character(c)) if (p.mode == 0 || p.mode == 38) && c.eq_ignore_ascii_case("b") && !event.repeat => { crate::statbox::toggle(); return; }
   ```
   Today's B key directly matches character `"b"`. It does not check `keys.opt` or any rebound key in `binds.rs`. Rebinding `stats_box` on the Controls page currently has zero effect on gameplay.
2. **Footprint budget barrier on Controls page:**
   - In `docs/fork-footprint.json`, `crates/eqoa/src/bindpage.rs` has budget `{ "additions": 7, "deletions": 7 }`. Current diff against upstream is `+5, -4` (headroom of +2 additions, all in test code; production code has 0 additions).
   - In `bindpage.rs`, `ROWS` is a static slice from `binds.rs` referenced by `entries()`, `defaults()`, `bindable()`, `draw()`, `label()`, `note()`, etc.
   - `crates/eqoa/src/binds.rs` has budget `{ "additions": 62, "deletions": 38 }`, currently at exactly `+62, -38` (headroom of +0 additions).
   - To make mod-owned keys dynamically appear and be rebindable on the Controls page, `bindpage.rs` would need substantial changes across its row indexing and rendering functions. Doing this within the remaining +2 lines in `bindpage.rs` without raising the footprint budget is structurally impossible.

**Question for lead:**
How should mod key bindings be configured?
1. Configure mod keybindings through mod options / settings on the Enhanced HUD page or Addon Manager (zero footprint on `bindpage.rs`).
2. Request owner approval to raise the footprint budget for `crates/eqoa/src/bindpage.rs` so a hook can route dynamic mod binds to the Controls page.
3. Keep default key `b` for `eqoa_stats_box` handled through the addon input event handler / viewer hook.
