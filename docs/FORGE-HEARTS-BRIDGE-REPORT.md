# Forge Hearts Bridge Slice Report

## 1. Executive Summary

Under the authorized `BUILD NOW` instruction, the bounded ImGui bridge next-slice required by `retro_health_hearts.lua` was implemented and rigorously verified directly in `crates/eqoa/src/fork_ext/addons/bridge.rs`. Verification followed a strict TDD and controlled negative oracle methodology. All 3 coordinate-targeted oracles in the remediated GPU test were executed with real negative controls, followed by full GREEN suite verification with `--locked --nocapture --test-threads=1`. No health, manager, runtime, textures, shared files, dependencies, policies, or maps were altered.

---

## 2. Implemented Native Adapter Contracts

Authoritative reference: pinned `sol2_ImGui_Bindings` @ `ec6f9ef7bab73e60f7ed32ef1e95dbee3b7b348e` (`UiForge/externals/sol2_ImGui_Bindings/sol_ImGui.h`).

| API / Feature | Upstream C++ Signature | Native Rust Bridge Implementation | Semantics & Guards |
| :--- | :--- | :--- | :--- |
| `ImVec4` Userdata | `sol_ImGui.h:2530-2536` | `Vec4(f32, f32, f32, f32)` | Constructors: `ImVec4.new()` (zeroes) and `ImVec4.new(x, y, z, w)`. Strict rejection of 1, 2, or 3 args. Read/write fields `.x, .y, .z, .w` finite-checked. |
| `ImGui.Checkbox` | `sol_ImGui.h:243-249` | `sys::igCheckbox` | Returns `(value: bool, pressed: bool)`. First return provides updated value for single-assignment callers (`retro_health_hearts.lua:79`). |
| `ImGui.GetCursorPos` | `sol_ImGui.h:194` | `sys::igGetCursorPos` | Returns two scalar numbers `(x, y)` as Lua multireturn tuple, not an `ImVec2` object. |
| `ImGui.GetCursorScreenPos` | `sol_ImGui.h:201` | `sys::igGetCursorScreenPos` | Returns two scalar numbers `(x, y)` as Lua multireturn tuple. |
| `ImGui.SetCursorPos` | `sol_ImGui.h:197` | `sys::igSetCursorPos` | Takes `(x: f32, y: f32)` finite-validated; sets window-local cursor position. |
| `ImGui.SameLine` | `sol_ImGui.h:182-184` | `sys::igSameLine` | Supports 0 args, 1 arg (`offset`), or 2 args (`offset, spacing`). Finite-checked. |
| `ImGui.PushClipRect` | `sol_ImGui.h:1651, 3275` | `sys::igPushClipRect` | Takes 5 scalars: `(min_x, min_y, max_x, max_y, intersect)`. Pushes `Stack::ClipRect`. |
| `ImGui.PopClipRect` | `sol_ImGui.h:1652, 3276` | `sys::igPopClipRect` | Pops current window's clip rect. Rejects popping parent window's clip from child window. |
| `ImGui.End` Guard | ImGui internal assert guard | `Frame::check` & `Stack` | Checks for unclosed `ClipRect` after current `Window`. Reports Lua runtime error *before* native `sys::igEnd()`. |
| `ImGui.Image` Overload 1 | Existing spike signature | `sys::igImage` | 3 total args: `(handle, width, height)`. Finite and positive dimension checks. |
| `ImGui.Image` Overload 2 | `sol_ImGui.h:236-241` | `sys::igImageWithBg` | 6 total args: `(handle, size_vec2, uv0_vec2, uv1_vec2, tint_vec4, border_vec4)`. Maps `border_vec4` to `bg_col` (arg 5) and `tint_vec4` to `tint_col` (arg 6) per `sys::igImageWithBg` binding layout. |
| Stack Unwind Loop | `Bridge::invoke` | LIFO stack drain | Unwinds `Stack::ClipRect => sys::igPopClipRect()` before `Stack::Window => sys::igEnd()`. |

---

## 3. Test Verification & Results

### 3.1 Initial RED Phase
- Test: `forge_bridge_retro_health_hearts_slice_contracts` executed against untouched baseline.
- Result: **FAILED** as expected with `RuntimeError: attempt to index global 'ImVec4' (a nil value)`.

### 3.2 Strengthened GPU Test Architecture (`image-readback-remediation.patch`)
The test `fork_ext::addons::bridge::tests::forge_bridge_image_overloads_and_wgpu_readback` was strengthened with coordinate-targeted assertions to eliminate broad whole-buffer oracles:
- **Texture Setup**: 4x4 RGBA texture partitioned into distinct quadrants:
  - Top-Left: RED `[255, 0, 0, 255]`
  - Top-Right: GREEN `[0, 255, 0, 255]`
  - Bottom-Left: BLUE `[0, 0, 255, 255]`
  - Bottom-Right: FULLY TRANSPARENT `[0, 0, 0, 0]`
- **Screen Origin Export**: Lua exports each rendered image's top-left coordinates via `GetCursorScreenPos()` (`img_a_x/y`, `img_b_x/y`, `img_c_x/y`).
- **Targeted Pixel Assertions**:
  1. **Image A (3-arg full texture)**: Asserts top-left interior `(+8, +8)` is RED (`p[0] > 200, p[1] < 40, p[2] < 40`) and top-right interior `(+24, +8)` is GREEN (`p[0] < 40, p[1] > 200, p[2] < 40`).
  2. **Image B (6-arg top-right UV crop with 0.5 green tint)**: Asserts interior `(+16, +16)` is green attenuated to ~128 (`p[1] >= 110 && p[1] <= 145, p[0] < 40, p[2] < 40`). A missing UV crop samples red; missing tint attenuation produces full green (255).
  3. **Image C (6-arg bottom-right transparent UV crop with opaque blue border/background)**: Asserts interior `(+16, +16)` is pure BLUE (`p[2] > 200, p[0] < 40, p[1] < 40`). A missing background fill leaves transparent/black; swapped tint/background fails the blue fill.

### 3.3 Real Controlled Negative Oracles (Executed & Verified)

Three real negative controls were independently compiled and executed against the live GPU test to prove the specificity of each assertion:

1. **Negative Oracle #1: 6-Arg Draw Call Disabled**
   - *Modification*: Suppressed `sys::igImageWithBg` call for 6-arg `Image` overload in production code.
   - *Result*: Test **FAILED** specifically at Image B:
     ```text
     thread 'fork_ext::addons::bridge::tests::forge_bridge_image_overloads_and_wgpu_readback' panicked at crates\eqoa\src\fork_ext\addons\bridge.rs:1139:13:
     Image B interior must be tinted green (~128), got RGBA([0, 0, 0, 255])
     ```
   - *Conclusion*: Proves Image B coordinate cannot pass through background or Image A pixel leakage.

2. **Negative Oracle #2: Tint Multiplier Bypassed**
   - *Modification*: Replaced `tint.raw()` with unattenuated white `[1.0, 1.0, 1.0, 1.0]` in `sys::igImageWithBg`.
   - *Result*: Test **FAILED** specifically at Image B:
     ```text
     thread 'fork_ext::addons::bridge::tests::forge_bridge_image_overloads_and_wgpu_readback' panicked at crates\eqoa\src\fork_ext\addons\bridge.rs:1139:13:
     Image B interior must be tinted green (~128), got RGBA([0, 255, 0, 255])
     ```
   - *Conclusion*: Proves Image B specifically asserts the 0.5 tint attenuation rather than raw texture green.

3. **Negative Oracle #3: Background Fill Bypassed**
   - *Modification*: Replaced `border.raw()` (which maps to `bg_col` in `igImageWithBg`) with transparent `[0.0, 0.0, 0.0, 0.0]`.
   - *Result*: Test **FAILED** specifically at Image C:
     ```text
     thread 'fork_ext::addons::bridge::tests::forge_bridge_image_overloads_and_wgpu_readback' panicked at crates\eqoa\src\fork_ext\addons\bridge.rs:1148:13:
     Image C interior must be opaque blue background fill, got RGBA([0, 0, 0, 255])
     ```
   - *Conclusion*: Proves Image C specifically asserts opaque background fill under a transparent UV crop.

### 3.4 GREEN Suite Verification

Production source was restored to its exact verified state. The full bridge test suite was executed serially with captured output:

- **Command**:
  ```powershell
  cargo test --locked -p eqoa fork_ext::addons::bridge -- --test-threads=1 --nocapture
  ```
- **Harness Output**:
  ```text
  running 3 tests
  test fork_ext::addons::bridge::tests::forge_bridge_image_overloads_and_wgpu_readback ... ok
  test fork_ext::addons::bridge::tests::forge_bridge_renders_real_lines_preserves_returns_and_rejects_expired_handles ... ok
  test fork_ext::addons::bridge::tests::forge_bridge_retro_health_hearts_slice_contracts ... ok

  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1055 filtered out; finished in 0.79s
  ```

### 3.5 Test Boundaries & Deferred Retirement Ownership

- **Bridge Test Scope**: Verifies handle invalidation and host safety boundaries. Specifically, `UiForge.ReleaseTexture(tex)` causes subsequent bridge `ImGui.Image` calls with the released handle to fail safely with `"expired texture handle"`. Furthermore, cross-bank handle releases fail with `"foreign texture handle"`.
- **Manager GPU Suite Scope**: Verifies end-to-end WGPU deferred retirement. Valid draw calls issued in the current frame survive handle release and render correctly without GPU resource panics or missing draw lists across frames. This lifecycle is owned and verified in `manager::tests::forge_manager_gpu_failure_isolation_and_after_render_retirement`.

---

## 4. Footprint & Boundary Compliance

- **Modified Files**: Only `crates/eqoa/src/fork_ext/addons/bridge.rs` and `docs/FORGE-HEARTS-BRIDGE-REPORT.md`.
- **Untouched Subsystems**: `manager.rs`, `health.rs`, `runtime.rs`, `textures.rs`, dependencies, manifests, and maps were completely untouched.
- **Compiler Lock**: The compiler slot was utilized only for the authorized bridge remediation and negative oracle runs, and is now fully released. Worker is idle and standing by.
