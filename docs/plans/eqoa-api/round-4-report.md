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


## Part A fix round (2026-11-11)

All HIGH and MEDIUM findings in reviews a1/a2 are resolved. The unchanged reviews
were committed first as `e79bad4`. Work stayed on `feat/eqoa-api-r4`, as LoopRook;
no push, merge, tag, protected-branch change, game data or per-worktree target was made.
The sections above describe the original Part A state; this section supersedes their
freshness, JSON permission, shutdown and pending-review implementation limitations.
Independent re-review and owner playtest are still pending.

### Findings, implementation and regression evidence

Tests below use Play/walker or protocol-shaped fixtures for map integration, and
public Lua file calls with actual sandbox mounts/worker completion dispatch for JSON.
The final minimap projection fixture now enters through `forge_snapshot`, not a
hand-published view. RED means an observed failing run, not an inferred control.
Logs remain under ignored `.superpowers/sdd/round4/fix-*.log`.

| Finding | Fix and commits | Behaviour test and observed negative control |
|---|---|---|
| a1 H1 | `498537f`, `46ea2f9`: allocation-free source fingerprints gate map/marker rebuilding; native projection is checked live for player movement, scale, window and Move HUD. Cached Forge snapshots invalidate when these change. | `round4_live_map_refreshes_waypoints_markers_projection_and_window`: entry once, then quest/group/personal waypoint changes, movement, zoom, wide window and saved Move HUD placement. Identical refresh emits no extra map change. RED on the original incremental path; GREEN through the real refresh path. |
| a1 H2 | `498537f`, `f922583`: expire guides after the real deferred entity/group batch, as well as refresh/reset. | `round4_guide_follows_live_entity_and_lost_is_owner_scoped`: native entity mutation/removal through capture/flush, movement followed, exactly one lost event to its owner. RED without live expiration; GREEN after it. |
| a1 H3 | No new budget increase: the accepted viewer allowance already exists at the starting head. Its recorded reason covers exactly the three caller replacements. | Footprint PASS; `viewer.rs` measured and allowed **+104/-14**. No shared-file or budget changes in this fix round. |
| a1 M1 | `4848b8d`: minimap excluded from all/DisableUI/GetHiddenBy expansion; it remains explicitly hideable. Original Wereoxxs minimap-visible expectation restored. | `round4_minimap_claim_uses_play_health_and_releases_without_hiding_compass`, existing bundled Wereoxxs enable/disable test. All-hide assertion observed RED with minimap included; final addon/client suites GREEN. |
| a1 M2 | `f922583`, `ef80dfb`, `46ea2f9`: actual native-draw invocation instrumentation and real GPU capture/lease tests; input ownership and runtime device recovery exercised. | `round4_live_capture_quota_lifecycle_and_device_cleanup`, `round4_wide_pointer_layout_and_observed_press_share_coordinates`, `round4_device_recovery_flushes_json_and_releases_views_claims_guides_and_options`. Removing draw dedupe failed the call count; omitting VM cleanup failed lifecycle checks; omitting Move HUD precedence failed pointer routing. Capture is absent without a lease, stock/capture are exclusive, resize invalidates the old texture, disable frees canvas/leases, and device recovery releases claims/guides. |
| a1 M3 | `185148d`: pointer x/y, plain claims and layout x/y/width/height now share **physical window pixels**. Legacy frame_x/frame_y retain their existing native HUD anchors. | `round4_wide_pointer_layout_and_observed_press_share_coordinates`, `round4_pointer_claim_routes_window_coordinates_and_expires`. Observed RED with central-frame pointer conversion; GREEN with a 1920x1080 window, direct layout claims, exact coordinate comparisons, outside-click control and focus loss. |
| a1 M4 | `4848b8d`, `f922583`: texture quota failure yields nil and a diagnostic, with `view.error` and `view.get_texture()` providing the Error without a Lua exception. | `round4_live_texture_quota_returns_nil_error`: real captured target, bank quota zero, nil/Error, then successful retry after quota restoration. Reinstating the throwing quota path observed RED; GREEN after restoration. |
| a1 M5 | `4848b8d`, `f922583`: inside is measured against the physical visible circle. | `round4_map_view_projects_native_player_scale_and_rejects_pregame`: corner inside the square but outside the circle is false; player-centre/inverse and ordinary inside controls remain. Original square comparison observed RED. |
| a2 H1 + M4 | `771f5f0`, `ef80dfb`, `14ad71f`: one process-wide writer, per-path acceptance generations, bounded wake notifications, shared Windows case-alias keys, and a final commit/cancellation gate. Disable/shutdown/drop flush for up to 2 s and join the idle writer; a slow closing writer cannot start a second writer. Completed generation entries retire. | `round4_json_private_journal_flushes_on_drop_and_reloads`, `round4_save_generations_reject_old_commit_and_disable_flushes`, `round4_save_timeout_is_bounded_and_cancels_late_rename`, runtime recovery test above. Near-14 MiB queued just before drop is on disk afterward. A paused old commit fails after a newer generation is accepted, including Windows case aliases. Disable waits for the last value before returning; deleting it afterward is not undone. A deliberately stalled write returns within the tested 2-2.5 s window and cannot rename later. Observed RED without drain, generation check, cancellation, or Windows alias normalization; GREEN after fixes. |
| a2 M1 | `771f5f0`, `14ad71f`: only manifest capability `large_data` grants 128 MiB, at startup. JSON calls do not change the ordinary 8 MiB limit. Structural preflight bounds node/key/depth growth before serde allocation. | `round4_json_calls_keep_ordinary_heap_and_manifest_opt_in`, runtime manifest/recovery test, `round4_json_nodes_are_enforced`. Ordinary heap test observed RED on the unconditional raise. Manifest-backed runtime saves the near-cap journal with the explicit allowance; ordinary calls remain at 8 MiB. |
| a2 M2 | `771f5f0`, `14ad71f`: materialisation errors release the partial tree, collect it, and deliver correlated resource.failed; the host continues. Disable also drops partially materialised JSON and worker channels. | `round4_json_memory_failure_delivers_failed_without_faulting_host`, `round4_disabling_releases_a_partially_materialised_json_tree`. Both observed RED before their respective fixes; final suites GREEN. The memory-failure fixture runs under a deliberately small heap and executes Lua afterward. |
| a2 M3 | `771f5f0`, `ef80dfb`: one materialisation at a time, at most 512 values or 1 ms per dispatch; cooperative checks between values. | `round4_catalogue_materialisation_benchmark_asserts_real_workload`: actual 1,743,225-byte catalogue, exactly 70,579 values. Release baseline 101.794 ms; sliced release maximum 1.079 ms across 162 dispatches. Unsliced mutation observed RED (78 dispatches, 372.453 ms maximum in debug). Final sliced release test asserts both workload/slicing and maximum below 4 ms. |
| a2 M5 | `f922583`, `46ea2f9`: production overlay composition is shared with its behavioural test; map-open key ownership uses the actual capture policy and ownership state machine. | `round4_map_text_capture_owns_keys_and_releases_but_unclaimed_keys_pass`, `round4_map_overlay_consumes_real_mod_commands_and_restores_native_ui`. P/M/O/G/Enter are consumed while text capture owns them; releases follow the owner; unclaimed keys pass. Real Lua panel commands reach the final native overlay, stock quads/shift are restored, closed-map commands remain for the stock HUD, and no empty pass runs. Removing capture policy and removing composition/empty-pass suppression each observed RED. |
| a2 M6 | `771f5f0`: load_json also accepts the mod's own private names through the existing sandbox, with the same read/depth/node caps. Resource mounts remain read-only. | `round4_json_private_journal_flushes_on_drop_and_reloads`: >13 MiB journal, fresh host, exact stage/payload reload. Private-path denial mutation observed RED; final suites GREEN. Traversal/resource-write controls remain. |
| a2 M7 | `ef80dfb`, `14ad71f`: added cap, quota, root, shared-mount, worker failure and drain tests; failure callbacks check request ids. | `round4_json_read_cap_is_enforced`, `round4_json_depth_is_enforced`, `round4_json_nodes_are_enforced`, `round4_json_root_is_enforced`, `round4_json_shared_mount_and_inflight_quota`, `round4_json_worker_save_quota_reports_correlated_failure`, drain tests above. Relaxing read/depth/node/root/inflight limits, denying the shared mount, removing worker quota checking and removing drain each produced the expected RED. Final suites GREEN. |

The mutation batches produced 15 and 6 expected failures, respectively; a later
pointer/closed-view batch produced four expected failures. Separate Windows alias
and partial-load retirement controls each failed once. All temporary mutations were
restored before the final successful gates. These are intentional control failures,
not unresolved test failures.

The first broad addon gate caught a compatibility mistake in changing frame_x/frame_y:
eight existing HUD/layout/picture tests failed. Those fields were restored, preserving
the bundled mods unchanged; the pointer contract now consistently uses layout's primary
window-pixel fields. It also exposed a Windows same-file queue alias rejection, now fixed.
A subsequent stale wide-window assertion was corrected to the final contract. No bundled
HUD picture expectation was weakened. The final full gate passes them all.

### Small LOW fixes and retained limits

- a1 L2: `view:close()` releases quota immediately, including six close/recreate cycles
  without waiting for GC; that quota test observed RED before pruning closed leases.
  Otherwise discarded views release on GC, now documented.
- a1 L3/L4: invalid guide coordinates return nil,Error; point guides end on world loss.
  `round4_point_guide_rejects_bad_coordinates_and_ends_on_world_loss` ran RED/GREEN.
- a1 L5: window-wide mouse notifications and lack of touch translation are explicit in
  the spec. Touch support was not added.
- a2 L1/L2: track only the latest coalesced save id, documented; competing-path quota is
  checked before snapshot copying.
- a2 L3: the historical unsandboxed 14 MiB Lua encode/I/O baseline is explicitly ignored
  by default. It remains opt-in historical evidence, not a live-mod measurement. Actual
  public-API async save and catalogue workloads still run and assert their sizes.
- a2 L4: shared and native embedded catalogues now also compare byte-for-byte against
  the pinned data. The general-purpose JSON loader does not require catalogue hashes.
- a2 L6: disconnected load workers release their pending count and diagnose failure;
  disable cancels/drops their service. Paused completion retention remains bounded.
- a2 L10/L12: empty native overlay passes are suppressed and the circle-mask capture
  writes to the temporary directory, allowing read-only source checkouts.

Not done in this Part A fix scope: minimap-sized capture/4K sharpness work (a1 L1),
large-guide dot culling (remainder of a1 L3), touch translation, hidden native search-button
removal (a1 L6, Part B), cleanup of the original stylistic reflows, packaging the shared
catalogue with Part B's release allowlist/source hashes (a2 L5), guaranteed resource-event
retention under a full combat event queue (a2 L7), changing protocol position semantics to
dead-reckoned journal coordinates (a2 L8), or the Part B/C mods/native removals. The entity
fixture is protocol-shaped data through the packet capture boundary, not a claim that it
decodes a real packet. No oracle/disc/live-server or owner terrain/indoor playtest is claimed.

A slice checks its budget between values, so an individual allocation or GC can exceed
1 ms. Native snapshot copying remains synchronous. On a save flush timeout, pending
commits are cancelled and diagnosed; it is not reported as a durable save. The bounded
wait does not preempt an operating-system filesystem call. Native capture GPU tests use
synthetic prepared Play state; real terrain/indoor visuals remain for owner playtest.

### Final verification

Every cargo command ran in the foreground under the shared mkdir lock (20-minute
acquisition timeout), shared CARGO_TARGET_DIR and scratch EQOA_CONFIG. No build or test was detached; every cargo process was awaited to completion. Explicit SKIP lines are separate
from Rust ignored counts and do not prove those workloads ran.

| Gate | Pass | Fail | Ignored | Explicit SKIP lines |
|---|---:|---:|---:|---:|
| Addon suite, single thread | 405 | 0 | 25 | 9 |
| Full client suite, single thread | 2094 | 0 | 32 | 208 |
| Current round4 regressions within full client | 35 | 0 | 1 historical baseline | 0 |
| Release catalogue workload benchmark | 1 | 0 | 0 | 0 |
| WASM check | PASS | 0 | n/a | 0 |
| Footprint against reviewed base | PASS | 0 | n/a | 0 |
| Diff whitespace check against 9e7257a | PASS | 0 | n/a | 0 |

Addon suite: 129.15 s; full client: 219.59 s; WASM: 2.88 s. The nine addon skips
are four Windows symlink privilege cases, two missing pinned Forge prerequisites and
three missing reviewed external bundled-UI goldens. The full suite additionally lacks
disc/module/live-S2 fixtures. Existing Wereoxxs/ARPG/Modern UI load/save/disable and
round 3's four HUD mods, native picture comparisons and performance workload passed.

`performance_busy_world_zero_addons_frame_and_packet_allocation_bound` passed its
200-entity/200-ability, 300 frame/menu and 600 packet/chat iterations: frame 16.151 us,
menu 17.637 us, event 1.781 us, chat 0.136 us; allocations and allocated bytes were zero
in all four paths. The four-mod HUD storage/text allocation phases stayed zero after
warmup; native art remained 22 allocations per active frame.

Final full-client catalogue dispatch maximum: 1.540 ms in debug (501 dispatches).
Release: 1.079 ms (162 dispatches), versus the measured unsliced 101.794 ms baseline.
Three near-14 MiB save snapshots took 13.598 ms combined in the full client test;
the asserted payload was 14,679,936 bytes. These are host-specific fixtures, not FPS
or worst-case allocation/GC promises.

The accepted viewer budget is unchanged at +104/-14; all fix-round implementation
is fork-owned. Final release build is run after this report's commit, as the last gate;
its outcome is recorded in the final six-line handoff and ignored fix-release-final.log.


## Part B: journal and search mods (in progress)

The Part B brief was committed first (`5ced426`). Work remains on
`feat/eqoa-api-r4`; no push, merge, tag or protected-branch change was made.
The independent API review b1 is preserved unchanged in `a7f7c5f`.

Implemented checkpoints: manifest declarations (`2b6e947`); live shared JSON reads
and successful-save notifications (`2b3c463`); bounded delayed commands (`bb6cddb`);
player file actions (`b1989d0`); one-time journal/key migration (`fe996e3`); generic
mod option action rows (`8a342dd`). The journal migration copies a >1 MiB fixture,
keeps the original and existing destination, and does not resurrect deleted mod data.

Review fixes committed: H1 export extension/name restrictions (`4d101b4`); M1
separate command queues, four pending messages per sender and 4 KiB JSON arguments
(`cafdd94`); M2 redirected Desktop/picked-file support with private junction denial
(`25a1a02`); M3 three persistent-sequence imports outside save quota (`d60ac07`);
M4 one shared activation across VMs, non-forwardable command authority and a request
guard retained while a disabled mod's picker remains open (`36df7c6`).

Most recent completed focused run: **15 passed / 0 failed / 0 ignored / 0 SKIP**
(`review-m4-green2.log`, 3.38 s). All Cargo invocations used the shared foreground
lock/cache and scratch config. A blocked-picker regression caught a guard Drop
mutex deadlock; its verified test executable was stopped, Cargo/lock drained,
and lazy guard construction fixed it before the successful rerun.

Observed RED controls so far: missing shared loader; reserved shared-write path;
missing command/file APIs; self-command RefCell borrow; hidden resource data from
double guarding; absent journal/key migration; unsafe `.bat` export; >4 KiB
command acceptance; redirected-folder rejection; imports exhausting private quota;
two VMs exporting from one physical action; missing mod-name dialog prefix.
Actual worker copies/collisions/cancellation, pointer/key authority, retained
imports and near-14 MiB save use scratch data. Picker selection and Desktop
resolution are injected; the junction fixture exercises real Windows redirection.
No live native dialog, disc, oracle or owner playtest is claimed.

**Not complete:** LOW review controls/fixes are in progress. The bundled journal
and search mods, native search/journal removal, moved catalogue/font packaging,
PNG captures, parity/performance measurements, footprint tightening/dry-run
comparison, broad addon/client/WASM gates and final release build remain pending.
No full-suite or completion claim is made by the focused results above.


Review LOW controls: the baseline run reproduced missing remove notifications,
`kind="file"` on worker disconnect, and a visible partial final export name.
The corrected focused run passed **20/0/0**, no SKIP, in 5.82 s. A subsequent
five-protection mutation batch produced **7 expected failures / 13 passes**:
ordinary command queue/sender guards, blocked-dialog guard, persistent import
sequence, delegated-mark expiry, rebound-key migration and the key-collision
fixture all detected their removed protection. The four mutated source files
were restored automatically and verified byte-for-byte before the next green run.

The native export now syncs a temporary file and uses a non-replacing atomic
Windows rename; its pause fixture proves the final filename is absent before
publication and complete afterward. Shared deletion notifies readers; disconnected
workers retain their original operation kind. Key migration preserves both
explicit assignments when they collide; Controls remains the place to rebind.
The rfd reference is cfg-gated on targets that provide it. Remaining review LOWs:
no per-pair cross-frame command rate limit; no retry/fallback policy for a legacy
migration on filesystems lacking hard-link support; real picker cancellation
still requires closing the OS dialog; no live non-Windows file-backend claim.
Migration and native removal must ship together so no intervening native-only
sightings are omitted. Native removal is still pending at this checkpoint.

Part B journal checkpoint: enabled bundled journal ports live creature filters, cross-world 30-unit same-level dedupe, the Forge name/id schema, bounded incremental validation/indexing, quiet30s/max120s saves and disable flush. `journal-max-green.log`: 4 passed, 0 failed/ignored/SKIP. Play/protocol fixtures cover rejected self/pets/corpses/flags/type/blank/nonfinite, disabled VM, large reload, Forge import invalid/collision controls, disk export and invalid original preservation. A continuous-discovery fixture initially reused channel1 for every added slot; corrected to the actual channel for each slot. Pending export snapshot RED then GREEN1/0/0; worker waits for accepted async save. Native removal and search remain pending.

Part B search checkpoint: bundled search ports catalogue/journal/live/group/place results, UTF-8 80-character editor/navigation, nearest live/known guide and dots, live follow/fallback, group departure, F4 and Space Mono presentation. `search-embedded-green.log`:2 passed/0 failed/ignored/SKIP; restored Part B subset28/0/0/SKIP0 (`mod-restored-green.log`). Temporary Lua mutations removed distance dedupe and guide creation:25 passed,3 expected failures; both files restored byte-for-byte before the green run. The Places fixture initially queried a name absent from the catalogue of camps; fixed to the actual Saltwater camp. Capture/performance and native removal gates still pending.

Part B native-removal checkpoint: deleted native map/search.rs and map/journal.rs and their input/feed/draw/exit hooks, including mapshot search fixtures. Native F2/F4 rows are gone; one-time fork-owned journal/key migration remains. Catalogue/font/licences moved under the search mod; round-minimap typography references that font until Part C. Part B subset28/0/0/SKIP0 after removal; bundled installer23 passed/0 failed/1 ignored (external pinned-reference prerequisite), SKIP0; player-release Python suite20 passed. Native map/play/viewer/binds divergence against reviewed11aa5f0 falls from +4523/-153 to +985/-144. Tightened13 shared budgets, including existing loose net/session and render/ui budgets; full table in final section pending. Footprint PASS. origin/master advanced independently to3439fb1, so reviewed-base accounting stays11aa5f0; dry-run upstream conflicts are recorded separately.

Part B GPU checkpoint: real Manager, both default-on mods, actual Space Mono ready, native map/minimap draw passes and GPU readback now run with a1,411,026-byte/23,000-spawn journal. Initial unfiltered result sort exhausted4M Lua instructions; cached distances fixed it (observed RED then GREEN). Debug Manager.frame CPU120frames: mean3.065ms,p50 2.957ms,p95 4.175ms,max6.733ms; release measurement pending. `map-manager-font-green.log`:1/0/0/SKIP0; whole Part B29/0/0/SKIP0 in44.59s. PNGs r4-search-map/minimap.png inspected; native disc-free fixtures show real chrome/round minimap, with terrain absent because noISO is available. Release numbers/full gates remain pending.
