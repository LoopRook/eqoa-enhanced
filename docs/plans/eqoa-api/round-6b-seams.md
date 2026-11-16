# Round 6B Upstream Seam Proposals

Per `FORK.md` and the Round 6 Part B brief, no new upstream lines were added outside `crates/eqoa/src/fork_ext/`. Actions that cannot be dispatched from `fork_ext/` without editing an upstream file remain `not_yet` and are recorded here for the lead/upstream review.

## 1. `eqoa.actions.camera` (one-shot camera style / heading / pitch / distance)

- **Why `fork_ext/` cannot dispatch it today:**
  `crates/eqoa/src/play.rs` receives `walker: &mut Walker` in `Play::frame`, then calls:
  - `crates/eqoa/src/play.rs:3325`: `fork_hooks::begin_iteration(self, game_frame);` (no `Walker` parameter)
  - `crates/eqoa/src/play.rs:3326`: `fork_hooks::frame_library(self, lib, Some(walker));` (immutable `Option<&Walker>`)
  Neither hook provides `&mut Walker` (`walker.cam` / `walker.scene`), and `Play` itself does not own `Walker`. Native camera changes (`crate::camstyle::set_style`, orbit angle / pitch / zoom setters on `WalkCamera`) require `&mut WalkCamera<Ee>` and `&mut impl Scene<Ee>` inside `Walker`.
- **Current status:** `eqoa.actions.camera` is `not_yet` with reason `"requires &mut Walker hook in Play::frame (proposed in round-6b-seams.md)"`.
- **Proposed 1-line upstream seam (`crates/eqoa/src/play.rs:3326`):**
  Either pass `Option<&mut Walker>` into a single camera hook after `begin_iteration`:
  ```rust
  fork_hooks::begin_iteration(self, game_frame);
  fork_hooks::apply_camera(self, walker);
  ```
  where `fork_hooks::apply_camera` is a no-op when zero camera requests are queued (zero cost on vanilla / zero-addon frames) and applies native `camstyle::set_style` / orbit pitch & zoom clamps through `vi::camera::WalkCamera` when a brokered `ActionKind::Camera` dispatches.
