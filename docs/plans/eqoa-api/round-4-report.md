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

All cargo commands ran in the foreground under the shared mkdir lock (20-minute acquisition timeout), `CARGO_TARGET_DIR=%USERPROFILE%\Paseo\targets/shared`, and scratch `EQOA_CONFIG`. Long logs remain under ignored `.superpowers/sdd/round4/`. Explicit SKIP lines are counted separately from Rust ignored tests and are not evidence that those workloads ran.

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


## Part B: bundled creature journal and map search

Part B replaces the native journal/search implementations with two bundled mods whose manifests default to enabled. Existing saved-profile enable migration remains a deferred B2 finding, so this is not a claim that every upgrading player automatically gets both mods enabled. The journal records protocol-shaped live creature observations, retains the Forge name/id schema, incrementally loads and indexes saved data, saves after quiet/max intervals and on disable, and provides import/export through both its options and the search footer. Search combines catalogue, shared journal, live entities, group and native places, with the rebindable F2/F4 keys, UTF-8 editor, native guides/dots and Space Mono presentation. The generic map-open action preserves F2 when the minimap is off and respects the Map master setting.

The catalogue, font and notices are packaged with search, with catalogue bytes pinned and bundled source hashes updated. One-time fork-owned migration preserves the legacy journal and rebound keys without replacing existing mod data or explicit mod bindings. Native `map/search.rs`, `map/journal.rs`, their hooks and old key rows are removed. The round minimap and its remaining theme dependency belong to Part C and are retained here.

The API additions cover declared read-only shared mounts/change notifications, bounded delayed commands, player-initiated file actions, private staging and generic option buttons. The B1 review is committed unchanged. Its H1/M1-M4 and selected LOW fixes are included. The supplied B2 review remains uncommitted and unchanged for its assigned fix session; the pre-existing saved changes verified in this resume already cover some observations made against the older committed head. No additional B2 fix work was started.

### Regression evidence and negative controls

All 34 Part B tests ran through the actual runtime, with Play/protocol-shaped data where applicable. These are simulated observations, not decoded capture or live-server evidence. Test names below are suffixes under `round4b_`; related cases are grouped so their controls can be read together. RED denotes an observed test failure before implementation or with the protection temporarily removed. Mutation sources were restored byte-for-byte before green verification.

| Tests / behavior | Negative controls actually run |
|---|---|
| `manifest_rejects_invalid_shares_commands_and_permissions`; `shared_json_requires_running_owner_and_declared_path` | Invalid declarations, disabled/missing owner, undeclared paths, traversal and reserved shared-write paths rejected. Missing shared loader and missing API paths observed RED. |
| `mod_commands_are_declared_bounded_and_delayed`; `mod_command_can_target_its_own_vm` | Undeclared/missing/disabled targets and oversized arguments rejected; no synchronous dispatch. Original self-command borrow failure reproduced and fixed. |
| `review_commands_preserve_other_events_and_sender_fairness` | 4 KiB arguments, separate 64-command queue and four pending messages per sender; command flooding preserves player/import events. Removing queue/sender protections produced expected RED. |
| `file_actions_require_permission_and_player_activation`; `player_command_import_export_copy_files_and_cancel`; `option_buttons_are_player_activations_and_disable_cleanly` | Missing permission, uninitiated calls, arbitrary file paths, disabled VM and cancelled requests rejected; copied import and disk export verified. No real OS picker was opened. |
| `review_export_refuses_executable_and_reserved_names` | `.bat`, `.lnk`, `desktop.ini`, Windows reserved names, leading dots and separators refused; unsafe export acceptance observed RED before the allowlist/sanitizer. Only JSON/TXT/CSV/PNG names accepted. |
| `review_redirected_desktop_and_picked_file_keep_private_checks` | Actual Windows junction fixture: redirected Desktop/picked import succeed while a mod-storage junction is denied. Original blanket reparse refusal observed RED. Imports still use a private copy. |
| `review_import_ids_survive_enable_and_staging_cannot_starve_saves` | Four 6 MiB imports across enables retain only the newest three, ids stay monotonic, and a near-14 MiB private save succeeds. Reset sequence/quota behavior observed RED before the fix; persistence mutation also failed. |
| `review_one_physical_action_is_spent_across_vms`; `review_disabled_dialog_stays_single_and_has_mod_title`; `review_unused_delegated_action_expires_after_dispatch` | A second action from the same physical activation, recursive delegation, a second open dialog after disable/re-enable, and reuse of an expired mark are denied. Missing title prefix, double activation and removed request/expiry guards observed RED. |
| `review_disconnect_reports_original_import_kind`; `review_export_has_no_partial_final_file`; `review_remove_notifies_shared_readers` | Baseline reproduced wrong operation kind, visible partial destination and missing removal notification. Paused export sees no final filename until synced atomic rename; disconnected worker retains import kind; deletion emits change. All then GREEN. |
| `first_start_copies_legacy_journal_without_overwrite`; `migration_keeps_rebound_search_and_stop_keys`; `review_key_migration_keeps_existing_mod_collision` | >1 MiB copy keeps original; existing destination and deleted-data sentinel prevent overwrite/resurrection. Explicit mod binding/unbound values and colliding keys survive. Missing migration and migration mutations observed RED. |
| `journal_records_play_entities_with_native_filters_and_dedupe` | Rejects self, pets, corpse/type/flags, blank/Unicode whitespace names and nonfinite positions; disabled VM records nothing. Known ids and same-level nearby cross-world duplicates excluded. Removing 30-unit dedupe produced RED. |
| `journal_reloads_large_file_imports_forge_and_exports_current_snapshot`; `export_waits_for_accepted_async_snapshot_before_first_file_exists` | >1 MiB reload, real Forge-format import, invalid records, collisions and disk export checked. Pending first-save export originally failed with missing file (RED); accepted snapshot fence then passed. |
| `journal_saves_after_quiet_and_preserves_invalid_existing_json`; `journal_max_interval_flushes_during_continuous_discoveries`; `journal_protects_existing_json_array_root`; `optional_json_missing_is_not_found_without_warning` | Before-deadline no-save controls, continuous sightings, malformed/array original preservation, quiet missing file and denied-path diagnostic controls. Array root and normal missing-file warning assertions observed RED before fixes. |
| `journal_caps_total_and_import_and_export_truncated_view` | 80,001 initial spawns yield an 80,000-spawn exported view; 50,001 valid imported spawns yield at most 50,000. Assertions verify the boundary inputs actually ran. |
| `search_selects_live_with_dots_follows_and_falls_back_to_known` | Nearest live vs recorded spawn, all known dots, moving live target, lost target fallback, shared reload and F4 clear. Removing guide creation produced RED. Metadata searches combine records; cross-world/level fixture observed RED before correction. |
| `search_group_departure_places_and_unicode_editor_limit`; `search_lists_inactive_group_member_as_unavailable`; `search_f2_opens_map_with_minimap_off_and_obeys_map_master` | Group departure clears guide, inactive member is unavailable, native place selected, UTF-8 80-character bound, editor/navigation, Map-off and Minimap-off controls. Inactive member and F2/Minimap-off cases observed RED. |
| `text_editor_receives_keys_and_releases_held_gameplay` | M/P/O/G typing reaches the editor without map shortcuts or gameplay movement; held combat/camera inputs released. Non-editor input path remains available. |
| `manager_map_mods_large_journal_gpu_captures_and_frame_cost` | Real Manager, workers, loaded Space Mono, actual native map/minimap GPU drawing/readback, footer export/import disk results and completion receipts. Asserts both mods enabled, 23,000 loaded spawns and open search. Original uncached distance sorting exhausted the Lua instruction budget (RED); cached distances passed. |

The focused API review restoration run passed 20 tests after a mutation batch produced seven expected failures and 13 passes. The journal/search mutation batch produced three expected failures and 25 passes; restored code passed 28. The later edge run reproduced five expected failures (27 passes), and the F2/metadata run two (two passes), before the final 34-case green run. These intentional RED controls are not unresolved gate failures.

### Gates and measurements

| Gate | Pass | Fail | Ignored | Explicit SKIP lines |
|---|---:|---:|---:|---:|
| Addon suite, single thread | 426 | 0 | 25 | 9 |
| Full client suite, single thread; run once at the end | 2080 | 3 | 32 | 208 |
| Corrected contract fixtures after full run | 3 | 0 | 0 | 0 |
| Part B regressions within full client | 34 | 0 | 0 | 0 |
| Bundled installer within full client | 23 | 0 | 1 | 0 |
| Zero-addon allocation gate within full client | 1 | 0 | 0 | 0 |
| Release Manager/frame-cost workload | 1 | 0 | 0 | 0 |
| Player-release Python tools | 20 | 0 | 0 | 0 |
| WASM check | PASS | 0 | n/a | 0 |
| Footprint against reviewed base | PASS | 0 | n/a | 0 |
| Diff whitespace against 9e7257a; CRLF notices preserved | PASS | 0 | n/a | 0 |

Addon suite: 237.59 s; full client: 404.78 s; release workload: 19.99 s; WASM: 32.95 s; Python tools: 45.60 s. Rust ignored counts and explicit SKIP lines are different: neither proves its workload ran. The nine addon SKIPs are four Windows symlink privilege fixtures, two missing pinned-Forge prerequisites and three missing external reviewed UI goldens. The full suite additionally skips unavailable disc/module/live-S2 prerequisites.

Current bundled Wereoxxs slot/weapon-bar and native bundled UI pipelines passed, as did ARPG/Modern load-save-load-disable and private migration, all four Round 3 HUD picture cases and their combined per-frame workload. The three external bundled-UI golden comparisons were skipped after producing actual captures. Pinned external Forge lifecycle/GPU tests remain ignored without `EQOA_FORGE_REFERENCE`; no success for that external suite is claimed.

The single full-client run reported three stale test contracts, not a green full suite: `fixture_frees_f2_while_production_keeps_map_search_on_it` still unwrapped the deleted native bind row; `group_capture_excludes_player_and_inactive_slots_before_lua` contradicted the saved change retaining inactive members without positions; `report_state_includes_claimed_hud_elements_and_movement_flags` expected the old label rather than `player search open`. Only those tests were corrected, preserving self exclusion, inactive-position denial, HUD claims and movement-flag assertions. Their renamed `round4b_gate_` cases passed together in the focused rerun (3/0/0, no SKIP, 0.13 s; committed as 07ffc89). Production code did not change after the full run. The full suite was not repeated, respecting the brief's once-at-the-end instruction. These three original failures remain visible in the table rather than being reported as full-suite passes.


All Cargo runs use the shared foreground mkdir lock, shared target directory, scratch config and a 20-minute process timeout. The first resumed compile detected another worktree's stale renderer artifacts in the shared target; a renderer artifact refresh resolved the missing-method errors without a source fix.

Resumed focused runs passed the other 33 cases but reproduced a Manager test deadline failure. Additional state confirmed that the import receipt arrived while shared reload had not finished; another run hit the separate initial-load deadline. Both large async waits now have a 60-second bound and exact workload assertions remain. The corrected Manager run passed 1/0/0/SKIP0 in 39.10 s: initial load 12.25 s, footer import/shared reload **20.47 s**, beyond the former 20-second deadline. The implementation and formatted test changes were committed as `6c47993` before the broad gates. No B2 runtime fix was made.

The first broad addon run reproduced one legacy compatibility regression: **425 passed / 1 failed / 25 ignored / 9 explicit SKIP lines**, 234.17 s. `fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring` detected F8 toggling the stats box while Forge chat owned text capture. The binding wrapper now permits callbacks only for the sole native editor owner while capture is active; legacy Forge chat and other VMs remain blocked. The existing test also verifies that the native editor retains its own control and another VM does not fire the same key. This observed RED is the negative control for the gate fix, committed as `76ba018` after its compatibility case passed. The broad gate then passed all 426 cases.

Both mods were enabled, Space Mono was ready, search stayed open, the catalogue loaded and the journal contained **1,411,026 bytes / 23,000 spawn records under one creature name**. Assertions fail if that workload does not run. The 120-frame timings measure CPU time in `Manager.frame`, including mod callbacks/drawing preparation; they exclude GPU submission/readback, initial load and the later import/export waits. This is a frozen-world steady-state sample, not live FPS or a maximum-name-diversity benchmark.

- Release: `journal=1411026 bytes, 23000 loaded spawns; frames=120; mean=1.404 ms p50=1.092 ms p95=3.339 ms max=6.141 ms`
- Debug, final full suite: `journal=1411026 bytes, 23000 loaded spawns; frames=120; mean=3.059 ms p50=2.913 ms p95=4.313 ms max=7.279 ms`

The zero-addon busy-world gate ran 300 frame/menu and 600 packet/chat iterations with **zero allocations and zero allocated bytes in all four paths**:

- `menu_frame_us=12.396 allocs/menu_frame=0.0 bytes/menu_frame=0.0`
- `frame_us=15.976 allocs/frame=0.0 bytes/frame=0.0; event_us=1.850 allocs/event=0.0 bytes/event=0.0; chat_us=0.140 allocs/chat=0.0 bytes/chat=0.0`


Owner captures: [map search](r4-search-map.png) and [minimap search](r4-search-minimap.png). Both use real native map/minimap composition and mod GPU rendering, with no terrain art because no owned disc is available in this worktree. They are owner review images, not golden baselines.

### Shared footprint and merge dry run

Against the reviewed base `11aa5f088bb52d5f306b0224e37b782b1bf4503d`, map/play/viewer/binds divergence falls from **+4,523/-153** before native removal to **+985/-144** afterward. Every changed budget was tightened to its measured value, including two pre-existing loose allowances:

| Shared file | Previous allowance + / - | Final allowance + / - |
|---|---:|---:|
| bindpage.rs | 10 / 5 | 9 / 5 |
| binds.rs | 55 / 38 | 39 / 37 |
| play.rs | 278 / 62 | 148 / 55 |
| viewer.rs | 104 / 14 | 66 / 11 |
| net/session.rs | 218 / 23 | 207 / 22 |
| render/ui.rs | 85 / 2 | 67 / 2 |
| map/chrome.rs | 304 / 19 | 184 / 12 |
| map/journal.rs | 1,567 / 0 | 0 / 0 |
| map/mini.rs | 313 / 44 | 242 / 14 |
| map/mod.rs | 70 / 10 | 16 / 6 |
| map/search.rs | 1,716 / 0 | 0 / 0 |
| map/tests.rs | 116 / 0 | 71 / 0 |
| map/tool.rs | 73 / 3 | 54 / 3 |

The footprint gate passes against that reviewed base. `origin/master` advanced independently to `3439fb17b5e2b66acfddc33a5eb7f824da1890d5`; it was not merged. Dry-run `merge-tree --write-tree --name-only` conflicts against that pinned upstream fall from **two** at `d98edd4` (`play.rs`, `viewer.rs`) to **one** after removal at `e13b39c` (`play.rs`). This is conflict-count evidence, not a completed merge. Using the advanced upstream as the policy accounting base reports unrelated unreviewed upstream drift and is not substituted for the reviewed base.

### Parity limits and not done

No live disc/terrain/indoor/owner playtest, oracle run or live-server packet decode is claimed. Picker/Desktop selection is injected in tests; actual copy, reparse protection, retained imports, atomic export and cancellation are exercised on disk. The OS picker cannot be forcibly closed on disable; the request gate stays held until the player closes it. Capture and timing fixtures do not establish live-world FPS or worst-case GC/OS latency.

The journal uses the public protocol-position snapshot rather than native actor dead-reckoning, as already recorded in Part A. JSON numeric decoding does not preserve the original distinction between an integer token and an integral floating-point token. These are parity limits, not claims of exact native equivalence.

Remaining B2 review observations are deferred, not accepted behavior or passing live-world tests. The review used the older committed head; feedback, self exclusion, live bind labels, aggregate journal metadata matching and F2 with Minimap off were already in the saved work committed in this resume. Remaining concerns include:

- Active entity/player updates can restart unfinished result builds; existing saved profiles can leave the new mods disabled. The benchmark reported above is a frozen-world steady-state fixture, not coverage of these paths.
- Full-map header overlap, outside-click/wheel propagation, fractional wheel selection and result-offset clamping remain for the fix session.
- Known-to-live guide upgrades, moved/removed waypoint tracking, cross-world status/group-zone retention, the dedicated Track waypoint control and native guide colors/dot/rim styles differ.
- Native catalogue overrides (`EQOA_MAP_DATABASE` and an adjacent catalogue) are no longer the search source. Some old player docs/package contents still describe them. Search is now native-mod-only; the browser/non-mod builds lose the former native group/place/live search despite WASM compiling.
- Names are stored trimmed; native retained the original name. Opening the editor cancels autorun/follow, controller navigation is absent, and composed dead-key/IME input is not supplied by the logical-key path.
- Event refresh, choose/sort, per-key filtering, eager whole-journal reload and synchronous save snapshot costs need active-world/max-diversity measurements and the review's optimization work. The retained 80,000-spawn boundary test is not that performance evidence.
- The review also flags save dirty-time bookkeeping, silent early export refusal, shared-journal retry after late owner enable, duplicate catalogue packaging and remaining theme dead code. Theme/minimap cleanup stays with Part C.

Retained B1 LOWs: no per-pair command rate limit across frames beyond the pending per-sender quota; no migration retry/fallback on filesystems without hard-link support; no live non-Windows file-backend verification. Key collisions are preserved for the player to rebind in Controls. Migration and native removal must ship together. B2 review fixes and all Part C minimap/theme work are outside this completion scope.

The native release build is run after this report/capture commit as the last gate. Its result is recorded in the six-line handoff and ignored `final-native-release.log`; no merge, push or tag is performed.


Part B GPU checkpoint: real Manager, both default-on mods, actual Space Mono ready, native map/minimap draw passes and GPU readback now run with a1,411,026-byte/23,000-spawn journal. Initial unfiltered result sort exhausted4M Lua instructions; cached distances fixed it (observed RED then GREEN). Debug Manager.frame CPU120frames: mean3.065ms,p50 2.957ms,p95 4.175ms,max6.733ms; release measurement pending. `map-manager-font-green.log`:1/0/0/SKIP0; whole Part B29/0/0/SKIP0 in44.59s. PNGs r4-search-map/minimap.png inspected; native disc-free fixtures show real chrome/round minimap, with terrain absent because noISO is available. Release numbers/full gates remain pending.

## Part B fix round

This section supersedes the open HIGH/MEDIUM findings and incomplete full-suite gate in the preceding Part B report. Scope is the B2 review and the owner's fix brief; Part C remains separate. Develop's released 0.5.3 default-on hotfix was merged first (`ac21584`); its production profile implementation was not duplicated here. No push, tag or merge into a protected branch was performed.

### Findings, fixes and controls

Test names below omit the common `round4b_` prefix. Tests use real Hosts or Manager/Runtime from Play-shaped snapshots, shipping Lua and actual asynchronous disk workers. Intentional negative runs are separate from acceptance gates.

| Finding | Fix and commits | Runtime evidence and negative control |
|---|---|---|
| H1 | Query-owned source snapshot; movement refreshes existing row positions without restarting the build (`ea6e721`). | `fix_moving_world_finishes_search_and_enter`: moving entities every frame, bounded completion and Enter. Restoring pre-fix Lua fails ?moving entities must not restart a query build?. |
| H2 | Merge develop; add actual pre-existing-profile launch and explicit-Off persistence regression (`b1d20f2`). | `fix_existing_profile?`: real `Runtime::new`/headless launch. Disabling missing-key bundled defaults and registration produces ?eqoa_map_search missing from an old profile must be on?; restored code passes. Journal/search are packaged here. The same test includes `eqoa_round_minimap` when packaged, but that third mod belongs to Part C and has NOT run on this branch. |
| M1 | Visible early Import/Export refusals and timed status messages (`65def4f`); existing receipts/count/collision messages retained. | `fix_journal_reports_early_export_refusal` checks the paired search footer with an actual malformed/protected journal. Pre-fix Lua fails the visible refusal assertion. Manager physically activates footer actions and verifies saved/imported file results and suffix receipt. |
| M2 | `eqoa.map.get().search_anchor` comes from the real full-map chrome footprint through the existing overlay hook (`423814f`). Recapture `ebd597b`. | `fix_search_toolbar?` tests occupied header bands. Replacing measured y with 44 fails on header `[4,30,324,60]`. Manager GPU capture and [full-map PNG](r4-search-map.png) place Search below progress; [minimap PNG](r4-search-minimap.png) was rendered again and is byte-identical. |
| M3 | Retain self exclusion and inactive unavailable rows; strengthen fixture with an inactive actor that has valid coordinates and a raw protocol roster containing self (`41a0380`). | `search_lists_inactive_group_member_as_unavailable` asserts unavailable/non-selectable and self exclusion. Removing the active-position guard fails the unavailable-row assertion (0/1); restored code passes (1/0). |
| M4 | Whole-window modal pointer claim while open; outside click closes and is consumed; wheel changes rows only over the list (`423814f`). | Real Manager asserts outside wheel/click claims; Host checks selection unaffected outside. Removing only the whole-window claim fails ?open search must consume an outside wheel? (0/1); restored shipping embedded code passes (1/0). The negative Manager fixture reads source Lua only under the test-only `EQOA_R4_TEST_SOURCE` flag; normal gates use embedded shipping bytes. |
| M5 | Existing F2 opens full map when minimap is off/hidden and obeys Map master option. No new production fix required. | `search_f2_opens_map_with_minimap_off_and_obeys_map_master`, retained from Part B with its earlier recorded RED control; rerun in the fix-round gates. |
| M6 | Indexed known-to-live reacquisition, waypoint move/removal tracking, world-aware status, clear rejected guides; full-map Track waypoint control within toolbar (`ca210e5`, `2b81d37`). | `fix_known_guide_upgrades_and_waypoint_tracks_changes` also checks world status and rejected set_guide. Pre-fix Lua leaves the line at `[100,200]` rather than live `[700,800]`. |
| M7 | Closed observation does no work; places cached by world; per-name spawn indexes; cached searchable names and bounded 500-name filtering; growing query refines prior rows; shared reload lazy when closed and retries late owner (`ca210e5`). | `fix_closed_search_defers_observation_and_shared_reload`, `fix_shared_reload_is_lazy_and_retries_late_journal_owner`, and `fix_busy_world_max_journal_cost_and_incremental_queries`. Pre-fix Lua fails closed-observe, late-owner and missing-index assertions. Release workload/margins below. |
| M8 | Wheel sign advances whole rows, offsets clamped after shrink (`423814f`). | `fix_wheel_is_local_and_steps_whole_rows`: fractional input, integer selection/offset, outside-list negative control. Pre-fix Lua changes selection outside the list. |
| Leftovers | Journal records `actor_position` with protocol fallback; reset dirty deadline at accepted snapshot (`65def4f`). Restore upstream bind tests and Play parentheses; tighten budgets (`1fef6e3`). | `fix_journal_records_actor_position_and_protocol_fallback` and `fix_save_deadline_restarts_after_snapshot`: pre-fix Lua records `[10,0,20]` rather than actor `[110,5,120]` and immediately resaves newer sightings. `binds.rs` equals reviewed upstream `11aa5f0`; shared footprint passes. |

The nine pre-fix-Lua controls produced **0 passed / 9 failed**, then byte-for-byte source restoration and the focused positive run produced **11 passed / 0 failed** (29.06 s). Separate Rust controls for H2 and M2 each produced **0 passed / 1 failed**. The busy-world control's missing-index failure proves the optimized workload structure is required; it is not a before/after timing measurement. All mutated sources were restored byte-for-byte before acceptance runs.

SOURCE hashes, embedded module list, independent release packaging fixture, API schema documentation and player notes were refreshed (`398386d`, `c8b9945`, `cc7bd57`). Packaging continues to verify an independent exact allowlist. No shared budget increased: `binds.rs` tightened from +39/-37 to 0/0, `bindpage.rs` +9/-5 to +7/-3, `play.rs` +148/-55 to +147/-54; map/mod.rs remains +16/-6. There is no fix-round diff in theme.rs, mini.rs, fork_ext/map.rs or renderer files. Theme dead code and round-minimap work stay with Part C.

### Final gates

| Gate | Passed | Failed | Ignored | Explicit SKIP lines |
|---|---:|---:|---:|---:|
| Focused fix regressions | 11 | 0 | 0 | 0 |
| Complete Part B debug | 48 | 0 | 0 | 0 |
| Addons, single thread | 428 | 0 | 25 | 9 |
| Complete Part B release | 48 | 0 | 0 | 0 |
| Full client, one complete final run | **2,095** | **0** | 32 | 208 |
| Player-release Python tools | 20 | 0 | 0 | 0 |
| WASM | PASS | 0 | n/a | n/a |
| Footprint against `11aa5f0` | PASS | 0 | n/a | n/a |
| Diff whitespace against `0b12613` | PASS | 0 | n/a | n/a |

WASM check passed in 47.37 s. Addon tests took 246.09 s; release Part B 70.88 s (452.18 s including compile); full client **456.17 s** (669.92 s including compile); Python tools 80.785 s. The full run has zero filtered tests and includes the three corrected Part B contract fixtures, both strengthened M3/M4 cases and all 48 round4b cases.

Explicit SKIP lines are counted case-sensitively as `SKIP`, not test names containing ?skip?. The nine addon SKIPs are four Windows symlink-privilege fixtures, two pinned Forge prerequisites and three absent external reviewed UI goldens. The full run's 208 additionally include unavailable disc/module/live-S2 prerequisites. Ignored tests are separate: 25 addon cases include the historical unsandboxed benchmark and missing external pinned Forge suites; the full suite has seven additional external/package and survey/benchmark ignores. None is claimed as exercised.

The zero-addon busy-world gate passed in that full run: 300 frame/menu and 600 packet/chat iterations, **zero allocations / zero bytes** in every measured path. `menu_frame_us=19.718`, `frame_us=21.763`, `event_us=1.510`, `chat_us=0.102`. Existing Wereoxxs/ARPG/Modern and Round 3 workloads also passed; external goldens/reference packages retain the skips above.

All Cargo commands ran in the foreground under `%USERPROFILE%\Paseo\cargo-build.lock`, using the shared target, scratch `EQOA_CONFIG`, one test thread and a 20-minute per-command timeout. The runner waits/retries while another agent holds the lock and checks nonzero workload counts. Its own debug binary is preserved under the lock for controls; a zero-test invocation of a replaced shared binary was rejected as evidence.

The first cap regression attempt failed because its fixed 5,000-frame loop clicked Export before async load finished. `39175ef` replaces that loop with a bounded 90-second readiness/export wait and real activation retries; exact 80,001?80,000 and 50,001?50,000 boundary assertions remain. The focused corrected run passed (31.70 s), and the entire 48-case Part B debug run passed (179.02 s). Shared debug/release renderer artifacts also produced missing-method compile errors; artifact-only cleaning rebuilt this worktree's renderer without source edits. Those failed attempts are not reported as passing gates.

### Busy-world release frame cost

| Release state | Mean ms | p95 ms | Maximum ms |
|---|---:|---:|---:|
| Closed | 5.520 | 16.777 | **74.620** |
| Open while typing | 2.702 | 5.900 | **46.049** |

Minimum search-VM instruction reserve across cache construction and sampled phases was **1,084,000 / 4,000,000 (27.1%)**, above the unchanged 20% floor. The older real-Manager 23,000-spawn fixture also ran in release: mean 1.332 / p95 3.165 / max 6.409 ms.

**The busy-world release maxima exceed the supplied 18.97 ms baseline.** The structural M7 changes and budget checks pass, but these measurements do not demonstrate resolved hitches or a maximum-frame improvement. This is a remaining performance limitation for owner review, not a passing latency threshold; none was specified or weakened.

The workload asserts **journal and search running, 200 moving entities, 80,000 loaded spawns, 10,000 distinct names and 4,441,112 journal bytes**, with eight indexed records for the selected name. It checks the same cached names survive typing, all 10,000 matching rows finish, and closed search has not observed the live list. Each phase samples 120 frames and moves all 200 actors every frame; open phase types `Perf creature 0` one character every eight frames. Cache construction also contributes to the instruction-margin check.

These are CPU timings around both Hosts' frame callbacks/workers. Play snapshot conversion, GPU submission/readback, search initial load, first-open cache construction and scheduled 30/120-second full journal save-copy are outside the timed window. It is a bounded active-world typing measurement, not live FPS, a worst-case latency guarantee or a max-size save-hitch benchmark. The fatal Lua budget is 4,000,000 instructions; the test requires over 800,000 remaining. Report maxima explicitly: means alone hide GC/OS spikes.

For context, the owner's independent pre-M7 Part C harness reported journal+search **mean 3.61 ms / max 18.97 ms**. Its setup differs from this two-Host fixture; the numbers are not a controlled speedup comparison. The final debug Part B run measured closed mean 0.578 / p95 1.091 / max 1.661 ms and open-typing mean 3.016 / p95 4.043 / max **66.139 ms**. The final full-client debug run additionally measured closed mean 0.558 / p95 0.876 / max 1.707 ms and typing mean 3.144 / p95 5.942 / max 56.354 ms. Release results above are the requested acceptance measurement; spikes remain visible rather than being averaged away.

### Not done

No live disc/terrain/server, physical Desktop picker, controller, IME, non-Windows backend or oracle result is claimed. Captures use real GPU composition with synthetic map data and no disc terrain. Tests exercise actual files with injected selection. Part C's mod/profile integration and round minimap remain untested here. LOW guide styling, browser/non-mod search availability, autorun/follow capture behavior, trimmed names, external catalogue overrides and remaining B1 LOWs retain the documented limits. Actor-position journal sampling is now fixed; the earlier protocol-only journal limitation above is superseded. No new features were added beyond the specified parity fixes.

The final native gate is `cargo build --release --locked -p eqoa`, run after this report commit under the same shared lock/target protocol. Its outcome is recorded in the six-line handoff and ignored `run/r4-fix/final-native-release.log`. No build or test is left running at handoff.

## Part C: round minimap mod and upstream's map files back (2026-11-11)

Branch `feat/eqoa-api-r4c` (from `feat/eqoa-api-r4` at d9f177a; develop merged first, 3a096b7). Not pushed or merged. Independent review and the owner's playtest are pending.

### What changed

- **New bundled mod `eqoa_round_minimap`** (on by default; `bundled-mods/eqoa_round_minimap/`, about 230 lines of Lua). It claims `eqoa.hud.hide("minimap")`, holds one `eqoa.map.view{size=112}` and shows the picture (cropped by the frame border) with `eqoa.hud.image{shape="circle"}` in the rectangle from `eqoa.hud.layout().minimap` (so it follows Move HUD), with a ring. The content is upstream's own minimap drawing: terrain, indoor view, camps, coaches, quest marks, waypoints, group, and the creature/player/pet dots in their name colours. Hovering a creature, player or pet gives the card (name, level, Player/Pet/NPC, distance; Space Mono, gold title); hovering a camp/coach/town/quest/waypoint mark gives its name. The wheel inside the circle goes to `eqoa.input.minimap_wheel`; clicks, wheel and pointer over the circle are claimed with 14 strips that follow the circle's edge. N works because the view is empty while the stock setting is off. Mod off = upstream's square minimap, untouched.
- **Rim arrows**: upstream's own rim marks (waypoint, group waypoint, quest steps) are clamped to the square's edge, so the circle crops the ones that point diagonally. The view offers only `layers="all"`, so the mod draws its own arrow just outside the ring for the player's waypoint and the group waypoint, only where the square's mark would be cropped. A quest step pointing diagonally off the edge is still cropped (see gaps).
- **API additions** (neutral, in fork-owned files): `eqoa.map.get()` now has `minimap` (stock setting on and in view, whether or not a mod hides it), also in the map-change fingerprint; documented in the spec. The Map Search mod follows `map.minimap` instead of `hud.visible("minimap")`, which is false while another mod hides the stock minimap (without this its button and box vanished: observed RED).
- **Capture bug fixed (Part A's view)**: the native map pipelines write colour only, so the captured texture was cleared to transparent and stayed alpha 0 everywhere; `hud.image` drew nothing (found by reading the canvas back: 34,560 non-zero RGB pixels, 0 non-zero alpha). The canvas now clears to opaque black (ground with no art is black). Part A's synthetic-texture check could not have seen this.
- **Removed from upstream's files**: circle mask, `Dot` and hover card, masked draws, ring, theme font, badge restyle, the binding-aware legend (the M map has upstream's key legend), the `Map::off` change, fixtures in `tool.rs`/`tests.rs`/`community.rs`, `theme.rs`, the circle and legend code in `fork_ext/map.rs`, and in `render`: `map.rs`, `map.wgsl`, `ui.wgsl`, `ui.rs` (circle mask, `add_font_at_height`, `draw_range`, `draw_circle_range`) and `tests/map_circle.rs`; also the two circle test files (`map_sync_tests.rs`, `map_move_tests.rs`). `play.rs` feeds upstream's dot tuples again.
- **Two defects found by the full suites and fixed** (both came in with Part B; neither was in my brief): other mods' `eqoa.input.bind` callbacks fired on typed keys while another mod's text editor owned the keyboard (F8 toggled Stats Box while typing in a chat editor; `text_capture_blocks(vm)` now keeps them quiet, the editor's own VM still sees its keys); and `eqoa.files.load_json` of a file a mod has not written yet (the journal on first run) recorded a diagnostic in the Addon Manager (it is `not_found` now, which is silent). Both were observed RED before and GREEN after in existing tests.
- Release notes: `FORK-UPDATE-NOTES.md` Unreleased. Spec: section 7, `MapState.minimap`.

### Tests (real Manager, bundled mod files, GPU readback)

Fixture: Play fixture + a disc-free native scene (`Map::round4_fixture`/`round4_mini_frame`, a green stand-in for terrain: no disc here) + the route the viewer uses (`fork_ext::map::draw_minimap`). Negative controls were run by mutating the mod (or the search mod) and watching the named test fail, then restoring.

| Requirement | Test | Negative control (run) |
|---|---|---|
| stock minimap hidden and its clicks blocked while on, back when off; one native draw per frame | `round_minimap_hides_stock_blocks_its_clicks_and_returns_when_off` (stock takes clicks before the mod runs; 5 frames = 5 draws; mod off = 3 frames, 3 draws) | `hud.hide` removed: FAIL "the mod hides the stock minimap" |
| circle follows a Move HUD move; picture cut to a circle | `round_minimap_circle_follows_move_hud_and_clips_to_a_circle` (corner outside the circle is background while the stock square draws that corner; after the drag the picture is at the new place and gone from the old one) | image placed at a fixed rectangle instead of the layout: FAIL "picture at the new place" |
| dot under the pointer gives the right card | `round_minimap_dot_card_wheel_zoom_and_n_toggle`: Test Rat at (+40,+30) shows "Test Rat / Level 7 / NPC / 50 away"; no card away from it | hit test disabled: FAIL "a card on the dot" |
| wheel zooms the native minimap; outside the circle does not | same test: `take_wheel()` outside = 0, inside > 0, `mini_wheel` lowers the scale | `minimap_wheel` call removed: FAIL "inside the circle: a zoom request" |
| N toggles | same test: stock setting off = no round minimap, on = back | the on/off/on sequence is its own control |
| rim mark position for an off-edge marker | `round_minimap_rim_arrow_only_where_the_circle_crops_the_square_mark`: a south-east waypoint 3000 away gives one arrow just outside the ring on the south-east diagonal; an east waypoint and a near one give none | crop test forced false: FAIL "one rim arrow for the diagonal waypoint: []" |
| search button and box on the round minimap | `round_minimap_carries_the_search_button_and_box_when_the_search_mod_is_on` (F2 opens the box, red controls render) | search mod back on `hud.visible("minimap")`: FAIL "F2 opens the search box" |
| world map keeps window coordinates at Render Scale 1/2/4 | `world_map_stays_in_window_coordinates_at_render_scales` (moved from `map/tests.rs` to `fork_ext/map_render_tests.rs`; the old one relied on a fork-only capture) | runs at 3 scales x 2 sizes |

Captures for the owner (disc-free scene, green stands in for terrain; no indoor fixture without the disc): `r4-minimap-outdoors.png`, `r4-minimap-moved.png`, `r4-minimap-hover-card.png`, `r4-minimap-search.png` in this folder.

### Suites

| Run | Pass | Fail | Ignored | Explicit SKIP lines |
|---|---:|---:|---:|---:|
| `cargo test -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` | 432 | 0 | 25 | 9 |
| `cargo test -p eqoa --bin eqoa -- --test-threads=1` (once, at the end) | 2070 | 0 | 32 | 208 |
| `cargo test -p render --lib` | 47 | 0 | 0 | 0 |
| `cargo check -p eqoa --target wasm32-unknown-unknown` | PASS | | | |
| `python -X utf8 tools/test_player_release.py` | 20 | 0 | | |
| footprint `--base 11aa5f0` | PASS | | | |

The zero-addon allocation gate and round 3's four-mod allocation gate pass (the latter only after `hud_layer.rs`'s allocation-free text path was kept: see `ui.rs` below). The SKIPs are the missing disc, modules, live server, Forge reference and Windows symlink privilege as before. Bundled Wereoxxs, ARPG, Modern UI and the round 3 and 4 mods pass in the addon suite.

### Frame time (release, busy world: 200 extra creatures near the player, 300 frames each, whole frame including the fixture's native minimap and the ImGui pass)

| Mods on | mean ms | p50 | p95 | max |
|---|---:|---:|---:|---:|
| none | 1.20 | 0.78 | 3.68 | 7.25 |
| round 3's four | 2.83 | 2.40 | 5.99 | 9.31 |
| creature journal | 1.80 | 1.27 | 5.36 | 8.49 |
| journal + map search | 3.61 | 3.18 | 6.77 | 18.97 |
| round minimap, pointer away | 2.84 | 2.23 | 6.53 | 12.22 |
| round minimap, pointer on the circle | 2.91 | 2.13 | 6.30 | 25.63 |
| round 4's three | 5.30 | 4.17 | 14.82 | 49.33 |
| round 3's four + round 4's three | 5.98 | 5.23 | 11.96 | 29.64 |

The workload ran: the test asserts every wanted mod loaded and enabled, the circle was showing and the world held at least 200 entities. The machine was shared with other builds, so p95 and max are noisy (the same configuration moved by a few ms between runs); read the means. The first version of the hover scan called `eqoa.entities.list()` (200 tables) every frame and cost 12-13 ms a frame while the pointer was on the circle; it now reads the list every 30 frames and tests only the creatures near the pointer in between (about 12 ms once per 30 frames while hovering, nothing otherwise). Not measured: develop's native round minimap plus search in the same harness (it needs a second build of develop with a matching benchmark). The native round minimap's own cost is replaced by one extra full-window capture draw; the mods' share is the difference from the "none" row.

### Footprint before/after (start of round = develop 9e7257a vs this branch, against upstream 11aa5f0)

| | start of round | now |
|---|---:|---:|
| fork lines in shared (upstream) files, added / deleted | 5,417 / 269 | 1,007 / 209 |
| dry-run merge conflicts with origin/master (`git merge-tree`) | 5 files (options, play, settings, viewer, net/session) | 4 files (options, play, settings, net/session) |

Budgets in `docs/fork-footprint.json`: eleven removed (chrome, community, journal, markers, search, tests, theme, tool, render `map.rs`, `map.wgsl`, `ui.wgsl`); `map/mini.rs` 242/14 to 1/0, `map/mod.rs` 16/6 to 4/0, `play.rs` 148/55 to 95/54, `render/ui.rs` 67/2 to 1/1. No budget raised.

### What still differs from upstream in the map family (`git diff 11aa5f0 --`)

| File | +/- | Reason | Sync cost |
|---|---|---|---|
| `map/mini.rs` | +1/0 | the neutral map overlay call (guides) | one line next to `let mut r = ...raster(ui)`; conflicts only if upstream rewrites that spot |
| `map/mod.rs` | +4/0 | `#[path] mod extension` (private-accessor module `fork_ext/map_api.rs`, native only) and the overlay call in the M map draw | two small hunks |
| `render/src/ui.rs` | +1/-1 | `Font::find` made `pub` so addon HUD text streams glyph quads without upstream's per-string Vec | one word; can go if upstream exposes glyph lookup |
| everything else in `map/**`, `tracker.rs`, `groupmark.rs`, `mapsuggest.rs`, `render/src/map.rs`, `map.wgsl`, `ui.wgsl` | 0 | equal to upstream | none |

### Packaging

`bundled-mods/SOURCE.txt`, `BUNDLED_MOD_FILES` and the test list in `tools/`, and the embedded list in `fork_ext/bundled.rs` carry the five new files and refreshed hashes (including the changed `eqoa_map_search/main.lua`). `.gitattributes` keeps `mods/resources/map-entities.json` byte-exact (autocrlf made its pinned hash fail on Windows). The real packaging dry run is recorded under "Release and packaging" below.

### Parity gaps and not done

- Clicking the round minimap does not open the M map (no mod API opens the map; M works). The stock square did.
- Quest steps that point diagonally off the minimap edge are cropped by the circle (only waypoint and group waypoint get rim arrows); a `layers="map"` view option would let the mod draw every rim mark itself.
- Ground with no map art is opaque black in the circle (upstream's square shows the 3D world through it).
- No indoor capture (no disc). Terrain, tiles, real dots and real marks were never drawn in this PC's tests (green stand-in); the owner's playtest must confirm them.
- Hover cards for the quest "!" and "?" marks and the XP-camp badges use the markers list (names and labels), not upstream's richer native cards.
- develop-vs-mods frame-time comparison not measured (above).
- Runtime disable/enable of the mod was tested (stock returns, one draw a frame); a lost graphics device with a held view is covered by Part A's tests, not re-run with the mod.


### Release and packaging

Release build (`EQOA_UPDATE_CHANNEL=enhanced-dev`, `EQOA_BUILD_COMMIT=5f4a607`, shared target): PASS, 3 min 22 s, last gate. Packaging dry run with the real `tools/build_player_release.py` `build_release` (channel `enhanced-dev`, local catalogue, commit assertion): PASS; it wrote the Nightly ZIP (15.2 MB), the exe, `SHA256SUMS`, `update.txt` and `update-notes.md`, and its own verification of the bundled-mod allowlist and checksums passed. One check was relaxed for this run only, in a scratch script outside the repo: the `+crt-static` import check, because the shared local target is not built with CI's `RUSTFLAGS` (the unmodified command stops there with "imports an unbundled Visual C++ runtime"; a rebuild with CI's flags would have filled the shared target with a second full build). CI builds with `+crt-static`, so that check is CI's to pass.

## Integration (Parts A, B and C, 2026-11-12)

`feat/eqoa-api-r4c` merged into `feat/eqoa-api-r4` (head e68c8a3). Not pushed.

Conflicts and resolutions:

- `bundled-mods/eqoa_map_search/main.lua`: Part B's fix-round version kept (button placement from map chrome, toolbar, M4 claims, busy-world work). Part C's fix re-applied: the search button shows when another mod (the round minimap) has hidden the stock minimap. It reads `map.minimap` **or** `eqoa.hud.visible("minimap")`, in the draw gate and the F2 bind. Part C's bare `map.minimap` made three Part B tests fail (their fixture shows the minimap only through the HUD flag); the union keeps both cases.
- `crates/eqoa/src/map/mod.rs`: Part B's `map_toolbar::overlay` call (it notes the chrome footprint for the toolbar) kept; Part C's raster without the control font (theme.rs is gone) kept.
- `fork_ext/map_api.rs`: `api_key` hashes both `settings::map()` and `mini.up`; `MapState` carries `enabled`, `open`, `minimap`, `search_anchor`; the test fixture keeps Part C's minimap terrain stand-in.
- `addons/native.rs` MapState field list and `docs/modding/EQOA-API-SPEC.md` row: union (`enabled open minimap ... search_anchor`).
- `addons/async_json.rs` (quiet `not_found`), `addons/input.rs` and `native_state.rs` (other mods' binds quiet during text capture), `bindpage.rs` test name: both sides fixed the same things; Part B's versions kept (`mod_binding_allowed` is the stricter rule). Part C's `text_capture_blocks` is not used.
- `bundled-mods/SOURCE.txt`: union (search mod plus `spawns.lua` and the round minimap rows), every SHA-256 recomputed from the merged files; list matches the files on disk. `tools/build_player_release.py` and `tools/test_player_release.py` merged without conflict.
- `docs/fork-footprint.json`: `play.rs` budget taken to the measured +94/-53 (Part B had +147/-54, Part C +95/-54); no budget rose. `FORK-UPDATE-NOTES.md` and this report: all sections kept.
- Part C's deletions (theme.rs, map_circle test, map_move/map_sync tests) stand.

Gates: footprint PASS against 11aa5f0 (31 shared files, all at budget); `tools/test_player_release.py` 20 OK; `fork_ext::` 530 passed, 0 failed, 26 ignored, 0 SKIP; `map` filter 179 passed; full `eqoa --bin eqoa` 2086 passed, 0 failed, 32 ignored, 0 SKIP (first full run: 3 Part B search tests failed on the bare `map.minimap` gate, fixed as above and rerun); `cargo build --locked --release -p eqoa` OK. No frame-time benchmark run (the lead measures it).

## Part C fix round (2026-11-12)

Branch `feat/eqoa-api-r4` from d20328e. Sol's review and Terra's check are copied unchanged into `docs/plans/eqoa-api/round-4-review-c.md` and `round-4-check-c.md` (`d88a323`). Code and tests: `4de91e5` (harness), `eed2c5c` (fixes and tests), `37bad60` (README, hashes). No push, merge or tag.

### Findings to fixes

| Finding | Fix | Behaviour test (real Manager, GPU, Play-shaped entities) | Negative control (broken, RED, restored) |
|---|---|---|---|
| M1 hover read positions cached for 30 frames | `dot_under` reads `eqoa.entities.list()` each frame, only while the pointer is on the circle; nearest dot, lower id on a tie | `round_minimap_hover_follows_this_frames_creatures_and_picks_the_nearest_dot`: a rat that moves has no card at its old place and one at the new place (distance recomputed); a removed creature gives none; two dots 3 px apart pick the nearer whichever slot is first; equal distance picks the lower id; card per kind (Player, Pet, NPC, Level unknown); corpse and inactive give none; the native dot (magenta) is drawn under the hover position | cache the list once: RED "no card where the creature was". Tie compare flipped: RED "equal distance: the lower id" |
| M2 marker hover not tied to what is drawn | The mod places marks as the native `place_marks` does: camps and coaches only with Map Markers on, offers where they lie, steps on the picture rim, no towns, topmost last, only inside the circle. API: `MapState.markers_on` and `Marker.step` (quests) added, spec and field lists updated, `api_key` hashes both | `round_minimap_marker_hover_follows_what_the_minimap_draws`: camp card; Map Markers off gives none, on gives it back; a quest over a camp wins; a step's rim mark has its card; a step past a corner (outside the circle) has none; an offer beyond the picture has none; a town with no camp or coach beside it has none | `markers_on` ignored: RED "no card for a camp the minimap does not draw". Order reversed: RED "the quest is the topmost mark". `step` always false: RED "the step's rim mark has its card" |
| M3 card could leave the window | Lines wrapped at 260 frame px; card left of the circle, else right, then clamped on both axes to the window | `round_minimap_card_stays_inside_the_window_wherever_the_minimap_is_moved`: long name wrapped (card under 400 px); Move HUD to the far left and bottom: card inside the window and right of the circle | clamp removed: RED "left edge: card [-402, ...] is on the window" |
| M4 benchmark workload and baseline | The native minimap now gets the 200 entities as dots (and quest marks) centred on the player; render targets made once; GPU completion waited for inside the timed window; the no-mod configuration is the stock square minimap with the same dots and is the native baseline; asserts 200 dots and one native minimap draw per frame in every configuration; `R4C_SAMPLES` sets the frames timed | `round4c_frame_time_with_all_map_mods_in_a_busy_world` ran once in debug with `R4C_SAMPLES=3` (workload ran in all 8 configurations) | n/a: workload assertions only |
| M5 no render-scale coverage | `Rm` takes a window size and a render scale; mod-level matrix | `round_minimap_stays_in_window_coordinates_at_aspects_and_render_scales`: 1024x768 at 1x and 2x, 640x480 at 1x and 4x, 768x1024 at 1x and 2x (wide, tall, 4x); picture inside the circle, square corners cut, ring on the axes and diagonals at `r+1.5k`, nothing outside the ring, native dot under the hover, card inside the window, nothing hovered past the circle. The orange stand-in is a synthetic indoor-shaped picture (fills the whole square) in half the combinations | ring ignores scale: RED "ring at the diagonal". Real indoor terrain: unavailable (no disc) |
| LOW group waypoint colour | orange `(1,0.5,0.05)` as `group_waypoint.rs` | `round_minimap_rim_arrows_keep_the_waypoint_colours`: a waypoint SE magenta, the group's SW orange, read from rendered pixels | blue again: RED "the group's arrow is orange: [62,150,202]" |
| LOW pointer strips past the circle | each strip as wide as the circle is at the strip's end nearer the rim | same test: every 5 degrees a click 1.5 px outside the circle goes to the game and one inside is taken | strip width at the nearer-to-centre end: RED "a click just outside the circle at 15 degrees goes through" |
| Found while testing | The mod used one scale for both axes; the HUD frame is not square at 1024x768 (1.6 across, 1.71 down), so dots and marks were found about 6 px high. Hover now uses the width scale across and the height scale down | hover and scale tests | `ky` replaced by `k`: RED in 4 tests |
| Entity updates | The harness tells the API of each entity change as the session does (`Play::entity_slot_changed`, test only); the real runtime updates entities only then | | |

### Coverage the deleted native tests gave, now at mod level

Nearest-dot hover and card text per kind (player, pet, NPC, level unknown, corpse, inactive); circle border geometry at three aspects; rim placement of diagonal guidance (existing arrow test) with colours; render scales 1x, 2x, 4x; indoor-shaped picture clipped by the circle at the scales; hover, wheel and claim in window pixels; Move HUD (existing). Not re-added: the legend and badge restyle tests (those features were removed on purpose).

### Counts

| Suite | Passed | Failed | Ignored | SKIP lines |
|---|---|---|---|---|
| `round_minimap` filter | 10 | 0 | 0 | 0 |
| Addons, `fork_ext::addons:: -- --test-threads=1`, scratch `EQOA_CONFIG` | 439 | 0 | 25 | 9 (symlink privilege, missing EQOA_FORGE_SPIKE and golden pictures: as before) |
| Full client, `--bin eqoa -- --test-threads=1`, scratch `EQOA_CONFIG` | 2091 | 0 | 32 | disc, modules, server, Forge as before |
| `tools/test_player_release.py` | 20 OK | | | |
| wasm `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS | | | |
| Footprint against 11aa5f0 | PASS, no budget raised | | | |
| `bundled-mods/SOURCE.txt` | every hash matches the files (`main.lua`, `README.md` refreshed) | | | |
| Release build (`enhanced-dev`, shared target) | PASS, 2 min 8 s, last | | | |

A first full-suite attempt without `--test-threads=1` failed in the Forge ImGui bridge tests (they share one ImGui context); the suite is single-threaded by design and passed that way. The zero-addon allocation gate and the Wereoxxs, ARPG, Modern UI and round 3 and 4 mod tests are inside the addon run. The debug runs used `R4C_SAMPLES=3`.

### Benchmark

Timings pending: the lead runs them on an idle machine (the owner was playing on this PC, so none were taken here). The harness ran once in debug with 3 frames per configuration only to prove each workload runs: 200 dots drawn by the native minimap, one native minimap draw per frame, every wanted mod loaded. Command for the numbers:

`cargo test --release --locked -p eqoa --bin eqoa round4c_frame_time_with_all_map_mods_in_a_busy_world -- --test-threads=1 --nocapture` (shared `CARGO_TARGET_DIR`, under the lock, scratch `EQOA_CONFIG`; 300 frames per configuration).

Configurations: stock square minimap with no mods (the native baseline), round 3's four, creature journal, journal + search, round minimap with the pointer away, round minimap with the pointer on the circle, round 4's three, all seven. A frame is the native minimap's layout and drawing (with the 200 dots), the capture the mod asks for, the addons' callbacks and the ImGui pass, up to GPU completion. Develop's old native round minimap cannot run in this harness (its circle mask, dots and card are gone from the native files on this branch), so there is no number for it and none is estimated. Watch the "pointer on the circle" row: the hovered circle now builds the 200-entity list every frame (the old 30-frame cache was the staleness bug). If that row is too high, the fix is an entity delta source in the API, not a cache.

### Not done

- Clicking the circle still does not open the M map (no neutral map-open API); unchanged from Part C.
- Quest steps pointing diagonally off the edge: hover correctly gives none there (the circle crops them); the mod still draws no rim mark for them.
- A step's rim position uses a fixed 8 frame-px half glyph where the native uses the font's height; the hover can be about a pixel off the drawn glyph on the rim.
- Real terrain, real indoor pictures and the native hover cards' extra text could not be checked (no disc); the owner's playtest confirms them.
- Release timings (above).


## Hover cost fix (2026-11-12)

Cause: the current-frame hover correction built the entire guarded Lua entity list on every hovered frame. The lead measured about 0.7 ms with the pointer away and 8.9?10 ms on the circle (200 creatures, release, idle PC). Those before numbers are supplied measurements, not rerun here.

Kept the inherited compact current-frame dot capture, list-record reuse (fresh list containers, read-only cached elements), and regression coverage. Removed the unfinished profiling hooks. Compact dot columns alone measured 1.557 ms mean / 4.186 ms max; batching projection into those columns measured 1.800 / 3.919. These intermediate results did not meet the goal.

The final mod calls `eqoa.entities.pick_dot(query)`: one native pass over current actor positions using this frame's minimap projection, player/group exclusions, circle clipping, nearest distance and lower-ID tie breaking. Only the selected ID needs a full entity lookup for its name/level/kind. There is no position or hover-result cache. `eqoa.entities.dots()` also exposes the compact world-position columns for addon authors; both helpers are documented. All implementation remains fork-owned.

### Final release benchmark runs

Required command, run twice sequentially on unchanged final code, 300 frames per configuration:
`cargo test --release --locked -p eqoa --bin eqoa round4c_frame_time_with_all_map_mods_in_a_busy_world -- --test-threads=1 --nocapture`

Each cargo command used `CARGO_TARGET_DIR=%USERPROFILE%\Paseo\targets/shared`, a fresh empty temporary `EQOA_CONFIG`, the shared `cargo-build.lock` directory (wait/retry, remove in finally), and a foreground subprocess with a 30-minute timeout. No other build/test workload ran during either measurement. No worktree target directory was created.

Run 1:

```text
R4C frame time (release, stock square minimap, no mods (native baseline)): frames=300 mean=0.363 ms p50=0.316 p95=0.643 max=1.880
R4C frame time (release, round 3 mods (4)): frames=300 mean=0.863 ms p50=0.773 p95=1.345 max=2.498
R4C frame time (release, creature journal): frames=300 mean=0.475 ms p50=0.417 p95=0.778 max=1.099
R4C frame time (release, journal + map search): frames=300 mean=1.595 ms p50=1.585 p95=3.737 max=5.220
R4C frame time (release, round minimap, pointer away): frames=300 mean=1.270 ms p50=1.206 p95=1.904 max=2.417
R4C frame time (release, round minimap, pointer on circle): frames=300 mean=1.528 ms p50=1.493 p95=2.217 max=3.350
R4C frame time (release, round 4 mods (3)): frames=300 mean=2.224 ms p50=2.215 p95=4.049 max=8.285
R4C frame time (release, round 3 (4) + round 4 (3)): frames=300 mean=2.928 ms p50=2.802 p95=5.038 max=9.179
```

Run 2:

```text
R4C frame time (release, stock square minimap, no mods (native baseline)): frames=300 mean=0.321 ms p50=0.282 p95=0.511 max=1.061
R4C frame time (release, round 3 mods (4)): frames=300 mean=1.198 ms p50=1.122 p95=1.854 max=3.118
R4C frame time (release, creature journal): frames=300 mean=0.484 ms p50=0.427 p95=0.764 max=1.387
R4C frame time (release, journal + map search): frames=300 mean=1.648 ms p50=1.607 p95=3.709 max=6.239
R4C frame time (release, round minimap, pointer away): frames=300 mean=1.072 ms p50=1.019 p95=1.629 max=2.201
R4C frame time (release, round minimap, pointer on circle): frames=300 mean=1.236 ms p50=1.131 p95=2.006 max=2.546
R4C frame time (release, round 4 mods (3)): frames=300 mean=2.494 ms p50=2.406 p95=4.451 max=8.438
R4C frame time (release, round 3 (4) + round 4 (3)): frames=300 mean=3.227 ms p50=3.083 p95=5.584 max=9.110
```

Pointer-on-circle means: **1.528 / 1.236 ms**, maxima **3.350 / 2.546 ms**. The combined 600-frame mean is **1.382 ms** (about 84?86% below the supplied before range). Both maxima meet <4 ms; run 2 meets <1.5 ms mean, while run 1 misses it by 0.028 ms. The overall mean meets the target; two independent per-run passes are not established. Pointer-away means were 1.270 / 1.072 ms, so the final hover increment was 0.258 / 0.164 ms.

### Validation

- Final required `fork_ext:: -- --test-threads=1`: **536 passed, 0 failed, 26 ignored**, including all **10 round-minimap tests** (current-frame motion/despawn, nearest/ties, cards, markers, scales, clipping and Move HUD).
- Native helper test `round4_entity_dots_are_plain_columns_of_the_creatures_the_native_minimap_draws`: final release **1 passed**. Covers drawn-creature eligibility, actor-position fallback, independent column copies, per-frame refresh without snapshot reconstruction, projection/picking, excluded IDs, no hit and invalid scales.
- Negative controls: disabling corpse filtering fails the eligibility assertion; disabling the picker's exclusion check fails `excluded dots cannot be picked`. Both restored, positive tests pass.
- Full client suite before the final native-picker refinement: **2,093 passed, 0 failed, 32 ignored**. The final picker was subsequently checked by the complete fork suite and the focused native-helper test; the full client suite was not repeated after that refinement. Disc/oracle/golden-fixture coverage is not claimed; ignored tests remain ignored.
- `python -X utf8 tools/test_player_release.py`: **20 passed** on final code; bundled Lua SHA-256 refreshed in `bundled-mods/SOURCE.txt`.
- `python tools/check_fork_footprint.py --base 11aa5f088bb52d5f306b0224e37b782b1bf4503d`: **PASS**, 31 shared files, no budget changes.
- `cargo build --locked --release -p eqoa`: **PASS**, final native client, 1m 49s (shared target). All bundled SHA-256 entries independently verified: **88 match**. Diff whitespace check passed with CRLF-aware settings.

## Owner-test fix round (2026-11-13)

Owner's live test found seven things. Work on `feat/eqoa-api-r4` as LoopRook; no push, merge or tag. develop was merged in
for its privacy guard (`tools/check_no_local_paths.py`: exits 0 here; the tools tests pass); round 4 docs now use
placeholders. The disc was used through `$EQOA_ISO` (and, for the suites, a hard link at the ignored `run/eqoa.iso`, removed after).

| # | Fix | Test and negative control |
|---|---|---|
| 1 | Square inside the circle. Cause (measured on the disc's own art, not the stand-in the earlier tests used): upstream's frame art reaches 5 frame pixels farther in at the sides and 2 at top and bottom than the 6-pixel border, so it covers the picture's edge. A held view's capture now leaves the frame art out (two hook lines around upstream's untouched `blit_frame`, `fork_ext/map.rs::capture_without_frame`); stock draws keep it. | `round_minimap_shows_no_square_frame_inside_the_circle_with_the_discs_art`: real UI.CSF art and the SUPPORT atlas from the disc; every 3 degrees round the circle but the "N", 1/2/4 pixels in, must be the terrain colour. RED before (grey frame pixels at the left/right tangents), GREEN after; control: the stock square's own frame edge is found by the same sampling. Outdoors only: an indoor capture was not made (the capture and frame removal do not depend on indoors, but no indoor world is available to a test). |
| 2 | Zoom. The mod forwards every notch whole; both ends of upstream's range are reached (outdoors 6 to 2 units a pixel in five notches, out to 25 in twelve, the limits hold; five notches in one frame arrive as five). The limits are upstream's. Backlog entry added (`docs/backlog.md`), nothing built. | `round_minimap_wheel_reaches_both_ends_of_upstreams_zoom_in_whole_steps`, control: one notch short is still above the limit. Not covered: indoors (its limits are `indoor::SCALE_LIMITS`; the same forwarded value reaches the same `mini_wheel`, but a test cannot enter upstream's private indoor state). |
| 3 | Addon Manager errors. Round minimap: no claim for the zero-wide top/bottom strips or an unready rectangle; no `Marker.world_id` (markers are already one world's). | Zero-diagnostics gate `every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics`: all 11 bundled packages (the eight `eqoa_*` and the three Forge UIs), each through the real Manager on Play-shaped data: entry, minimap hidden/shown, Move HUD, pointer and wheel, map open/closed, zoning, disable/enable. Run against the previous round minimap it fails with exactly the owner's two messages (44 and 1,818 occurrences in that session); control `the_zero_diagnostics_gate_fails_on_a_deliberately_bad_call`. |
| 4 | Search box and button sizes: native 20 square button, 252 by 300 panel, 28-pixel rows, 12-pixel font, all times the HUD scale (minimap height over 112), as the native search drew them. | `round_minimap_search_button_and_box_keep_the_native_sizes_at_every_scale` at 640x480, 1024x768 (1x, 2x) and 1920x1080; control: not the old fixed 336 by 400. The journal footer test now clicks Export/Import where the scaled buttons are. |
| 5 | One Options page, **Enhanced**, after upstream's last: Enhanced HUD rows first, then Mods & Addons rows; same row ids and saved keys; `fork_ext/options.rs` only. Player text now says Options -> Extended -> Enhanced -> Addon Manager. | `the_fork_pages_and_rows` walk (every row, toggle and action) on the one page; controls: no second fork page, the page is taller than a game page, upstream's pages still fit. Window assertions (page and list of pages on the 448-high screen) use the test font (16 points); the real font's size was not measured. |
| 6 | Frame rate, below. | |
| 7 | Terra's leftovers: hover card cuts a single oversized word with "..." (and never grows taller than the window); Move HUD placement at 2x and 4x; the `view.project` inside assertion is back (with a far-point control). | `round_minimap_card_cuts_a_single_oversized_word_with_an_ellipsis` (control: spaced words wrap uncut), `round_minimap_follows_move_hud_at_render_scales_two_and_four` (three window/scale cases, moved and old places). No separate mutation run for the Move HUD test. |

### Frame rate (item 6)

What could and could not be measured. There is no scripted scene runner that draws a real world with the minimap and mods: `playshot`
needs a live server and `mapshot` draws only the map. So this is the existing benchmark harness (a Play-shaped session through the
real Manager, 200 creatures, the native minimap chrome from the disc's own UI art over a terrain stand-in, 1024x768, the whole frame to
the GPU's completion, 300 frames, release, alone, twice). It has no 3D world, so the absolute times are small; compare the rows.
develop's 0.5.3 native round minimap was **not** measured: that tree has no addon harness and there is no runner to put it in.
The GPU cost of the offscreen capture is the difference in "GPU wait" between the stock row and any round-minimap row.

| Configuration (mean ms per frame; CPU to submit + GPU wait) | Run 1 | Run 2 |
|---|---|---|
| All mods off (upstream square minimap) | 0.47 (0.32 + 0.15) | 0.27 (0.17 + 0.09) |
| Round minimap only, pointer away | 1.09 (0.90 + 0.19) | 0.93 (0.77 + 0.16) |
| Round minimap only, pointer on circle | 1.24 (1.05 + 0.19) | 1.04 (0.89 + 0.16) |
| Round 4's three mods (journal, search, round minimap) | 2.71 (2.46 + 0.25) | 2.05 (1.87 + 0.18) |
| All seven (round 3's four + round 4's three) | 3.39 (3.16 + 0.23) | 2.78 (2.59 + 0.19) |

p95 at the all-seven row: 5.7 and 4.4 ms; max 14.4 and 8.6 ms. Plainly: the offscreen capture itself costs about 0.05 to 0.1 ms of GPU
time here; the rest of the round minimap's cost is Lua and ImGui on the CPU (about 0.6 ms), and **Map Search is the largest single mod**
(journal plus search alone is 1.6 ms). The runs differ by 0.2 to 0.7 ms from machine noise; none of this is a real-world fps claim.

### Gates

All cargo runs foreground-equivalent under the shared lock and `CARGO_TARGET_DIR`, one test thread, scratch `EQOA_CONFIG`, `EQOA_ISO` set.

| Suite | Pass | Fail | Ignored | SKIP lines |
|---|---:|---:|---:|---:|
| Addons `fork_ext::addons::` | 447 | 0 | 25 | 9 (four symlink-privilege, pinned Forge grid and hearts, three bundled picture goldens) |
| Full client, `EQOA_ISO` only | 2100 | 0 | 32 | 208 |
| Full client, also `run/eqoa.iso` (hard link to the same disc) | 2100 | 0 | 32 | 49 |

Finding: **`EQOA_ISO` alone unlocks none of the 208 disc tests**; they read the checkout's `run/eqoa.iso`. With that file present (a hard link,
no copy, git-ignored) 159 more disc tests ran and passed (25 minutes); the 49 left need a 2002 disc, `run/client.bin`, live S2 files or
a save state. `cargo check --target wasm32-unknown-unknown`: PASS. Release build: PASS, last. Footprint: PASS, with two budget rises that
need review: `map/mini.rs` +1 to +3 (this round's frame-art hook) and `crashfile.rs` +2/-1 to +8/-2 (develop's privacy commit, brought by the
merge; develop's own manifest needs the same change), plus the two new privacy-tool files listed as fork-owned.
`python -X utf8 tools/test_player_release.py`: 20 passed; `SOURCE.txt` hashes refreshed for the three edited files.

### Not done

Indoor capture and indoor zoom were not tested; the real world scene and develop 0.5.3 comparison could not be run; the Move HUD 2x/4x and
ellipsis tests had no separate mutation run; Options window height was checked with the test font only.

## Performance round (2026-11-13)

Branch `feat/eqoa-api-r4`, five steps, one commit each (plus fixes). Not pushed, merged or tagged. The numbers are from the clean timing batch at the end of this section (owner's PC idle, release, alone, twice each, before and after); figures quoted in the steps are from that batch.

### How it is measured (step 1)

`EQOA_PERF` now also prints, from fork-owned code (`fork_ext/addons/addon_perf.rs`), every 5 seconds:

```
addons: <id> <ms> (lua x, api x, bridge x, draw x), ..., imgui x
core: <shared native work> <calls per frame>x <ms>, ...
addon-api: <id> <call> <calls per frame>x <ms>, ...   (the costliest eqoa.* / frontiers_forge.* calls)
```

`lua` is the addon's own Lua (callbacks and render events) with its native calls taken out; `api` the `eqoa.*` / `frontiers_forge.*` calls; `bridge` the Forge `ImGui` table and draw lists; `draw` the native HUD layer drawing the addon's queue; `core` the shared work (deferred capture flush, event emit/deliver, record builds, guide update/draw). Nothing is timed, stored or wrapped with `EQOA_PERF` unset (the ImGui and adapter wrappers are put in at addon load only when it is set). Test: `addon_perf_line_splits_each_addons_lua_from_its_native_calls` (control inside it: timers off, nothing stored).

### Step 2: the guide (about 8 ms)

Cause, found with the new line in a scene where creatures' updates reach the API (the earlier benchmark had none, so no event ever fired): with a guide set the search mod re-read the world on every `entity.*`/`player.changed` event, i.e. every frame, and its `observe_state` called `eqoa.entities.list()`. That list was cached whole: any creature moving rebuilt all 200 guarded records (11-15 ms measured). The native side was not the cause (420 dots, one dashed line: 0.03 ms). Fix, in the mod only: no subscription to the entity and player streams (they also cost a record per event); with a guide set and the panel closed only the guide's own target is read (`player.get`, `map.get`, `group.get` for a member, `entities.named(name)` for a creature); the open panel re-reads on a 0.25 s timer or when something changed. Capability `eqoa.entities.named` added to the mod. Before: the guide cost 11.5-11.6 ms a frame; after: 0.11-0.15 ms (batch below). Test `round4_guide_costs_under_half_a_millisecond` (release: asserts < 0.5 ms; guide of 420 spawns; asserts the guide is set, its dots drawn on the minimap, and 200 creatures walking with events). Its red run is the before row of the batch (the old Lua fails the 0.5 ms assertion at 11.5 ms). The existing search and journal tests (23) pass.

### Step 3: the seven mods in a real-shaped scene

Harness (`perf_round_tests.rs`, `round4_seven_mods_in_a_real_shaped_scene`): real Manager, 200 creatures moving every frame with the session's update reaching the API for 1 in 8 per frame (`R4P_CHURN`, about 25 events a frame; the earlier runs used 1 in 4), the player running (so the snapshot cannot be reused), the stock minimap chrome from the disc's art, the native HUD layer drawn through a `Ui` to the GPU (the disc's atlas and strings; the surfaces and font are stand-ins), the ImGui pass, to the GPU's completion. The two Round 3 toggles (`stats_box`, `target_of_target`) are on; it asserts 200 dots and that the HUD commands were drawn (stats box 33 a frame, numbers 3).

Causes found and fixed (all fork-owned):

| Cause | Fix | Measured |
|---|---|---|
| Every record (creature, player, event payload, list element) was built with about 50 protected `mlua` calls (the VM runs under a memory limit) | `record_raw.rs`: the whole record in one protected call, tested equal to the old build (`raw_record_build_matches_the_call_by_call_build`, with a depth-fallback control and a counter that proves the fast path ran) | creature record 27-38 us to 8 us (micro-benchmark) |
| `entities.list` rebuilt every element when any one changed | per-element reuse by id | with the guide fix: the guide's 11.5 ms to 0.1-0.15 (batch) |
| Each event published to every addon deep-copied its JSON (7 addons, 7 copies); `Core::emit` 92 us | payload is an `Arc` | `events.emit` 1.31 to 0.22 ms a frame (25 events, batch) |
| Journal asked `player.get()` on every entity event, before its cheap checks | checks first, player read once a frame | part of the journal's 2.54 to 0.83 ms (batch) |

Before/after per mod and together: see the timing batch below (all seven +11.2-11.3 ms before, +4.4-4.6 after).

What is left, and why the seven are still above 2 ms, is in the timing batch below. Not done: lazy event payloads (a handler reading only `after` still pays for `before`), an index for the deferred entity patch (a linear `position` scan of the list).

### Step 4: the Forge layer against Wereoxxs

Harness `round4_forge_uis_split_between_our_layer_and_their_lua` (same scene, one UI at a time; Wereoxxs with all eight of his windows turned on, as the owner runs it; ARPG and Modern UI as shipped). Numbers are in the timing batch below: about a third of the Forge UIs' own time was ours before (adapters 0.26-0.59 ms, bridge 0.11-0.20), and is 21-30 % after. The scene is a fixture (few abilities, an empty chat), so the owner's 15 ms of Wereoxxs is mostly his Lua and scene size, which this does not reproduce.

Fixed in our layer (`fork_ext/addons/`, no Forge mod file touched): the ability, item and bar-slot objects were tables of about 20 closures each, rebuilt on every `GetAbility`/`GetAbilitySlot`/`GetItem` call (about 25 us a call, 10-16 calls a frame); they are now reused while the `Copy` snapshot they came from is unchanged (`object_cache.rs`). `entity_list.*` compiled `require('frontiers_forge.entity')` from source on every call; it is now found once per VM. After: adapter time falls to 0.09 (Wereoxxs), 0.05 (ARPG) and 0.20 ms (Modern UI). Remaining in his Lua, not fixed: 0.75-1.0 ms each; for Modern UI also `GetAllEntities` (24 `Entity.new` calls through Lua). Test `forge_adapter_objects_are_reused_only_while_their_snapshot_is_unchanged` (the same object while unchanged; a changed cooldown or icon gives a new object with the new values); the existing Forge suites (adapter surface, picture and icon tests, ability bar tests) pass unchanged. One behaviour change to know: a script that stored its own field on an ability object would now find it on the next call while the snapshot is the same.

### Step 5: "play" 13 ms with all mods off

Test `round4_play_phase_hooks_do_nothing_when_no_addon_is_enabled` (busy fixture, 200 creatures, no addon): `forge_snapshot` (inactive path), `begin/end_game_event`, `frame_library`, `begin_iteration`, `health_redraw` and `health_hidden` each run a warmed loop and print their mean; it asserts nothing was captured or queued (no entity records, empty deferred queue). Release, 2000 frames: `forge_snapshot` 0.0038 ms a frame, `frame_library` 0.0002, `begin_iteration` 0.0001, the rest under 0.0001 (about 0.004 ms together, against the 0.3 ms line), so no change was made: the fork's hooks are lazy already (`capture_requested()` gates the capture; the inactive paths only reset a few thread-locals). The 13 ms is upstream's own play phase (movement, poses, entity updates); not touched.

### Gates

| Suite | Pass | Fail | Ignored | SKIP lines |
|---|---:|---:|---:|---:|
| Addons `fork_ext::addons::`, one thread, `R4C_SAMPLES=5` | 455 | 0 | 25 | not counted (output not captured) |
| Full client, one thread, `R4C_SAMPLES=5`, `EQOA_ISO` and `run/eqoa.iso` | 2109 | 0 | 32 | 0 (the full run had no SKIP lines printed) |

Footprint PASS (no shared file touched, no budget changed); `check_no_local_paths.py` clean. wasm check PASS; release build PASS (3m52s, last); `test_player_release.py` 20 passed; `SOURCE.txt` hashes current.

### Timing batch (2026-11-14, owner's PC idle)

Every benchmark above, release mode, alone (the shared cargo lock held), one test thread, 300 frames each (`R4C_SAMPLES=300`; hooks 2000), run twice. **After** is d13d0b7c. **Before** cannot be bb032292 itself (the harness and the timing line did not exist there), so it is d13d0b7c with each fix switched off in a scratch branch (since deleted): the old search and journal Lua restored from bb032292, the call-by-call record build, the whole-list cache, a deep copy of every event payload per addon, no adapter object reuse, `require('frontiers_forge.entity')` compiled per call. The timing line itself stays in both. Both runs of each agree within about 5 %. Scene as described under step 3 (25 entity updates a frame). All figures are mean ms a frame.

**Per mod, the mod's own time (lua + api + bridge + draw), each mod alone:**

| Mod | Before | After |
|---|---:|---:|
| Stats box | 0.17 | 0.16 |
| Target of target | 0.04 | 0.04 |
| Mana bars | 0.06 | 0.06 |
| HUD numbers | 0.19-0.20 | 0.13 |
| Creature journal | 2.54 | 0.83-0.86 |
| Map search (no guide; own, journal data loaded) | 2.88-2.89 | 0.12-0.13 |
| Round minimap | 0.18-0.26 | 0.17 |

**Whole frame over the no-mod frame (0.28 ms), each alone and the seven together:**

| | Before | After |
|---|---:|---:|
| Stats box / target of target / mana bars / HUD numbers | +0.97 / +0.81 / +0.81 / +0.97 | +0.95 / +0.78-0.81 / +0.78-0.80 / +0.89 |
| Creature journal | +4.49 | +2.35-2.48 |
| Journal + map search | +7.8-7.9 | +2.8-2.9 |
| Round minimap | +1.15-1.75 | +1.0-1.1 |
| **All seven** | **+11.2-11.3** | **+4.4-4.6** |

The whole-frame figure includes about 0.8 ms that any enabled mod pays for the shared native capture (below), which is why a mod costing 0.04 ms of its own shows +0.8.

**The guide (420 known spawns, the search mod with the journal's data, minimap and round minimap on, 200 creatures walking):**

| | Before | After |
|---|---:|---:|
| Map search, own time, no guide | 2.97-2.99 | 0.12-0.16 |
| Map search, own time, guide set | 14.4-14.5 | 0.25 |
| Guide's native update and drawing | 0.03 | 0.02 |
| **Guide cost** (own time + native, guide minus no guide) | **11.5-11.6** | **0.11-0.15** (under the 0.5 ms asserted) |

**Forge UIs, our layer against his Lua (own time; the `ImGui` bridge includes the draw lists; adapters are `frontiers_forge.*`):**

| UI | Total before to after | His Lua | Our adapters, before to after | Our bridge |
|---|---|---:|---|---:|
| Wereoxxs (all eight windows on) | 1.49-1.55 to 1.26 | 1.00-1.06 to 1.00 | 0.30-0.32 to 0.09 | 0.17 |
| ARPG | 1.13-1.28 to 0.90-0.91 | 0.76-0.86 to 0.75 | 0.26-0.29 to 0.05 | 0.11-0.13 to 0.11 |
| Modern UI | 1.78 to 1.16-1.20 | 1.00 to 0.77-0.79 | 0.59 to 0.20 | 0.19-0.20 |

So our layer was 31-41 % of these UIs' own time before and is 21-30 % after; what remains is mostly his Lua (66-79 %). Whole frame over no mods: Wereoxxs +2.43-2.53 to +2.14-2.15, ARPG +2.09-2.44 to +1.83-1.84, Modern UI +2.78 to +2.08-2.19. In this fixture (few abilities, an empty chat) Wereoxxs costs about 1.3 ms; the owner's 15-20 ms was a full chat, effects and group, which this does not reproduce.

**Play-phase hooks with no addon enabled (release, busy fixture, 2000 frames):** `forge_snapshot` 0.0038 ms, `frame_library` 0.0002, `begin_iteration` 0.0001, `begin/end_game_event` and `health_redraw`/`health_hidden` under 0.0001. Together about 0.004 ms, against the 0.3 ms line: nothing to make lazy; the 13 ms is upstream's.

**Are the seven still above 2 ms? Yes: +4.4-4.6 ms over no mods.** Where it goes (after, per frame): the shared native capture about 1.5 ms (`api.refresh`: applying about 25 entity updates, 1.16 ms, and the player's record, 0.35), event delivery about 1.7 ms (182 deliveries, mostly the journal's), and the mods' own time about 1.7 ms. **The most expensive mod is the creature journal, 0.83-1.0 ms of its own**, because it subscribes to `entity.added` and `entity.changed` and each of the 25 updates a frame is delivered to it as a record of the creature before and after the change, of which it reads only the after (a handler reading one field still pays for both); the other six together own 0.7 ms. The next cost is not a mod: the capture exists as soon as any mod is on. No new fixes were started, as asked. The options, if wanted: build the event's `before` only when read, and index the entity patch (a linear scan of the list per update).

### Second pass (2026-11-14): the seven mods to 2 ms

Asked for: the seven mods together at 2 ms or less over no mods in the same harness. **Not reached: +4.6-4.8 ms, from +4.9-5.2 for the same scene measured at the same time** (about 7 % better). What was done, what each step gave, and why the rest cannot go without changing what mods see:

| Step | Done | Result |
|---|---|---|
| 1. `before` built lazily | Change events (`entity.*`, `group.changed`, ...: any event whose data has `before` and `after`) now build those two records on first read; the same table every time, read-only, a missing field still diagnosed, an absent `before` (entity.added) nil. Test `change_event_before_and_after_are_built_only_when_read` (the build counter is the control: a handler reading only `after` leaves `before` unbuilt; reading it later builds exactly one record and gives the values an eager build gives) | measured effect nil: the journal reads `after`, and the `before` record was only about 8 us of a ~60 us delivery, about what the lazy closure costs. Kept, tested. |
| 1b. envelope | The event envelope table is made in one protected call and its guard no longer builds a list of keys per event | envelope 0.87 to 0.58 ms a frame |
| 2. entity patch | `entity_hint` (remembered place, verified before use, a scan only on a miss); `changed_fields` without cloning keys; no second copy of the record when no addon listens for the event. Test `entity_updates_keep_the_list_right_through_removals_and_the_changed_keys_are_exact` (removal shifts the places; the scan fallback finds the moved creature; no duplicates) | the scan was 0.1 ms of 0.9; patch 0.95 to 0.85 |
| 3. lazy capture per area | **Stopped, not done.** Capturing entities only once an addon asks for them cannot be done without a mod seeing different data: the first `eqoa.entities.*`, `target.get`, `group.get` or `player.get` after enabling would find an empty or stale list until the next frame's rebuild (the API cannot rebuild it from inside a Lua call: the game state is not reachable there), and readers are spread over a dozen calls (a missed one would be a silent bug). With the seven mods on, every area is in use anyway, so it would not have helped this target; it would help a single mod alone (about 0.8 ms each). | |
| 3b. reuse unchanged parts | The player record is rebuilt every frame because the key includes the position (the player runs); its stat view is 0.2 ms of the 0.6. Memoising it needs equality on `net::dump` types (upstream's crate, not fork-owned): stopped. | |
| 4. journal's own Lua | Names are trimmed once, the grid is keyed by numbers, not strings built per lookup (identical results); the journal's own Lua is about 0.2 ms: its 0.8-1.0 ms in the table above is mostly the shared entity capture, which the timing line charges to whichever addon runs first. | nothing left in the Lua |

Interleaved A/B, three rounds each, the two binaries run alternately, same scene (release, alone, 300 frames): **all seven over no mods: d13d0b7c 4.94 / 5.05 / 5.24 ms, now 4.58 / 4.75 / 4.83**; journal alone 2.94 / 2.78 / 2.85 to 2.68 / 2.38 / 2.30; the guide 0.165-0.183 to 0.130-0.158 ms. (The PC was not as quiet as in the first batch, so the absolute figures are higher than above; only the paired comparison counts.)

**Where the +4.7 ms is, per frame (profile of the new head):** the entity updates applied to the API, 25 a frame at about 35 us each, 0.85 ms; the player's record 0.6; the event envelope and handler for the journal about 0.6 and 0.35; the other six mods' own render callbacks about 1.0 together; the rest the manager's per-frame dispatch and the snapshot (about 0.7). The most expensive single thing is the shared entity capture plus its event delivery (about 1.8 ms with the journal on), which any mod that reads creatures pays; the most expensive mod is the creature journal. Going below 2 ms needs the capture to stop producing a full record per creature update, which is a design change (typed deltas the mods read on demand), not a tuning pass.

Gates after the second pass: addons suite (one thread, `R4C_SAMPLES=5`) 456 passed, 0 failed, 25 ignored; full client suite 2111 passed, 0 failed, 32 ignored (a first run had one failure, `addon_perf_line_splits_...`, from 81 stale `eqoa-forge-manager-*` temp folders left by killed runs colliding with a new scratch folder: not code; cleared and rerun green); wasm check PASS; release build PASS (last); `test_player_release.py` 20 passed; footprint PASS; no local paths; `SOURCE.txt` hashes current.
