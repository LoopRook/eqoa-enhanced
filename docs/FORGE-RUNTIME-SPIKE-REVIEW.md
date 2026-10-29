# Forge Runtime Stage 1 Spike — Independent Code Review

**Reviewed Artifact**: `.superpowers/sdd/2026-10-28-forge-compatibility/runtime-spike-review.txt`
**Referenced Report**: [`docs/FORGE-RUNTIME-SPIKE-REPORT.md`](FORGE-RUNTIME-SPIKE-REPORT.md)
**Audited Sources**: [`crates/eqoa/src/fork_ext/addons/`](../crates/eqoa/src/fork_ext/addons), [`crates/eqoa/src/viewer.rs`](../crates/eqoa/src/viewer.rs), [`crates/eqoa/Cargo.toml`](../crates/eqoa/Cargo.toml)

---

### Verdicts

- **Specification Compliance**: **PASS / APPROVED**
  Implements the Stage 1 constrained feasibility spike strictly to specification. Explicit opt-in via `EQOA_FORGE_SPIKE`, clean fallback to zero runtime overhead when unset, real Dear ImGui + WGPU overlay render pass, physical 1:1 post-resolve coordinates, and strict WASM isolation.
- **Code Quality & Architecture**: **PASS / APPROVED**
  Careful separation of concern: the native engine exposes no raw pointers to Lua; Dear ImGui stacks are tracked and unwound on error; resource lifetimes and texture releases are deferred past the render pass to prevent GPU use-after-free; and profiles use atomic file writes.

---

### 1. Specification & Scope Compliance

1. **Zero-Overhead Default**:
   - `Spike::from_env` returns `None` unless `EQOA_FORGE_SPIKE` is explicitly set in the environment.
   - When unset, `g.forge_spike` is `None`: no Dear ImGui context, no Winit platform hooks, and no overlay render pass are created.
2. **Post-Resolve Physical Rendering**:
   - In [`crates/eqoa/src/viewer.rs`](../crates/eqoa/src/viewer.rs#L5260-L5268), `spike.draw` executes **after** `g.renderer.scale.resolve(&mut enc, &target)`.
   - Draws directly into the resolved window `target` with framebuffer scale locked to 1.0 (`HiDpiMode::Locked(1.0)`). Addons draw in true 1:1 window pixels regardless of whether the 3D scene uses 1x, 2x, or 4x render scaling.
3. **WASM Isolation**:
   - Addon modules and all four C/C++ dependencies (`mlua`, `dear-imgui-rs`, `dear-imgui-wgpu`, `dear-imgui-winit`) are isolated under `target.'cfg(not(target_arch = "wasm32"))'`.
   - Verified that browser client builds are completely untouched.
4. **Last-Registration-Wins Semantics**:
   - `UiForge.RegisterCallback` replaces the function reference in the Lua registry table `forge_callbacks` under `kind + 1`. Subsequent registrations cleanly overwrite earlier ones.

---

### 2. Deep Dive: Input Capture, Lifetimes & Safety

#### Input Capture Asymmetry (Material Finding)
In [`crates/eqoa/src/fork_ext/addons/spike.rs`](../crates/eqoa/src/fork_ext/addons/spike.rs#L111-L132):
```rust
match event {
    WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
        self.context.io().want_capture_keyboard()
    }
    WindowEvent::MouseInput { state: ElementState::Pressed, .. }
    | WindowEvent::MouseWheel { .. } => self.context.io().want_capture_mouse(),
    _ => false,
}
```
- **Behavior**: All events update ImGui via `handle_window_event`. However, only `ElementState::Pressed` returns `true` (swallowing the event). `ElementState::Released` always returns `false` and falls through to the game viewer.
- **Evaluation**: For the Stage 1 target (`screen_grid`), this has zero negative impact because the overlay is non-interactive. However, for interactive text input or modal UI in later phases, the game engine will receive key/mouse release events for actions it never saw pressed.
- **Recommendation for Stage 2**: Track active captures so that release events corresponding to consumed press events are also swallowed.

#### Context, Drop & Resource Lifetimes
1. **Drop Ordering**:
   - In `Spike`, field declaration order drops `script: Function` and `host: Host` before `renderer: WgpuRenderer` and `context: Context`.
   - `Drop for Spike` executes `self.save()` and `self.cleanup(4)` (`OnEject`) while the context and platform backend remain attached and functional, detaching window cleanly at the end.
2. **Deferred Texture Deallocation**:
   - In [`textures.rs`](../crates/eqoa/src/fork_ext/addons/textures.rs), calling `UiForge.ReleaseTexture` marks the entry `alive = false` but does not immediately unregister it from `WgpuRenderer`.
   - Actual deallocation is deferred to `collect()` after the render pass completes, eliminating GPU validation errors or use-after-free panics if an image was emitted earlier in the frame.
3. **Bridge Stack Recovery**:
   - In [`bridge.rs`](../crates/eqoa/src/fork_ext/addons/bridge.rs), `Frame` tracks active `Begin`/`End` windows and `Push`/`Pop` style colors up to depth 32.
   - If Lua execution panics or hits the instruction/memory budget, `Bridge::invoke` unwinds all pushed state down to the baseline before returning, preventing ImGui context corruption on subsequent frames.

---

### 3. Sandboxing & Bounded Budgets

| Budget Boundary | Limit | Enforcement Mechanism |
| :--- | :--- | :--- |
| **Lua Heap Memory** | 8 MiB | `lua.set_memory_limit(8 * 1024 * 1024)` |
| **Instruction Budget** | 200,000 inst | Lua debug hook triggers every 1,000 instructions |
| **Native Call Limit** | 4,096 calls/frame | Checked in `Frame::check()` on every bridge call |
| **Stack Nesting** | 32 levels | Checked in `Begin` and `PushStyleColor` |
| **Source / Profile Files** | 1 MiB max | Checked prior to read/parse and write |
| **Texture Allocations** | Max 16 textures | RGBA8 format only, dimensions restricted to 1..256 |

---

### 4. Deliberate Stage 1 Limits Acknowledged

1. **Single File Opt-In Only**: Evaluates `screen_grid.lua` exclusively via `EQOA_FORGE_SPIKE`. Drop-in folder discovery, multiple mods, and package manifests are deferred.
2. **No Raw FFI or Memory Access**: `require("ffi")` is rejected; `EEmem` and raw pointer casting are unavailable.
3. **Disabled Dynamic Lua Functions**: `pcall`, `xpcall`, `dofile`, `loadfile`, `loadstring`, and `package.loadlib` are removed to guarantee instruction budget termination cannot be caught in an infinite loop.
4. **Offline Shot Path Scope**: Standalone CLI `shot_with` does not execute addons; GPU fixture uses dedicated offscreen target.
5. **Native Maps Retained**: Confirms both native world map and minimap remain active defaults without replacement.
