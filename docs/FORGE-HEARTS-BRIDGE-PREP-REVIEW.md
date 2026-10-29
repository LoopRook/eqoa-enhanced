# Hearts Bridge Prep Slice — Independent Review

**Review Date**: 2026-10-28
**Scope**: Static review of prepared hearts bridge patch files under `.superpowers/sdd/2026-10-28-forge-compatibility/hearts-bridge-prep/` (`parent-red.patch`, `parent-green.patch`, `bridge_extras.rs`) against authoritative C++ `sol_ImGui.h`, `scripts/retro_health_hearts.lua`, and current `crates/eqoa/src/fork_ext/addons/bridge.rs`.
**Review Mode**: Independent STATIC review only. Zero source edits to active Rust, zero compiler/build invocations.

---

## Executive Summary

The prepared patches in `hearts-bridge-prep/` draft the next Dear ImGui bridge extensions required by `retro_health_hearts.lua`, covering `ImVec4` userdata, `Checkbox` multireturns, cursor query/manipulation, `SameLine` spacing defaults, `PushClipRect`/`PopClipRect` lifecycle, and the 6-argument `ImGui.Image` overload.

**Critical Evaluation**: These are **uncompiled, unverified prepared draft patches**. While the architectural stack safety and C++ binding alignment are promising, the proposed test suite contains **two concrete test blockers** (`pcall` usage in a sandboxed host, and inexact `f32` vs Lua double decimal comparisons) as well as a complete omission of `ImGui.Image` test coverage.

The patches must be considered **strictly unverified drafts**, conditionally pending author remediation of tests and real GPU image verification before any integration or compiler activation in Stage 2E.

---

## Detailed Findings

### 1. Stack Safety: Clip Rectangles and Nested Windows

1. **Clip Rect Window Scope Guard (`parent-green.patch:218-220`)**
   - `PushClipRect` checks `if !state.stack.iter().any(|k| *k == Stack::Window)` and immediately errors if called outside an active window. This prevents native `sys::igPushClipRect()` from crashing or asserting on a null `g.CurrentWindow`.
   - Stack depth is bounded at 32 (`state.stack.len() >= 32`).

2. **Unclosed ClipRect Guard Before Native `igEnd` (`parent-green.patch:117-133`)**
   - Dear ImGui asserts internally if `ImGui::End()` is called while a window clip rect remains on `DrawList->_ClipRectStack`.
   - The patch inspects `state.stack[last_window + 1..]` for any remaining `Stack::ClipRect`.
   - If unclosed, it returns a Lua runtime error **before** invoking native `sys::igEnd()`.
   - When the Lua frame/invocation fails, `Bridge::invoke` catches the error and unwinds `state.stack` in reverse order:
     ```rust
     while let Some(op) = state.stack.pop() {
         match op {
             Stack::Window => sys::igEnd(),
             Stack::Color => sys::igPopStyleColor(1),
             Stack::ClipRect => sys::igPopClipRect(),
         }
     }
     ```
     `ClipRect` is popped with `sys::igPopClipRect()` before `Window` is popped with `sys::igEnd()`, preserving the health of the shared ImGui Context.

3. **Window Nesting and Cross-Window Isolation (`parent-green.patch:57-84`)**
   - In `pop_clip_rect()`, searching starts at `search_start = last_window + 1`.
   - If a child window or popup is open and calls `ImGui.PopClipRect()` when no clip rect was pushed within that child window, it detects whether an outer parent window holds an active clip rect (`self.stack.iter().any(|k| *k == Stack::ClipRect)`).
   - If so, it returns an explicit error (`"cannot pop parent window clip rect from child window"`) without calling native `sys::igPopClipRect()`. This prevents a child window from corrupting the parent window's clipping hierarchy.

---

### 2. Overload and Return Semantics

1. **`Checkbox` Multireturn Order (`parent-green.patch:139-146`)**
   - Authoritative upstream `sol_ImGui.h:243-249`:
     ```cpp
     inline std::tuple<bool, bool> Checkbox(const std::string& label, bool v) {
         bool value{ v };
         bool pressed = ImGui::Checkbox(label.c_str(), &value);
         return std::make_tuple(value, pressed);
     }
     ```
   - `parent-green.patch` returns `Ok((value, pressed))`.
   - In Lua:
     - Single-return assignment (`retro_health_hearts.lua:79`: `state.show_accent = ImGui.Checkbox(...)`) assigns the updated `value`.
     - Multi-return assignment (`retro_health_hearts.lua:89`: `local new_val, pressed = ImGui.Checkbox(...)`) correctly captures both the updated value and the clicked trigger flag.

2. **Cursor Position Scalars (`parent-green.patch:149-164`)**
   - Upstream `sol_ImGui.h:194, 201` explicitly returns `std::tuple<float, float>`, not an `ImVec2` object:
     ```cpp
     inline std::tuple<float, float> GetCursorPos() { const auto vec2{ ImGui::GetCursorPos() }; return std::make_tuple(vec2.x, vec2.y); }
     ```
   - `parent-green.patch` returns `Ok((pos.x, pos.y))`.
   - In Lua: `local start_cursor_x, start_cursor_y = ImGui.GetCursorPos()` correctly unpacks as two numbers.

3. **`SameLine` Spacing Defaults (`parent-green.patch:180-198`)**
   - Upstream overloads `SameLine()`, `SameLine(offset)`, and `SameLine(offset, spacing)` map to Dear ImGui defaults: `offset = 0.0`, `spacing = -1.0`.
   - `parent-green.patch` handles `Option<f32>` for both parameters, validating bounds with `finite()`. Calls like `retro_health_hearts.lua:192` (`ImGui.SameLine(0, state.heart_spacing)`) unpack and execute seamlessly.

4. **`ImVec4` Userdata (`parent-green.patch:7-43, 92-108`)**
   - Upstream `sol_ImGui.h:2530-2536`:
     ```cpp
     lua.new_usertype<ImVec4>("ImVec4",
         sol::constructors<ImVec4(), ImVec4(float, float, float, float)>(),
         "x", &ImVec4::x, "y", &ImVec4::y, "z", &ImVec4::z, "w", &ImVec4::w
     );
     ```
   - `parent-green.patch` provides getters and setters for `x, y, z, w` with `finite()` checks.
   - `ImVec4.new` strictly permits 0 arguments (zeros) or 4 arguments (floats). Passing 1, 2, or 3 arguments errors out, matching C++ sol2 overload resolution.

5. **`ImGui.Image` Public Overloads and Parameter Mapping (`parent-green.patch:269-328`)**
   - Publicly in Lua, `ImGui.Image` has **3 total arguments** `(handle, width, height)` and **6 total arguments** `(handle, size, uv0, uv1, tint, border)`.
   - In the Rust closure implementation, the pattern `move |lua, (handle, args): (AnyUserData, mlua::MultiValue)|` binds the handle first, so `match args.len()` evaluates the **2 or 5 trailing arguments** after the handle.
   - Upstream `sol_ImGui.h:236-241`:
     ```cpp
     inline void Image(void* textureID, const ImVec2& image_size, const ImVec2& uv0, const ImVec2& uv1, const ImVec4& tint_col, const ImVec4& border_col) {
         ImGui::ImageWithBg(ImTextureRef(textureID), image_size, uv0, uv1, border_col, tint_col);
     }
     ```
   - In Dear ImGui C++, `ImageWithBg` takes `(tex, size, uv0, uv1, bg_col, tint_col)`.
   - `parent-green.patch` maps `border.raw()` to argument 5 (`bg_col`) and `tint.raw()` to argument 6 (`tint_col`), matching `sol_ImGui.h` exactly.

---

### 3. Texture Ownership

- Texture handles passed to `ImGui.Image` are resolved via `Textures::raw(&handle)`.
- This borrows the handle and checks `entry.alive` in the owner bank.
- It does **not** consume, drop, or alter texture lifecycle ownership.
- The external texture reference remains safely registered in `WgpuRenderer` under the host texture bank until frame completion and subsequent `after_render()` retirement.

---

### 4. Concrete Test Blockers & Coverage Gaps in Prepared Patches

The proposed test in `parent-red.patch:9-122` cannot compile/pass as written due to three concrete defects:

1. **`pcall` Stripped from Sandboxed Host Environment (Test Blocker)**
   - The proposed Lua script calls `pcall` in multiple places:
     - `local ok_partial, _ = pcall(function() ImVec4.new(1.0, 2.0) end)`
     - `local ok_nan, _ = pcall(function() v4.y = 0/0 end)`
     - `local ok, err = pcall(function() ImGui.PopClipRect() end)`
   - In `crates/eqoa/src/fork_ext/addons/host.rs`, `pcall` and `xpcall` are stripped from the sandboxed Lua environment for security and error isolation.
   - Calling `pcall` will fail immediately with `attempt to call global 'pcall' (a nil value)`.
   - *Required Remediation*: Negative test cases must use separate compiled chunks or sub-evaluations executed via Rust `host.run()` / `bridge.invoke()` expecting `Err`, rather than in-Lua `pcall`.

2. **Inexact Decimal Float Comparisons (`f32` vs Lua double) (Test Blocker)**
   - The test asserts exact equality for decimal literals:
     - `local v4 = ImVec4.new(0.1, 0.2, 0.3, 0.4)`
     - `assert(v4.x == 0.1 and v4.y == 0.2 and v4.z == 0.3 and v4.w == 0.4)`
   - `Vec4` stores single-precision `f32`. In IEEE-754 binary floating point, `0.1f32` rounds to `0.10000000149011612...`, whereas Lua double literal `0.1` is `0.10000000000000000555...`.
   - In Lua, `v4.x == 0.1` evaluates to **`false`** and trips the assertion.
   - *Required Remediation*: Use binary-exact dyadic fractions (e.g. `0.125`, `0.25`, `0.5`, `0.75`) or an explicit tolerance check (`math.abs(v4.x - 0.1) < 1e-6`).

3. **`ImGui.Image` Overload Entirely Untested (Coverage Gap)**
   - Despite adding `mlua::MultiValue` argument splitting for `Image` (3 vs 6 total arguments) and mapping to `sys::igImageWithBg`, `forge_bridge_retro_health_hearts_slice_contracts` **never calls `ImGui.Image`**.
   - Neither the 3-arg `(handle, w, h)` nor the 6-arg `(handle, size, uv0, uv1, tint, border)` signature is exercised.
   - *Required Remediation*: Author must add real GPU image readback coverage with a valid texture handle to prove `lua.unpack_multi` and `igImageWithBg` render without panic or memory corruption.

4. **Status of `bridge_extras.rs`**
   - `bridge_extras.rs` was an isolated prototyping scratchpad defining `StackKind`.
   - The unified diff in `parent-green.patch` directly modifies `bridge.rs` (using the existing `Stack` enum and `Frame::pop_clip_rect`), which is the true intended integration target.

---

## Conclusion

The prepared hearts bridge slice contains sound architectural ideas (especially around clip rect stack unwinding and sol2 parameter mapping), but the prepared patches are **strictly unverified drafts containing active test blockers**.

The author must revise the preparation to:
1. Eliminate `pcall` in favor of Rust-level error assertions.
2. Use exact dyadic or epsilon-bounded numeric assertions.
3. Add real GPU image readback coverage for the 6-argument `Image` overload.

**Status**: CONDITIONAL DRAFT ONLY (remediation required before compile/activation).
**Compiler Slot**: Sol owns compiler; no local builds executed.
