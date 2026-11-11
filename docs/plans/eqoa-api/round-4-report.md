# Round 4 report

## Part A: map API (2026-11-11)

**Partial: items 1 and 2 implemented; stopped at item 3 under the brief's upstream-edit gate. Part A is not ready for Part B.** Branch `feat/eqoa-api-r4`, starting at develop `9e7257a`; reviewed upstream base `11aa5f088bb52d5f306b0224e37b782b1bf4503d`. Commits are LoopRook's, on this branch only. No push, merge or tag.

The shared brief was copied byte-for-byte and committed first (`68527d9`), together with the requested skinnable-map backlog entry. The spec addendum was committed next (`4fbd6a3`). Existing normative map rows remain in the spec. Additions describe the fixed owner decisions; they do not claim that every new function is implemented.

### Implemented API items and tests

1. **`eqoa.map.get()` and `map.changed`** (`27eb6e6`). The Play capture projects the actual player's world ID, native world name, location, calibrated native cell extent, and personal waypoint. Missing geometry stays nil. Snapshot revisions advance when the projected state changes, and unchanged publishes do not emit a new map event. Follow-up `9baed1b`: cached player-world calibration/name also survive the native Places flow moving that world into `Map::home`.
   - `play::fork_hooks::native_tests::map_state_comes_from_play_and_changed_event_has_negative_control`: starts with a ready Play and native Tunaria geometry (x 2000..28000, z 0..34000), checks Lua state and change events; identical capture emits no second event; uninitialized geometry becomes nil and emits a change; out-of-world getter returns `not_in_world`.
   - `map::extension::api_tests::map_state_keeps_player_world_calibration_while_browsing_places`: native Places-shaped world transfer retains the known extent/name; a different, uncached world has no fabricated bounds.
   - Meaningful RED observed without the map getter binding, then GREEN. An earlier fixture borrowed CORE twice and was corrected before the meaningful RED. The Places regression separately failed on missing calibration before its fix.
2. **`eqoa.map.markers(world_id)`** (`5e4e8d8`). Static camps/coaches/towns read the same pinned native arrays and are cached by requested world. Dynamic values come from the native quest marks, community cache, personal waypoints and group waypoint. Native names/coordinates are retained; camp levels and the first quest's optional levels are exposed; community notes provide label metadata. The API does not modify any native marker or waypoint.
   - `play::fork_hooks::native_tests::map_markers_use_native_quest_cache_and_filter_other_worlds`: starts from Play with protocol-shaped quest marks, native community/group records and a personal waypoint created through native map click; checks all seven marker kinds.
   - Negative controls: other-world quest excluded, non-integer/string world rejected, writes to returned marker records rejected. RED observed with the missing method; then both Play map regressions passed together (2 passed, 0 failed, 0 ignored, 0 SKIP).

The new implementation is in `fork_ext/map_api.rs`, `fork_ext/api_capture.rs`, and the existing fork-owned addon core, state, event and capability files. A three-line cfg/path/module declaration in `map/mod.rs` gives the fork-owned adapter private native-cache access. No renderer, input, minimap algorithm, native search or journal code was changed.

### Item 3 scope conflict and required continuation

The scout facts were rechecked after both syncs:

- `map/mini.rs:483` has one native minimap preparation function. It rejects the open M map at line 488, fixes indoor zoom to `Indoor::scale()` at line 516, and fixes the view center to the player at line 519. The actual player arrow is separately drawn at the center around lines 685-690.
- `map/mini.rs:752` draws those already-prepared native layers; it has no independent center/zoom/size input.
- `map/mini.rs:458` prepares the native indoor floor image, and returns while the M map is open. The floor renderer is `map/indoor.rs:281`; scene input reaches it from `viewer.rs:6297` through `Play::indoor_draw`.
- The permitted overlay seam can append guide drawing with an existing transform. It cannot parameterize the native minimap preparation algorithm. Supplying a synthetic player position would put the actual player's arrow at that position, still leave indoor zoom fixed, and misrepresent the requested view. Cropping a cached image cannot supply terrain/floor coverage outside it. Modifying/reidentifying upstream-generated UI quads after the fact would couple the adapter to private drawing order rather than provide the requested native primitive.

**Continuation needs permission for a neutral parameterized native minimap primitive in `map/mini.rs`, beyond the allowed overlay seam and minimap hide check.** Its explicit inputs should separate actual player pose from requested center, zoom and output rectangle; the ordinary native minimap should call it with its current defaults. Indoor view preparation must use the requested extent and remain available for a held view when the M map is open. Existing fork-owned viewer adapters can supply scene inputs; a new viewer hook is not assumed necessary. The fork-owned view service can then own output textures, quotas, liveness, frame deduplication, masking and rim policy. This is a proposed continuation, not an implemented or approved extra seam.

No `map.view` success/texture is fabricated, and no design decision was silently narrowed to a fixed player-centered screenshot. The explicit stop condition in the brief applies here.

### Items not implemented

| Item | Status |
|---|---|
| 3: native map views, circle mask, rim projection | Blocked as above; no renderer pipeline, projection API or view lifecycle yet. |
| 4: mod-owned guides, overlay seam, guide.lost | Not started after the gate. |
| 5: minimap hide/click suppression, moved layout rectangle | Not started after the gate; Move HUD remains intact. |
| 6: pointer state/events/rectangle claims | Not started; existing runtime event route was inspected. |
| 7: mod drawing, keys and text over M map | Not implemented or tested; existing addon input route remains intact. |
| 8: async packaged JSON and catalogue move | Not started; catalogue and its existing pin stay in their current location. |
| 9: conditional async JSON save | No 14 MiB Lua save measurement was run; no conclusion about the 2 ms condition and no new save API. |
| 10: journal-complete entity deltas | Not implemented/audited to completion; existing entity data was left intact. |

The pre-existing normative `map.images`, `map.load_image`, `map.set_waypoint` and `map.clear_waypoint` rows still have pending implementations. Parts B/C, bundled map mods, migration/removal of native map features, and three-map-mod frame timing were not attempted.

### Validation

All cargo commands run in the foreground under the shared mkdir lock with a 20-minute acquisition timeout, `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, and scratch `EQOA_CONFIG`. Long output is kept under ignored `.superpowers/sdd/round4/`. Counts of tests returning Ok after printing SKIP are reported separately from Rust's ignored count; these are not proof of their skipped workloads.

| Suite/check | Pass | Fail | Ignored | Explicit SKIP lines |
|---|---:|---:|---:|---:|
| Focused Play map API tests | 2 | 0 | 0 | 0 |
| `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1 --nocapture` | 378 | 0 | 24 | 9 |
| `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture` (final code) | 2060 | 0 | 31 | 208 |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS | 0 | n/a | 0 |
| `cargo build --locked --release -p eqoa` | PASS | 0 | n/a | 0 |
| Footprint against reviewed base | PASS | 0 | n/a | 0 |

WASM check completed in 29.31 s and the release build in 2 min 05 s. The initial full client run before the Places regression was 2059 passed, 0 failed, 31 ignored, 208 explicit SKIP lines. The addon skips were four unavailable Windows symlink-privilege checks, two missing pinned Forge spike/hearts fixtures, and three missing reviewed bundled UI picture goldens. The client adds missing local disc/module/live-S2 fixture skips. No skipped fixture is counted as validation of the missing data. The unchanged native bundled ARPG/Modern package load/save/disable test passed, as did Wereoxxs coverage and round 3's four-mod load and performance tests; picture-golden comparisons remain skipped. No visual change was introduced, so no new screenshot was required. Owner playtest and independent Part A review remain pending.

### Footprint and measurements

- `python tools/check_fork_footprint.py --base 11aa5f088bb52d5f306b0224e37b782b1bf4503d`: PASS. `map/mod.rs` is +48/-7 against that base, within the existing +70/-10 allowance. This task adds only its three adapter-declaration lines to shared files. **No budget increases.** Overall measured shared divergence: +5420/-269 across 43 files (starting branch +5417/-269).
- Dry-run `git merge-tree --write-tree --name-only develop origin/master` and the same comparison using feature HEAD both return two conflict paths: `play.rs` and `viewer.rs`. Local `origin/master` is `ced7208`. No map-path conflict was added; no merge was applied.
- Zero-addon allocation gate `play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound` ran with a 200-entity/200-ability fixture, 300 frame/menu iterations and 600 packet/chat iterations, asserting zero allocations; no SKIP path. Initial debug run: frame 12.603 us, menu 9.579 us, event 1.351 us; allocations and allocated bytes all zero. Final debug run: frame 18.782 us, menu 16.941 us, event 1.157 us; frame/menu/event/chat allocation and byte counts remain zero.
- Round 3's existing four-mod benchmark ran (not a missing-fixture success), with 22 native-art allocations per active frame after warmup; the existing benchmark passed. These are host-specific debug measurements, not a live-server FPS claim or a measurement of the unbuilt round 4 mods.

No new target directory, game data, credentials or decrypted modules were added. Global experience files outside the worktree were not edited by this worker. Worktree report is the handoff record; QA pending.
