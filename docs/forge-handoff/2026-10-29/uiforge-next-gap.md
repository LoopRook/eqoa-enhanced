# Audit: UIForge Example Acceptance Gaps & Authoritative C++ Semantics

**Date:** 2026-10-29
**Status:** Read-Only Source Audit (Corrected against `forge-compatibility` HEAD)
**Target:** Pinned reference `uiforge_example` (`uiforge_example.lua`), private module (`modules/click_tally.lua`), and resource asset (`resources/gear-icon.png`).
**Audited Forge Source:** `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`
- `crates/eqoa/src/fork_ext/addons/bridge.rs` (SHA256: `d8cee6c61f6d2ff452a64a2e12d0e55aef3ad8478ce9ca83dff0a8481fbc23d7`)
- `crates/eqoa/src/fork_ext/addons/host.rs` (SHA256: `bca8494b266c335c832c8f03ad0d7c35ed041fd9e9f8a897cc6bb2c270edb500`)
- `crates/eqoa/src/fork_ext/addons/textures.rs` (SHA256: `c42e4d33c5d2e74a524a5303a5890afdb1b0f8ec1b220165e42d1b93de17bca4`)
- `crates/eqoa/src/fork_ext/addons/manager.rs` (SHA256: `2f9663437a10d5550f46c08d462b6b5e0be42a35ba303da937f71a7417e9a271`)

---

## 1. Executive Summary

This audit assesses the exact gap between current native Forge implementation and real, unmodified acceptance of `uiforge_example`.

A re-audit of the active `forge-compatibility` branch confirms that several core multi-return bindings and the entire resource/texture pipeline are **already completed**:
- `ImGui.Checkbox`, `ImGui.SliderInt`, `ImGui.SliderFloat`, `ImGui.ColorEdit4`, and `ImGui.InputText` already return full multi-return tuples (`(value, changed/pressed)`).
- `UiForge.resources_path`, `UiForge.LoadTexture` (package-relative and absolute), and `ImGui.Image` (3-argument and 6-argument overloads) are already implemented and tested in `textures.rs`.

The remaining gaps to achieve clean execution of `uiforge_example` are:
1. **Window Flag Mask Whitelist & Missing Composite Flags**: Expanding `ImGui.Begin` allowed flags (currently hardcoded to only 3 flags) and defining composite flags (`NoDecoration`, `NoInputs`, `NoNav`) in `ImGuiWindowFlags`.
2. **Missing Container & Hierarchical Widgets**: Menu bars, tab bars, tree nodes, child windows, and combo boxes.
3. **Missing Vector Primitives on `ImDrawList`**: Vector drawing functions (`AddCircleFilled`, `AddRectFilled`, `AddTriangleFilled`, `AddNgonFilled`, and outlined variants) and `ImGui.GetWindowDrawList()`.
4. **Standard Library**: Native `bit` module for `bit.band` and `bit.bxor` flag math.
5. **Font & Audio Handling**: Faithful fallback semantics matching C++ (`UiForge.LoadFont` returning default font; `UiForge.LoadSound` returning `nil` when audio is absent without crashing).

---

## 2. Exact Execution Flow & Callback Lifecycle

### A. Chunk Load & Startup Gating (Top of `uiforge_example.lua`)
The script evaluates top-to-bottom on execution:
1. **Module Requires**:
   - `local bit = require("bit")`: Needs native `bit` module.
   - `local ClickTally = require("click_tally")`: Successfully resolves via package loader to `uiforge_example/modules/click_tally.lua`.
2. **Global State Table**:
   - Initializes `state` if nil, including `demo_window_flags = ImGuiWindowFlags.None`.
3. **Callback Registrations**:
   - `UiForge.RegisterCallback(UiForge.CallbackType.Settings, Settings)`:
     - Invoked by Addon Manager settings panel; renders `ImGui.SliderInt("Number of Balls", state.ball_count, 1, 100)`.
   - `UiForge.RegisterCallback(UiForge.CallbackType.Save, Save)`:
     - Invoked on profile save; returns table `{ ball_count, balls = { ... } }`.
   - `UiForge.RegisterCallback(UiForge.CallbackType.Load, Load)`:
     - Invoked on profile load; consumes saved table to reconstruct ball positions and velocities.
4. **Asset Preloads & Script Gating (Lines 200–210)**:
   - `local times_font = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 20.0)`
   - `local times_font_large = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 36.0)`
   - `local missing_font = UiForge.LoadFont("C:\\Windows\\Fonts\\does_not_exist.ttf", 20.0)`
   - `local tada_sound = UiForge.LoadSound("C:\\Windows\\Media\\tada.wav")`
   - **Script Gating Analysis**:
     - The script calls `UiForge.LoadFont` and `UiForge.LoadSound` unconditionally at chunk evaluation.
     - However, in the frame loop (Line 506), audio usage is explicitly guarded:
       ```lua
       if tada_sound == nil then
           ImGui.Text("Could not load C:\\Windows\\Media\\tada.wav")
       else
           ...
       ```
       If `UiForge.LoadSound` returns `nil`, the script cleanly renders its fallback text without attempting to play audio or erroring.
     - For fonts (Lines 491, 495, 499), `ImGui.PushFont` is called directly. In authoritative C++ (`core.cpp:696`), `LoadFont` caches and returns the default font on missing/unparseable files, ensuring `PushFont` always receives a valid font handle.

### B. Frame Execution Driver
The script's top-level UI is executed during the native frame update via `Manager.frame(...)` (in `crates/eqoa/src/fork_ext/addons/manager.rs`).

---

## 3. Current Implementation Status vs Remaining Gaps

### Status Matrix: Core Bindings & Overloads

| API / Feature | Authoritative C++ Semantics (`sol_ImGui.h` / `core.cpp`) | Current Native Forge Status (`forge-compatibility`) | Status & Gap Analysis |
| :--- | :--- | :--- | :--- |
| **`ImGui.Checkbox`** | `std::tuple<bool, bool> (value, pressed)` | `Ok((value, pressed))` in `bridge.rs:326-327` | **ALREADY COMPLIANT**. Supports both single-assignment (`val = Checkbox(...)`) and tuple destructuring (`val, pressed = Checkbox(...)`). |
| **`ImGui.SliderInt`** | `std::tuple<int, bool> (v, used)` | `Ok((value, changed))` in `bridge.rs:476` | **ALREADY COMPLIANT**. |
| **`ImGui.SliderFloat`** | `std::tuple<float, bool> (v, used)` | `Ok((value, changed))` in `bridge.rs:504` | **ALREADY COMPLIANT**. |
| **`ImGui.ColorEdit4`** | `std::tuple<table, bool> (rgba, changed)` | `Ok((table, changed))` in `bridge.rs:524` | **ALREADY COMPLIANT**. |
| **`ImGui.InputText`** | `std::tuple<string, bool> (text, entered)`| `Ok((value, entered))` in `bridge.rs:553` | **ALREADY COMPLIANT**. |
| **`UiForge.resources_path`** | String path to `scripts/resources` | String in `textures.rs:282` | **ALREADY COMPLIANT**. |
| **`UiForge.LoadTexture`** | Resolves package resource then shared | Implemented in `textures.rs:287-312` | **ALREADY COMPLIANT**. |
| **`ImGui.Image`** | 3-arg and 6-arg overloads | Implemented in `bridge.rs:560-623` | **ALREADY COMPLIANT**. |
| **`ImGui.Begin`** | `std::tuple<bool, bool> Begin(name, open, flags)` returning `(open, shouldDraw)` | `bridge.rs:286-290` returns `(visible, is_open)` | **GAP**: `bridge.rs:261-266` hardcodes an `allowed` mask (`NoTitleBar \| AlwaysAutoResize \| NoBackground`) and errors on any other flag. Fails on `MenuBar` (L214) and `demo_window_flags` (L401). |
| **`ImGui.Combo`** | `std::tuple<int, bool> (currentItem, clicked)` | **ABSENT** in `bridge.rs` | **GAP**: Needed for L270 (`ImGui.Combo("Combo Box", ...)`). |
| **`ImGui.RadioButton`** | `bool RadioButton(label, active)` | **ABSENT** in `bridge.rs` | **GAP**: Needed for L261, L265. |
| **`ImGui.GetTime`** | `double GetTime()` | **ABSENT** in `bridge.rs` | **GAP**: Needed for L305, L539. |
| **`bit` Module** | LuaJIT BitOp (`band`, `bxor`) | **ABSENT** in `host.rs` | **GAP**: Needed for L6, L395, L397. |

---

## 4. Detailed Gap Specifications

### A. ImGui Window Flags & Composite Definitions
In Dear ImGui, several flags are composite bitmasks rather than independent single bits:
- `ImGuiWindowFlags.None` = `0`
- `ImGuiWindowFlags.NoNav` = `NoNavInputs | NoNavFocus`
- `ImGuiWindowFlags.NoDecoration` = `NoTitleBar | NoResize | NoScrollbar | NoCollapse`
- `ImGuiWindowFlags.NoInputs` = `NoMouseInputs | NoNavInputs | NoNavFocus`

Current `bridge.rs:663-667` only defines `NoTitleBar`, `AlwaysAutoResize`, and `NoBackground`. `uiforge_example.lua:370-392` iterates over 21 flags. The `ImGuiWindowFlags` global table must be populated with all 21 flags, and `ImGui.Begin`'s whitelist must be relaxed to allow these flags.

### B. Missing Container & Layout Widgets
1. **Menu Bar Subsystem**:
   - `ImGui.BeginMenuBar()` -> `bool` (starts menu bar).
   - `ImGui.EndMenuBar()` -> `void`.
   - `ImGui.BeginMenu(label [, enabled])` -> `bool`.
   - `ImGui.EndMenu()` -> `void`.
   - `ImGui.MenuItem(label [, shortcut [, selected [, enabled]]])` -> `bool` (clicked).
2. **Tree Hierarchy**:
   - `ImGui.TreeNode(label)` -> `bool` (returns true if node is open).
   - `ImGui.TreePop()` -> `void`.
3. **Tab Subsystem**:
   - `ImGui.BeginTabBar(str_id [, flags])` -> `bool`.
   - `ImGui.EndTabBar()` -> `void`.
   - `ImGui.BeginTabItem(label)` -> `bool` (returns true if tab selected).
   - `ImGui.EndTabItem()` -> `void`.
4. **Child Windows**:
   - `ImGui.BeginChild(id, size_x, size_y [, child_flags [, window_flags]])` -> `bool`.
   - `ImGui.EndChild()` -> `void`.
   - `ImGuiChildFlags` table defining `Border = sys::ImGuiChildFlags_Border` (1 << 0).
5. **Basic Interactive & Display Widgets**:
   - `ImGui.Button(label [, size_x, size_y])` -> `bool` (supports optional size arguments; L243 passes `200, 50`).
   - `ImGui.BulletText(text)` -> `void`.
   - `ImGui.TextDisabled(text)` -> `void`.
   - `ImGui.ProgressBar(fraction [, size_x, size_y [, overlay]])` -> `void`.
   - `ImGui.Dummy(size_x, size_y)` -> `void`.
   - `ImGui.IsItemHovered([flags])` -> `bool`.
   - `ImGui.SetTooltip(text)` -> `void`.
   - `ImGuiCol` table must include `PlotHistogram` (sys::ImGuiCol_PlotHistogram).

### C. `ImDrawList` Vector Drawing Primitives
Current `DrawList` in `bridge.rs:141` only implements `AddLine`. `uiforge_example.lua:332-366` and `RenderBalls` (L129) require:
1. `ImGui.GetWindowDrawList()` returning the active window's `DrawList` (currently only `GetBackgroundDrawList` exists).
2. DrawList Methods:
   - `AddRectFilled(p0, p1, col [, rounding, flags])`
   - `AddRect(p0, p1, col [, rounding, flags, thickness])`
   - `AddCircleFilled(center, radius, col [, segments])` (critical for bouncing balls)
   - `AddCircle(center, radius, col [, segments, thickness])`
   - `AddTriangleFilled(p0, p1, p2, col)`
   - `AddTriangle(p0, p1, p2, col [, thickness])`
   - `AddNgonFilled(center, radius, col, sides)`
   - `AddNgon(center, radius, col, sides [, thickness])`

### D. Native `bit` Module
- Required by `uiforge_example.lua:6`: `local bit = require("bit")`.
- Calls `bit.band(flags, flag)` and `bit.bxor(flags, flag)` in window flags loop (L395, L397).
- Needs native registration conforming to signed 32-bit BitOp semantics (`stdlib-next-brief.md`).

### E. Fonts & Audio APIs
1. **Fonts**:
   - `UiForge.LoadFont(path, size_px)`: In authoritative C++ (`core.cpp:696`), a missing or unparseable font returns `io.FontDefault` so `PushFont` never faults.
   - For native acceptance, `LoadFont` must return a real font handle matching the active default font atlas, or fonts must be explicitly documented as unsupported/deferred. No dummy/fake userdata objects.
   - `ImGui.PushFont(font)` / `ImGui.PopFont()`: Stack push/pop for active font.
2. **Audio**:
   - `UiForge.LoadSound(path)`: Authoritative C++ returns `nil` (`sol::nullopt`) when the audio file is missing or unplayable.
   - `uiforge_example.lua:506` explicitly checks `if tada_sound == nil then` and renders fallback text.
   - Returning `nil` for `LoadSound` is a completely valid, faithful C++ equivalent when audio hardware/files are unavailable. If a real sound backend is implemented, it must use the native audio crate (`snd`).

---

## 5. Next Steps for UIForge Example Acceptance

1. **Window Flags Whitelist & Enums**: Remove artificial whitelist check in `ImGui.Begin` and export all 21 `ImGuiWindowFlags` (including composite masks `NoDecoration`, `NoInputs`, `NoNav`, `None`).
2. **Expose Native `bit` Module**: Register `bit` module in Host package table.
3. **Add Missing Widgets & Overloads**: Add `Combo`, `RadioButton`, sized `Button`, `ProgressBar`, `Dummy`, `IsItemHovered`, `SetTooltip`, `GetTime`, and hierarchical menus/tabs/trees.
4. **Extend `DrawList`**: Implement `ImGui.GetWindowDrawList()` and shape rasterization methods (`AddCircleFilled`, `AddRectFilled`, etc.).
5. **Faithful Media Stubs/Fallbacks**: Provide real default font fallback for `LoadFont` and return `nil` for `LoadSound` (matching the script's built-in fallback branch).
