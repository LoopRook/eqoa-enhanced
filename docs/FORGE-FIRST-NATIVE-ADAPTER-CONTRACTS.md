# FrontiersForge First Native Adapter Contracts: Util, Player, and UI

This document specifies the audited source contract for the first adapter slice (`frontiers_forge.util`, `frontiers_forge.player`, `frontiers_forge.ui`). It corrects prior attribution errors, separates static source findings from implemented runtime code, documents explicit unverified/unresolved APIs, details the missing ImGui host gates for `retro_health_hearts.lua`, and outlines the proposed multi-owner HUD suppression architecture.

---

## 1. Upstream Reference Baseline & Verified Local Paths

* **FrontiersForge (pinned):** [mmvest/FrontiersForge@4026d5c](https://github.com/mmvest/FrontiersForge/tree/4026d5cebcede24943ece8dc34833e584f46cd90)
* **UiForge (submodule):** [mmvest/UiForge@6f2d125](https://github.com/mmvest/User-Interface-Forge/tree/6f2d125cc686cbc882f8098a939cc03cc23a59a1)
* **Local Reference Tree:** `C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference`
* **Native Worktree:** `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`

---

## 2. Implementation Status Classification

| Status Level | Definition |
| :--- | :--- |
| **Implemented; fixture-tested** | Implemented in the host and exercised by bounded host/GPU fixtures. Live game-state, input and DPI validation remain pending. |
| **Statically Verified** | Native source field/function identified and proven by code snippet; adapter binding not yet implemented in Lua bridge. |
| **Unresolved / Absent** | No direct equivalent field or function exists in the native client codebase. Requires engine design decision or new plumbing. |
| **Prohibited** | Incompatible with native memory safety or engine architecture (e.g. emulated guest memory pointers). |

---

## 3. First Slice: Statically Verified Contracts (`retro_health_hearts`, `screen_grid`)

These 5 APIs constitute the narrow first implementation slice directly required by `screen_grid.lua` and `retro_health_hearts.lua`.

### 3.1 Contract Table

| Module & Function | Exact Call Site | Lua Signature & Value Conventions | Native Field / Source Definition | File & Line Reference | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `util.IsInGame()` | `screen_grid.lua:124`<br>`retro_health_hearts.lua:165`<br>`ff_example.lua:52` | `() -> integer`<br>Returns numeric `1` (in-game) or `0` (out). **Not boolean.** | `self.play.as_ref().is_some_and(\|p\| p.session.self_id.is_some() && p.hud_ready())` | [`crates/eqoa/src/viewer.rs:5264`](../crates/eqoa/src/viewer.rs#L5264)<br>[`crates/eqoa/src/fork_ext/addons/host.rs:52-54`](../crates/eqoa/src/fork_ext/addons/host.rs#L52-L54) | **Implemented; fixture-tested** |
| `util.IsStartMenuOpen()` | `screen_grid.lua:124`<br>`retro_health_hearts.lua:165`<br>`ff_example.lua:233` | `() -> integer`<br>Returns numeric `1` (open) or `0` (closed). **Not boolean.** | `self.play.as_ref().is_some_and(\|p\| p.start.as_ref().is_some_and(\|s\| s.menu.visible))` | [`crates/eqoa/src/viewer.rs:5265`](../crates/eqoa/src/viewer.rs#L5265)<br>[`crates/eqoa/src/fork_ext/addons/host.rs:56-59`](../crates/eqoa/src/fork_ext/addons/host.rs#L56-L59) | **Implemented; fixture-tested** |
| `player.GetCurrentHp()` | `retro_health_hearts.lua:196`<br>`ff_example.lua:413` | `() -> integer`<br>Current real HP (e.g. `1`..`5000+`). Proposed absent: `0`. | `pub stat: [i32; 29]` (index `7` is HP)<br>`self.stats.stat[7]` | [`crates/net/src/dump.rs:180-182`](../crates/net/src/dump.rs#L180-L182)<br>[`crates/eqoa/src/play.rs:242, 4881`](../crates/eqoa/src/play.rs#L242) | **Statically Verified** (Unimplemented in Lua) |
| `player.GetMaxHp()` | `retro_health_hearts.lua:196`<br>`ff_example.lua:414` | `() -> integer`<br>Max real HP. Proposed absent: `0`. | `pub stat: [i32; 29]` (index `8` is max HP)<br>`self.stats.stat[8]` | [`crates/net/src/dump.rs:180-182`](../crates/net/src/dump.rs#L180-L182)<br>[`crates/eqoa/src/play.rs:242, 4882`](../crates/eqoa/src/play.rs#L242) | **Statically Verified** (Unimplemented in Lua) |
| `ui.DisableHealthBar(owner)`<br>`ui.EnableHealthBar(owner)` | `retro_health_hearts.lua:50, 54, 97`<br>`ff_example.lua:243, 247` | `(string \| nil) -> boolean`<br>Returns `true` on registration. | Native draw call: `draw_gauge(vui, ui, 7, 136, p, true, s.self_hp as f32 / 255.0)` | [`crates/eqoa/src/hud.rs:283`](../crates/eqoa/src/hud.rs#L283) | **Statically Verified** (Proposed hook; unplumbed) |

### 3.2 Source Proof Snippets for First-Slice Verification

* **Network Telemetry Data Origin (`net/src/dump.rs:177-185`):**
  ```rust
  /// The stat channel (0x42, 233 bytes): `VIClient::ReadStats` into
  /// `VIPlayer`. Indices 0-6 STR STA AGI DEX WIS INT CHA, 7 HP, 8 max HP,
  /// 9 power, 10 max power.
  #[derive(Clone, Copy, Debug, Default)]
  pub struct Stats {
      pub stat: [i32; 29],
      pub base: [i32; 29],
      pub p30: u8,
  }
  ```
* **Play Struct Field & Deserialization (`eqoa/src/play.rs:242, 2374`):**
  ```rust
  pub stats: net::dump::Stats,
  // ...
  Event::State { channel: 0x42, data } => {
      self.stats = net::dump::read_stats(&mut net::stream::Reader::new(&data));
  }
  ```
* **Existing Runtime Host Bindings (`eqoa/src/fork_ext/addons/host.rs:52-59`):**
  ```rust
  util.set("IsInGame", lua.create_function(move |_, ()| Ok(i32::from(state.get().in_game)))?)?;
  util.set("IsStartMenuOpen", lua.create_function(move |_, ()| Ok(i32::from(state.get().start_menu)))?)?;
  ```
* **Existing Viewer Per-Frame Evaluation (`eqoa/src/viewer.rs:5264-5266`):**
  ```rust
  let in_game = self.play.as_ref().is_some_and(|p| p.session.self_id.is_some() && p.hud_ready());
  let start_menu = self.play.as_ref().is_some_and(|p| p.start.as_ref().is_some_and(|s| s.menu.visible));
  spike.draw(&g.window, &mut enc, &target, in_game, start_menu);
  ```
* **Native HUD Health Bar Render Site (`eqoa/src/hud.rs:283`):**
  ```rust
  p = [p[0] + 59, p[1] + 21];
  draw_gauge(vui, ui, 7, 136, p, true, s.self_hp as f32 / 255.0);
  ```

---

## 4. Host Bridge & ImGui Gates Blocking `retro_health_hearts.lua`

> [!CAUTION]
> Implementing `util`, `player`, and `ui` adapter functions alone **will not make `retro_health_hearts.lua` functional**.
> The current bridge ([`crates/eqoa/src/fork_ext/addons/bridge.rs`](../crates/eqoa/src/fork_ext/addons/bridge.rs)) was implemented as a constrained spike (`AddLine`, `GetDisplaySize`, `SliderInt/Float`, `ColorEdit4`) and is missing the following required symbols:

1. **`ImVec4.new(x, y, z, w)`**: Missing user type. Required at lines 37, 178, 184, 189, 222.
2. **`ImGui.Checkbox(label, bool)`**: Missing binding. Required at lines 79, 82, 89.
3. **Cursor Positioning**: Missing `ImGui.GetCursorPos()` (lines 172, 176, 220), `ImGui.GetCursorScreenPos()` (line 212), and `ImGui.SetCursorPos(x, y)` (lines 183, 188, 211, 221).
4. **Layout Primitives**: Missing `ImGui.SameLine(offset, spacing)` (lines 192, 224).
5. **Clipping**: Missing `ImGui.PushClipRect(min_x, min_y, max_x, max_y, intersect)` (line 216) and `ImGui.PopClipRect()` (line 227).
6. **Texture Overloads**: `ImGui.Image()` in `bridge.rs` does not accept the 6-parameter signature `(texture, size, uv0, uv1, tint, border_col)` called at lines 179, 185, 190, 223.
7. **Resource Loading**: `Textures::install_for_addon` now supplies `UiForge.IGraphicsApi.CreateTextureFromFile(path)` and `UiForge.resources_path` for PNG resources. The future manager/application adapter still needs to install these bindings for each addon; their existence does not establish unchanged-script acceptance.
8. **Window Flags**: The spike already supplies `ImGuiWindowFlags.AlwaysAutoResize`, `NoBackground`, and `NoTitleBar`; these three constants are not outstanding gates.

---

## 5. Proposed Per-VM Multi-Owner HUD Claims Architecture

Native client currently renders HUD gauges unconditionally in [`crates/eqoa/src/hud.rs:269-346`](../crates/eqoa/src/hud.rs#L269-L346). To support Forge UI hiding without corrupting user options or maps:

```rust
// PROPOSED architecture (not yet implemented in native codebase)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HudElement {
    HealthBar,
    PowerBar,
    MainExpBar,
    SecondaryExpBar,
    Compass,
    TargetNameplate,
    GroupMemberPanel,
    GroupCompassMarkers,
    PetPanel,
    ActiveEffects,
    AbilityBar,
    ChatWindow,
}

#[derive(Default)]
pub struct HudClaimsRegistry {
    claims: std::collections::HashMap<HudElement, std::collections::HashSet<(u64, String)>>,
}

impl HudClaimsRegistry {
    pub fn claim(&mut self, elem: HudElement, vm_id: u64, owner: &str) {
        self.claims.entry(elem).or_default().insert((vm_id, owner.to_string()));
    }
    pub fn release(&mut self, elem: HudElement, vm_id: u64, owner: &str) {
        if let Some(set) = self.claims.get_mut(&elem) {
            set.remove(&(vm_id, owner.to_string()));
        }
    }
    pub fn release_all_for_vm(&mut self, vm_id: u64) {
        for set in self.claims.values_mut() {
            set.retain(|(id, _)| *id != vm_id);
        }
    }
    pub fn is_hidden(&self, elem: HudElement) -> bool {
        self.claims.get(&elem).is_some_and(|set| !set.is_empty())
    }
}
```

* **Proposed Hook Sites in `hud.rs`:** Wrap `draw_gauge` calls with `if !claims.is_hidden(HudElement::HealthBar)`.
* **Proposed Lifecycle Bound:** On VM drop, crash, or unload, call `release_all_for_vm(vm_id)`.
* **Map & User Settings Boundaries:**
  * Native user preferences in [`settings.rs`](../crates/eqoa/src/settings.rs) are never altered.
  * Neither the circular minimap ([`crates/eqoa/src/map/mini.rs`](../crates/eqoa/src/map/mini.rs)) nor the full world map ([`crates/eqoa/src/map/tool.rs`](../crates/eqoa/src/map/tool.rs)) are part of `HudElement`. `UI.DisableUI()` will not hide maps.
  * The global user toggle `hideui::hidden()` (`H` key, [`hideui.rs`](../crates/eqoa/src/hideui.rs)) remains the master user override.

---

## 6. Absent-Player Semantics: Proposed Native Adaptation vs Legacy Reality

Upstream FrontiersForge ran via PCSX2 memory injection and read raw guest memory offsets (`player_offset = 0x1FBBA0C`). When the client was out of world, it dereferenced uninitialized or stale guest RAM; upstream had **no defined absent-player return model**.

Any clean return convention in native Rust is an **explicit proposed adaptation**, not faithful emulation:
* **Numeric Stats (`GetCurrentHp`, `GetMaxHp`, `GetLevel`, etc.):** Proposed return: `0`.
* **Text (`GetName`, `GetLocationName`):** Proposed return: `""` (or `nil` for location).
* **Tables (`GetCoordinates`, `GetVelocity`):** Proposed return: `nil`.
* **Booleans (`IsDead`, `IsAutoAttacking`):** Proposed return: `false`.

---

## 7. Audit of Remaining `ff_example.lua` Target APIs

### 7.1 Verified Static Fields (Target ID & Math)
* **`Player.GetTargetEntityId()` (`ff_example.lua:456, 457, 468, 584`):**
  * *Native Field:* `pub target: u32` in [`crates/eqoa/src/play.rs:195`](../crates/eqoa/src/play.rs#L195).
  * *Proof:* Initialized to `u32::MAX` in `play.rs:1898`, cleared to `u32::MAX` in `play.rs:4464`, passed to `HudState.target` in `play.rs:7464`.
  * *Proposed Adaptation:* Translate `self.target == u32::MAX` to numeric `0`.
* **`Util.GetDistanceBetween(a, b)` (`ff_example.lua:378`):**
  * *Formula in `util.lua:366-371`:*
    ```lua
    function Util.GetDistanceBetween(point_a, point_b)
        local dx = point_a.x - point_b.x
        local dy = point_a.y - point_b.y
        local dz = point_a.z - point_b.z
        return math.sqrt(dx * dx + dy * dy + dz * dz)
    end
    ```
  * True 3D Euclidean distance over tables containing `.x`, `.y`, and `.z`.
* **`Util.GetExpRequiredForLevel(level)` (`ff_example.lua:231`):**
  * *Formula in `util.lua:59-73, 358-360`:* Direct lookup in static table `exp_for_levels[1..59]`. Returns `nil` for level $\ge 60$ or $\le 0$. Matches native formula `crate::hud::exp_for_level(level + 1)` in [`crates/eqoa/src/hud.rs:115-122`](../crates/eqoa/src/hud.rs#L115-L122).

### 7.2 Prohibited & Unresolved APIs (True Native Gaps)

| Upstream Function | Exact `ff_example.lua` Call Site | Upstream Legacy Mechanism | Native Status & Architectural Rationale |
| :--- | :--- | :--- | :--- |
| `Util.EEmem()` | `ff_example.lua:230` | `tonumber(ffi.C.EEmem)` | **PROHIBITED.** Native client has no emulated PS2 RAM. **Contract:** Calling `Util.EEmem()` must immediately raise a named Lua error: `"EEmem is unsupported: native engine has no guest EE RAM"`. Returning dummy 0 or null is rejected. |
| `Util.IsBattleMusicPlaying()` | `ff_example.lua:234` | Reads entity flag byte `player_entity + 0x263` | **UNRESOLVED.** Native client has combat timers and creature damage flags (`flags262 & 4`), but has no dedicated battle music trigger byte. Cannot be mapped directly without a design decision. |
| `Util.GetTextDialogText()` | `ff_example.lua:67` | Reads memory buffer of `VIWndPrivChat` | **UNRESOLVED.** Native `PrivChat` ([`crates/eqoa/src/dialog.rs:10`](../crates/eqoa/src/dialog.rs#L10)) holds private `text: Vec<u16>`; no public string getter exists on `Play`. |
| `Util.GetConfirmDialogText()`<br>`Util.GetConfirmDialogStyle()`<br>`Util.GetConfirmDialogSelection()` | `ff_example.lua:73, 74, 77` | Reads option box memory | **UNRESOLVED.** Native `DialogBox` ([`crates/eqoa/src/dialog.rs:47`](../crates/eqoa/src/dialog.rs#L47)) holds private fields; `Play.confirm_text`, `confirm_style`, `confirm_selection` **do not exist**. |
| `Util.GetInputDialogTag()`<br>`Util.GetInputDialogText()`<br>`Util.IsChatKeyboardOpen()` | `ff_example.lua:82, 86, 87` | Reads input widget mode tag and buffer | **UNRESOLVED.** `Play.input_dialog_tag` and `Play.input_buffer` **do not exist**. |
| `Util.GetLocalizedString(id)` | `ff_example.lua:91` | Reads `LOCALIZATION_TABLE` at `0x1FAF734` | **UNRESOLVED.** Native string table exists in `vi::ui::Vui.words: StringTable` ([`crates/vi/src/ui.rs:115`](../crates/vi/src/ui.rs#L115)), but no free function `lookup_string` exists. |

---

## 8. Summary of `ff_example.lua` Unconditional Startup Blockers

Beyond the unresolved functions above, `ff_example.lua` remains strictly blocked by top-level startup code:
1. **21 Unconditional `require()` calls (lines 1–21):** Halts at line 4 on `frontiers_forge.entity`.
2. **Startup Subscriptions (lines 1275–1282):** Unconditional calls to `SubscribeToCombatEvents()`, `SubscribeToChat()`, and `Icon.Acquire()`.
3. **Per-Frame Drains (lines 1597–1602):** Unconditional per-frame calls to `Input.Update()`, `Combat.Update()`, and `Chat.Update()`.


## Root review limits

The two utility flags have fixture coverage and a viewer source hook; no live Forge gameplay session has been validated. HP bindings and owner-scoped HUD suppression remain unimplemented. Before accepting the health addon, verify that the in-game readiness gate cannot become true before a valid positive max-HP snapshot arrives: the unchanged script divides current HP by max HP. Do not invent a max-HP value to avoid that edge case. Missing native equivalents above mean no verified mapping has yet been identified, rather than proof that native state can never be exposed.
