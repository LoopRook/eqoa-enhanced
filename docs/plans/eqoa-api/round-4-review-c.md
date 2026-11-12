APPROVE WITH CHANGES

Read-only review of `3a096b7..c9a2dca`, compared with native minimap reference `9e7257a` and upstream `11aa5f0`. No Cargo, source mutations, branch operations, or test negative controls were run by this reviewer. Only this report was written. Findings below distinguish source evidence from the builder/checker's execution evidence.

## HIGH

None found.

## MEDIUM

### M1 — Hover selection uses obsolete observations for 30 rendered frames

`bundled-mods/eqoa_round_minimap/main.lua:66`, `:80`, `:87`, `:94`.

The scan caches positions, names, levels and kind, and receives no entity-change/removal invalidation. Every intervening hit test projects those cached positions, although native dots move each frame. Scenario: hover two nearby moving creatures; the nearest live dot moves away or despawns after a scan. Its old location still produces its card, and hovering its new dot can produce no card or the other creature's card for up to 29 frames (almost a second at 30 FPS). Distance is also calculated from the obsolete position. The old native `dot_under` selected from that frame's observations with distance/id tie-breaking. The new tie-break is correct only for its stale input.

Cache immutable presentation data and candidate IDs, but refresh positions/validity from current observations or consume bounded entity deltas before choosing the nearest dot. Test movement, despawn and an equal-distance ID tie through the real mod runtime.

### M2 — Marker hover does not represent the visible minimap layer

`bundled-mods/eqoa_round_minimap/main.lua:116-128`; `crates/eqoa/src/fork_ext/map_api.rs:79-113`.

`mark_under` scans the general marker catalogue, takes its first hit, and projects original coordinates. It does not use the native layer's enabled filters, placed/rim coordinates or topmost ordering. Scenario: turn Map Markers off and hover an XP camp's coordinate: its card can still appear over blank terrain. Town entries are also hoverable although native minimap intentionally omits town labels. At overlapping camp/quest coordinates, catalogue order can select the camp despite the quest being drawn on top. Conversely an axis-aligned offscreen quest step is drawn on the native rim, but hovering that visible `?` cannot find it because the mod tests its original offscreen coordinates.

Expose or derive the actual visible, placed minimap marks and their card data. Test markers off, absent town labels, overlapping quest/camp priority and a rim-step card. Rich camp/quest cards are already acknowledged as reduced in the report; this finding concerns selecting invisible or incorrect marks, beyond that acknowledged text reduction.

### M3 — Moving the minimap can move its entire hover card outside the window

`bundled-mods/eqoa_round_minimap/main.lua:164-172`.

Card x is always `circle_left - width - 14`, with no left-window clamp; y has a top clamp but no bottom clamp. Long text is neither wrapped nor width-bounded. Scenario: use Move HUD to place the circle near the left edge, then hover a dot. The entire card is placed at negative x and becomes unreadable. Near the bottom edge, its lower lines can disappear. The replaced native card explicitly wrapped lines and clamped both axes to the window.

Restore bounded wrapping and window-aware placement, including a right-side fallback where appropriate. Extend the movement test to hover after dragging to left/bottom edges and use a long server-provided name.

### M4 — The benchmark does not exercise 200 native dots or establish the requested native baseline

`crates/eqoa/src/fork_ext/addons/round_minimap_manager_tests.rs:73`, `:83`, `:380-414`; `crates/eqoa/src/fork_ext/map_api.rs:170-177`; `docs/plans/eqoa-api/round-4-report.md:356`.

The Lua snapshot contains 200 entities, but the separately rendered scene calls `mini_frame` with `MiniView::default()`, a fixed player centre, no group and no target. It then covers the frame with a green terrain stand-in. The reviewed hover-card PNG actually shows a card without its creature dot. Thus a regression in native dot preparation/drawing, real marker layering or alignment can pass this workload. The frame timer also includes newly allocating output/depth targets every sample and stops after GPU submission, rather than measuring GPU completion. It is a useful fixture CPU comparison, not evidence for the full busy-world rendering cost. The mandatory develop-native round-minimap-plus-search comparison is explicitly absent.

Feed matching Play-derived observations into the rendered scene, assert native dots actually render and align with hover, reuse normal render targets, and compare the same CPU/GPU measurement boundaries against the native reference. Keep the existing reported values labelled as fixture measurements until then. The checker proved the existing timing workload executed; it did not resolve this workload mismatch.

### M5 — The replacement has no minimap render-scale regression coverage

`crates/eqoa/src/fork_ext/addons/round_minimap_manager_tests.rs:14`, `:81`, `:109`, `:212`; deleted `map/mini.rs::minimap_stays_in_window_coordinates_at_render_scales` and `fork_ext/map_sync_tests.rs::an_indoor_picture_is_clipped_by_the_round_minimap_at_every_render_scale`.

All new mod captures use one 1024x768 window and pass identical window/target sizes. The migrated scale test exercises the world map, not this capture/image/ring/hit path. Scenario: a render-scale change misaligns the round image, ring or hover at 2x/4x; all five replacement minimap tests remain green. This loses coverage for the specifically authorized scaling fix in FORK.md. An indoor disc fixture is unavailable, but the deleted synthetic clipping/control tests did not require proving real disc terrain to verify coordinate scaling.

Re-add a mod-level matrix with differing window and 1x/2x/4x target dimensions, wide/tall windows, moved placement, image/ring alignment, pointer hits and an indoor-shaped synthetic picture. Report real indoor terrain validation separately as unavailable.

## LOW

- `bundled-mods/eqoa_round_minimap/main.lua:151`: the added diagonal group waypoint is blue `(0.3,0.75,1)`; the native group waypoint is orange `(1,0.5,0.05,1)` (`map/group_waypoint.rs:37`). A group pin changes semantic colour as its bearing crosses the crop threshold. Use the existing native colour and test both bearings.
- `main.lua:43-52`: pointer strips use the widest cross-section within each strip, so their rectangles claim a small area outside the mathematical circle. At the top strip this is conspicuous relative to a zero-width pole. An outside click there can be consumed despite no visible map. Use an exact circle claim or document/test the approximation.
- There is no `pointer.pressed` action in the round mod: clicking the circle consumes the click without opening M. This is disclosed by the builder but loses native interaction. Restore it through a neutral map-open API or obtain explicit acceptance of that parity gap.
- Quest-step diagonal rim loss is real and disclosed. The brief explicitly permits recording the corner limitation when only `layers="all"` exists, so I do not treat that alone as a new blocking implementation defect. It remains a player-visible loss from the reference. Likewise the north label and rich marker cards are not fully ported. Badge restyle removal and the upstream M-map legend are intentional decisions.
- The packaging report's final paragraph honestly discloses bypassing the CRT import check in a scratch runner. That is checksum/assembly evidence, not a complete unmodified release gate. A normal CI/static-CRT build still needs to pass packaging before distribution.

## Remaining requested checks

**Parity that is structurally preserved:** the same 112-frame-pixel rectangle, six-pixel inset and 100-pixel picture follow HUD layout; `hud.image` corrects unequal frame-axis scaling into a physical circle. The ring uses that same centre/radius. Native dots, name colours, target and group remain in the captured native image; the mod does not redraw 200 dots in Lua. N uses stock minimap readiness, and wheel delegates to native indoor/outdoor zoom. Search now follows native minimap readiness while the stock layer is hidden; guide quads enter the captured image through the overlay seam. Mod-off routing returns to upstream square output/clicks. The available PNG confirms the basic ring/card presentation, not real terrain, indoor geometry or dot parity.

**Performance and lifetime:** one held view; capture texture/depth recreated on window/format changes, not per frame. Lease close/drop/VM release invalidates handles; device reset clears shared state. Native capture routing has a per-frame draw guard and the new tests assert one draw per active frame and stock restoration on disable. The capture is a full-window attachment capped at 2048 pixels on its longest side (`map_views.rs:374`), not a 112px target; 2x/4x main render scale does not increase its resolution. This is a deliberate quality/cost tradeoff requiring visual verification. A texture view is created each frame (`:409`); it could be held in Canvas. Lua allocates geometry/image/UV/claim tables each frame, texture info allocates a wrapper/UV table, and hovered cards allocate strings/lines. Markers are correctly cached by map change/world; entity scans avoid rebuilding the full list every frame but introduce M1 and periodic allocation/GC bursts. Profile those bursts rather than treating amortized means as latency evidence.

**Three upstream-file seams:** independently compared with `11aa5f0`: `map/mini.rs:560` +1 overlay; `map/mod.rs:88` native accessor module plus `:2044` overlay, +4 total; `render/src/ui.rs:588` public `Font::find`, +1/-1. All other requested map/render family paths are identical. These are neutral, small and appropriately delegate to fork-owned implementations. The glyph accessor supports the allocation-free addon text path.

**Four fixes beyond the brief:** opaque-black capture clear (`map_views.rs:420`) correctly restores visible alpha for RGB-only native pipelines; GPU image readback tests now cover actual capture visibility. It changes unmapped-ground transparency, explicitly disclosed. Text-editor suppression (`input.rs:241`, `native_state.rs:1219`) correctly blocks other native bind callbacks while allowing the native editor's own VM; existing `fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring` covers Forge chat suppression, though a direct two-native-VM owner/non-owner test would be stronger. Missing private journal mapping (`async_json.rs:337`) agrees with sandbox's `file not found` errors and native's silent `not_found` policy; the existing manager map-mod test asserts empty diagnostics on first load. Search readiness (`eqoa_map_search/main.lua:61`) correctly avoids `hud.visible` while a different mod hides stock; the new real-manager search test covers this. I inspected these tests, but did not execute them.

**Deleted tests:** I agree with the checker's deleted-assertion assessment. It is correct to drop fork legend/restyle tests where the brief deliberately restores upstream behavior; it is not sufficient to drop behavior now owned by the mod. Re-add mod-runtime equivalents of `minimap_card_calls_other_players_players_and_pets_pets`, `map_refinement_hover_chooses_the_nearest_visible_entity_and_keeps_level`, `minimap_entity_tooltips_keep_server_names_levels_and_validity`, `minimap_stays_in_window_coordinates_at_render_scales`, `border_outer_points_match_the_gpu_circle_at_every_aspect`, `diagonal_guidance_uses_a_circle_rim_and_corners_are_outside`, `an_indoor_picture_is_clipped_by_the_round_minimap_at_every_render_scale`, and the hover/search/hit assertions from `moved_minimap_moves_circle_border_hover_and_search`. Retain a fork-owned equivalent of `master_off_releases_world_and_closes_search_without_losing_saved_waypoints`; it remains an integration behavior. GPU circle tests should now cover the image boundary and unmasked controls instead of deleted renderer APIs. The world-map scale test was successfully relocated, not lost. The new single stationary NPC card test does not replace the player/pet/validity/nearest tests.

**Packaging/footprint:** independently hashed all five new minimap files and changed search `main.lua`: all match SOURCE.txt. New files are in release/test allowlists and the embedded installer; manifest is default-on. Independently ran only the read-only Python footprint checker: PASS against `11aa5f0`; current shared-file totals sum to the reported +1007/-209. Map-family seam measurements agree with the report/checker. Historical start totals and merge-conflict counts were not independently recomputed; no merge-tree object writes were performed. No claim of fresh Cargo, release, WASM, packaging dry-run or negative-control execution is made here.
