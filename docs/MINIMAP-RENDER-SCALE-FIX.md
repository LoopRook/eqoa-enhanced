# Minimap and world map render scale fix

The circular minimap and full world map terrain, cells, and backdrops are
composed in physical window pixels (`self.size`), while UI quads use frame
coordinates (640x448) that scale to render target dimensions.

Previously, live draw passes (`Map::draw` and `Map::draw_mini`) passed
supersampled target dimensions (`f.size`) to map-layer quad normalization. At
render scales 2x and 4x, terrain quads and the backdrop quad shrank into the
upper-left quadrant of the screen because their pixel-space coordinates were
divided by the larger render-target dimensions. For the world map, this left
terrain and backdrop as a small block in the upper-left while full-size UI
labels and grid lines floated over the visible 3D game scene.

Both `Map::draw` and `Map::draw_mini` now obtain the actual render target
dimensions directly from their target texture views (`target.texture().width()`,
`height()`), normalize terrain and backdrop quads against the composition
window size (`self.size`), and scale UI quads and masks to the target. Call sites
in `viewer.rs` and `tool::capture` drop the redundant size argument. Offline map
captures and live frames are handled uniformly. Pointer hit testing continues
to use physical window coordinates.

## Verification

- **Minimap GPU regression**:
  - Before fix: `cargo test --locked -p eqoa map::mini::tests::minimap_stays_in_window_coordinates_at_render_scales -- --nocapture` failed with `center missing at scale 2` after the x1 control passed.
  - After fix: Passed 13/13 tests across scales [1, 2, 4, 2, 1], verifying terrain, mask clipping, UI marker, unmasked control, and hit testing.
- **World map GPU regression**:
  - Before fix: `cargo test --locked -p eqoa map::tests::world_map_stays_in_window_coordinates_at_render_scales -- --nocapture` failed with `backdrop bottom-right missing at scale 2`.
  - After fix: Passed across scales [1, 2, 4, 2, 1] on both symmetric (64x64) and asymmetric wide (80x48) viewports, verifying full backdrop coverage, terrain positioning, and UI quad alignment.
- **Focused test suite**:
  - `cargo test --locked -p eqoa map:: -- --nocapture` passed 114 tests cleanly.
