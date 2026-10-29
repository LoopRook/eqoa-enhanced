# Minimap render scale fix

The minimap terrain and circle were composed in physical window pixels, but
the live draw passed supersampled target dimensions to their normalization.
At x2 and x4 the terrain and mask consequently shrank and moved. HUD quads
use frame coordinates, while their circular mask also needs target pixels.
Pointer hit testing continues to use physical window coordinates.

The minimap now obtains the actual render target dimensions from its texture
view, normalizes terrain against the window size, and scales the UI mask to
the target. This also handles the offline shot path, whose callback formerly
passed physical dimensions despite drawing into the larger target. The full
map and other renderer paths are unchanged.

## Verification

- Before the fix, `cargo test --locked -p eqoa minimap_stays_in_window_coordinates_at_render_scales -- --nocapture` failed with `center missing at scale 2` after the x1 control passed.
- After the target-size follow-up, `cargo test --locked -p eqoa map::mini::tests -- --nocapture` passed 13 tests. The synthetic GPU regression checks terrain, clipping, a masked UI marker, an unmasked control, window-coordinate hits, and x1 -> x2 -> x4 -> x2 -> x1. It uses no disc or account.
- `cargo test --locked --workspace` was stopped while `ee`'s exhaustive `unary_over_every_bit_pattern` was still running after more than a minute. The full suite was not completed.
- A live gameplay comparison and disc-based captures have not been performed. The GPU regression logs a skip if an adapter or device is unavailable.
