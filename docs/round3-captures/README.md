# Round 3 native reference provenance

These eight PNGs were captured by executing **dde6ecc** (develop's native HUD),
not by reproducing that drawing on the mod branch. The temporary native test is
preserved verbatim in `native_capture.rs`; it calls `statbox::draw`,
`Hud::draw_target_of_target`, `member_gauges`, `target_gauges`, `draw_gauge`, and
native `fork_hud::draw`. Production native routines were unchanged. The test initializes the left/top entry
state set by native Hud::draw before calling its private routines.

Procedure (2026-11-10, Windows, GPU available):

1. `git -c safe.directory=* worktree add --detach %TEMP%/eqoa-r3-develop-native dde6ecc`
2. Copy `native_capture.rs` to `crates/eqoa/src/round3_native_capture.rs` in that
   worktree; append a test-only child module to `hud.rs`:
   `#[cfg(test)] #[path="round3_native_capture.rs"] mod round3_native_capture;`
3. With `EQOA_NATIVE_CAPTURE_OUT=%TEMP%/eqoa-r3-native-png`, scratch `EQOA_CONFIG`,
   and `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, acquire the shared
   mkdir lock and run (1800-second timeout):
   `cargo test --locked -p eqoa --bin eqoa capture_develop_native_round3 -- --test-threads=1 --nocapture`
4. Copy the PNGs here. Remove the temporary worktree with
   `git -c safe.directory=* worktree remove --force <that exact temporary path>`.

Result: **1 passed, 0 failed, 0 ignored, 0 SKIP**, 2.62 seconds test execution.
Full command log: `%TEMP%/eqoa-r3-native-capture-final.log`. No worktree target directory.
The capture uses the same white 1x1 surfaces, solid synthetic ArialUI-shaped
12x21 glyphs, atlas geometry and string table as the mod picture fixtures.
Game-shaped native states and their byte-valued gauges are in the capture source.
Both sizes (640x480 and 1280x720) are captured. No copyrighted disc art is stored.

Tests **read** these PNGs. Each element rectangle is compared independently:
channel tolerance 2/255; at most 0.1% differing pixels **per element**, with an
exact blank-background comparison outside them. This prevents the screen's blank
area from masking a broken small gauge or readout.

Source discrepancy: at this requested commit `statbox::draw` actually selects
`Anchor::Left`, not Right; numeric target green/yellow are [0.4,1,0.4] and
[1,0.9,0.3]. The fixtures preserve the pinned native behavior rather than the
review's conflicting description.
