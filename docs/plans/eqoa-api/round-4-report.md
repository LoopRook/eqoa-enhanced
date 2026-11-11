# Round 4 report

## Part A: map API (2026-11-11)

The API surface for items 1-10 is implemented, using the lead's player-centred native-minimap rescope for item 3. Independent review, runtime freshness fixes described below and owner playtest remain pending. Branch `feat/eqoa-api-r4`, from develop `9e7257a`; reviewed footprint base `11aa5f088bb52d5f306b0224e37b782b1bf4503d`. Commits are LoopRook's on this branch only. No push, merge or tag.

The shared brief and skinnable-map backlog entry were committed first (`68527d9`); the spec addendum followed (`4fbd6a3`). Each API step has its own implementation/test commit. Item 5 preceded item 3 as its hide/layout prerequisite. All implementation is fork-owned apart from neutral native access/composition/caller seams.

### API items and regression evidence

Test names below are suffixes under `play::fork_hooks::native_tests` or `fork_ext::addons`; all named new workloads ran locally without a SKIP. RED means an observed failure before the corresponding implementation or missing source path was added.

| Item | What changed | Tests and negative controls |
|---|---|---|
| 1 | Native `map.get()` and revisioned `map.changed`, calibrated to the player's world even when browsing Places. Unknown geometry stays nil. | `map_state_comes_from_play_and_changed_event_has_negative_control`: Play state, no event on identical publish, reset/pregame rejection. `map_state_keeps_player_world_calibration_while_browsing_places`: native home-world fallback, unknown-world control. Both observed RED then GREEN. |
| 2 | Read-only world-filtered markers from native camp/coach/town tables and quest/community/personal/group-waypoint caches; static records cached. | `map_markers_use_native_quest_cache_and_filter_other_worlds`: all seven kinds from native/protocol-shaped fixtures; other-world exclusion, invalid world, mutation rejection. RED missing method then GREEN. |
| 3 | `map.view{size?}` lazily exposes the unchanged native minimap drawing in an owned texture while a mod holds a view and hides stock minimap. Native player centre, zoom/wheel and indoor scale remain authoritative. `project`/`unproject` use the real transform. One native draw per frame, stock OR capture; no extra target/draw with no view. HUD owned images support a physical circle mask. | `round4_map_view_projects_native_player_scale_and_rejects_pregame`: native player/scale, projection/inverse, out-of-bounds and removed-center argument controls. `round4_native_minimap_route_is_exclusive_and_no_view_has_no_capture`: exclusive routes and zero-view control. `round4_owned_texture_circle_masks_corners_but_square_keeps_them`: real GPU readback, centre/corner/background/square controls; strengthened physical-axis assertion observed RED on a capsule, GREEN after UV-preserving square crop. |
| 4 | VM-owned guides with dashed line/dots, moving entity/group resolution, owner-only `guide.lost`, disable cleanup. The same fork overlay function is called once from each native map composition. Personal/group waypoints are separate. | `round4_guide_follows_live_entity_and_lost_is_owner_scoped`: moving native entity updates target; removal emits once to owner, other VM receives none. |
| 5 | Minimap hide/show lease hides draw and clicks, preserves compass, and releases on disable. `hud.layout()` follows native Move HUD placement/reset. | `round4_minimap_claim_uses_play_health_and_releases_without_hiding_compass`; `round4_minimap_layout_tracks_upstream_drag_and_reset`: show/release and unrelated HUD-piece controls. |
| 6 | HUD-coordinate pointer state, pressed/wheel events and bounded VM rectangle claims through the existing input path. Claims expire, Move HUD takes precedence, and `input.minimap_wheel` forwards claimed steps to native indoor/outdoor zoom. | `round4_pointer_claim_routes_window_coordinates_and_expires`: native window/frame conversion, outside rectangle, expiry, one-shot wheel drain. |
| 7 | Existing addon key/text eligibility permits open M map, with chat/search/modal blockers. Native mod HUD is composited through the existing final addon-overlay adapter; ImGui retains that route. No new shared seam. | `round4_addon_input_and_render_continue_over_map_but_not_chat_or_search`: open-map render/text capture, native map stays open, map search/native search/chat rejection. RED on prior eligibility then GREEN. |
| 8 | Bounded worker reads/decodes read-only packaged/shared JSON above 1 MiB; request-correlated resource success/failure delivers mutable Lua tables. Mod catalogue is added under `mods/resources`. | `round4_async_json_loads_large_readonly_resource_and_reports_failure`: >1 MiB payload, malformed JSON, traversal/private-mount rejection, worker-thread identity. RED missing loader then GREEN. `round4_shared_catalogue_retains_forge_pin`: >1 MiB catalogue, exact SHA-256 and changed-byte negative control. |
| 9 | Conditional saver enabled by measured Lua cost. Finite Lua snapshot on caller; native encoding and atomic private write on worker, latest pending value wins. | `round4_measure_lua_journal_save_14_mib` asserts 62,000 records and exactly 14 MiB saved. `round4_async_json_save_is_private_and_latest_value_wins`: near-14 MiB saved value, latest stage, worker identity, package/traversal/cycle controls; RED missing method then GREEN. `round4_save_queue_coalesces_and_rejects_another_path` and `round4_json_encoder_caps_output_and_preserves_arrays_null`: deterministic coalescing, alternate-path, size/NaN/array/null controls. |
| 10 | Added protocol world/type, creature boolean, corpse bit and creature p255 search flags to snapshots, typed deltas and packet fingerprints. Existing id/name/level/position/class_code cover remaining journal inputs. | `round4_entity_journal_flags_are_protocol_values_and_corpse_delta_is_not_hp`: Play/protocol records and real packet boundary; zero HP without corpse, different world/noncreature with nil flags, no event for unchanged packet. RED missing fields and then missing fingerprint inputs; GREEN after both paths were covered. |

### Rescope and native footprint

Rechecked after the upstream syncs: `map/mini.rs:483` prepares the native minimap, `:753` draws it. Native layers and UI are not independently drawable at the permitted caller without depending on internal quad ordering. Views therefore accept `layers="all"` only. No arbitrary centre/zoom, map-only layer, rim-mark clamp or ring option is offered. A circular image crops marks near the native square's corners. Native minimap readiness and M-map/indoor preparation rules still apply; `view.texture` is nil until a usable capture exists.

The call-site redirect is `viewer.rs:6391`; minimap hit wrappers replace the scripted/live wheel and Play click checks. The private-cache accessor is a cfg/path module declared in `map/mod.rs`. The single overlay function is called at `map/mod.rs:2080` and `map/mini.rs:654`. Native minimap drawing/preparation algorithms and `render/map.rs`/`map.wgsl` were not changed by Part A.

Footprint: **PASS**, total shared divergence **+5425/-272 across 43 paths**, versus starting branch +5417/-269. This part adds +8/-3 to that measured divergence. `map/mod.rs` +49/-7 (budget +70/-10), `map/mini.rs` +267/-14 (+313/-44), `play.rs` +262/-60 (+278/-62). The only budget change is `viewer.rs` **+101/-11 -> +104/-14**, exactly the three authorized caller replacements for wheel-hit hide checks and stock/offscreen draw routing. No other budget was raised. The accessor/overlay additions fit existing map allowances. Parts B/C still own removal of native search/journal/circle/theme code and their footprint reduction.

Views are capped at four per VM with logical size 16-1024. One shared capture target is
bounded to 2048 pixels per axis (colour plus depth); owned texture registrations are
charged against the existing per-VM 64 MiB / 128-handle addon texture budget, which was
not raised. Pointer claims are capped at 32 rectangles per VM/frame; guides cap dots at
80,000 and labels at 128 bytes. These are explicit bounds, not a measured worst-case
frame-time guarantee for the later mods.

### JSON limits and measurements

Read: 16 MiB, two requests in flight per VM, root object/array, depth 32 and two million JSON values. Decode/read runs on the worker; Lua materialization runs at dispatch on the VM thread. JSON-enabled VMs opt into a **128 MiB** Lua heap from the ordinary **8 MiB** limit, to accommodate catalogue/journal tables. No JSON worker exists before use. This Lua limit does not include native parser/snapshot memory.

Save: string/key snapshot bytes and encoded output each at most 14 MiB, depth/node limits as above; existing aggregate private storage stays 16 MiB. One active write plus one replaceable pending value per VM/path; a different path is rejected until it drains. Only the latest coalesced completion is guaranteed. Accepted work drains on VM drop. Snapshots contain no Lua references, and encoded output is bounded while writing. No package writes.

The initial Lua baseline encoded and wrote a journal-shaped table with 62,000 entries to exactly **14,680,064 bytes** (whitespace padding to exact size): **1794.263 ms**, above the 2 ms condition. It uses long synthetic name keys, the real Lua interpreter and synchronous binary I/O in an isolated test VM; it is synthetic, not a live player's journal. A near-cap async fixture saves a **14,679,936-byte payload** and asserts the final third value. Three synchronous snapshot calls initially measured **12.580 ms combined**. Encoding/write is off-thread; caller snapshot copying is still synchronous and is not promised to fit 2 ms. Final gate measurements are below.

The shared catalogue pin remains `37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca`. The native embedded copy remains temporarily because moving its include would edit a map source outside Part A's permitted seams. Part B removes native search and that duplicate. No mod was written here.

### Validation

All cargo commands ran in the foreground under the shared mkdir lock (20-minute acquisition timeout), `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, and scratch `EQOA_CONFIG`. Long logs remain under ignored `.superpowers/sdd/round4/`. Explicit SKIP lines are counted separately from Rust ignored tests and are not evidence that those workloads ran.

| Final suite/check | Pass | Fail | Ignored | Explicit SKIP |
|---|---:|---:|---:|---:|
| Addons: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1 --nocapture` | 389 | 0 | 24 | 9 |
| Full client: `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture` | 2075 | 0 | 31 | 208 |
| New `round4_*` regression subset within full client | 15 | 0 | 0 | 0 |
| Existing map get/markers/home-world regression subset within full client | 3 | 0 | 0 | 0 |
| Zero-addon allocation benchmark within full client | 1 | 0 | 0 | 0 |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS | 0 | n/a | 0 |
| `cargo build --locked --release -p eqoa` | PASS | 0 | n/a | 0 |
| Footprint against reviewed base | PASS | 0 | n/a | 0 |
| `git diff 9e7257a --check` | PASS | 0 | n/a | 0 |

Addon suite: 130.03 s; full client: 221.25 s; WASM: 9.82 s; release: 2 min 00 s. An initial addon run before the stronger mask check also passed 389/0/24 with 9 SKIPs; the stronger isolated GPU check then failed on the capsule, and the corrected code passed the addon and full client gates above. The release will be repeated after this report commit, as required.

The nine addon SKIPs are four Windows symlink-privilege cases, missing pinned Forge grid/hearts fixtures, and three missing reviewed bundled UI picture goldens. The full suite additionally lacks local disc/module/live-S2 fixtures. Existing bundled Wereoxxs, ARPG and Modern UI load/save/disable tests passed. Round 3's four-mod fixtures and performance test passed, with 22 native-art allocations per active frame after warmup (UTF-8/queue/text-draw allocations zero). Picture-golden comparisons remain skipped.

`performance_busy_world_zero_addons_frame_and_packet_allocation_bound` asserted its 200-entity / 200-ability workload, 300 frame/menu iterations and 600 packet/chat iterations. Final debug measurements: frame **16.440 us**, menu **15.604 us**, event **1.746 us**, chat **0.140 us**; allocations and allocated bytes were **zero** in all four paths. No SKIP. These are host-specific fixtures, not live-server FPS claims.

Final full-client JSON measurement: 14 MiB Lua encode/write **1764.990 ms**; three near-14 MiB native snapshot calls **13.360 ms combined**. The final addon run measured 2135.340 ms and 12.165 ms respectively. This variability reinforces that the async API removes native encoding/I/O from the frame but does not guarantee a sub-2-ms snapshot.


### Handoff and remaining work

**Known runtime freshness gap, deferred to the requested later fix session:** the new
map fixtures explicitly publish `Play::native_view`, whereas the optimized runtime
`api_updates::refresh` normally avoids a full publish after entry. Map state/markers and
the minimap projection currently refresh through that full-publish path; ordinary
incremental frames can leave those values stale. Guide lost-event cleanup also runs at
publish, so disappearance through a packet delta is not covered by the committed guide
test. The entity packet fields in item 10 do use the incremental path and are tested
there. These item-1-6 gaps were noticed during report preparation without reading the
parallel review. They are not claimed fixed or covered by the green suite; resolve
them in the later fix session before the map mods rely on the API.

**Part B integration limit:** `load_json` deliberately accepts only the read-only
mounts fixed by the brief. Existing synchronous private file readers still have their
1 MiB limit. A saved journal above that size therefore needs an explicitly designed
private JSON loading/migration path before the journal mod can reload it; this part
does not silently widen the read-only loader's authority.

The deterministic GPU mask fixture is a synthetic owned texture, not proof of real-world terrain/indoor captures or live pointer routing. Owner playtest should cover native outdoor/indoor captures, zoom, held-view lifecycle, search-box layering over M map and disable/device recovery. The capture is linked below. Reviewed bundled UI picture goldens, pinned Forge spike/hearts, Windows symlink privileges and local disc/module/live-server prerequisites were unavailable as itemized by SKIP counts; skipped workloads are not evidence.

![Physical circle mask and square control](round-4-circle-mask.png)

Pre-existing `map.images`, `map.load_image`, `map.set_waypoint` and `map.clear_waypoint` remain pending implementations outside this round's numbered additions. No native search/journal removal, mods, migration, three-map-mod timing or map skin was done. The skinnable-map backlog entry remains for the round-8 neutral seam proposal.

The separately produced `round-4-review-a1.md` was left untouched and uncommitted; its findings were not consumed or applied. The GPU physical-circle correction came from this session's own capture and failing pixel control. No protected branch, game data, credential, per-worktree target directory or global experience file was changed.
