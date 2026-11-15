# Round 4 Part A review 2 (items 7-10, circle-mask test)

Range `3ffaaa4..459179d`, branch `feat/eqoa-api-r4`. Read-only review; no cargo run. Items 1-6 are
covered by `round-4-review-a1.md` and not repeated.

## Verdict: APPROVE WITH CHANGES

Item 7 (map-open input) and item 10 (entity fields) are sound. Items 8 and 9 do what the brief
asks while the process is running, but the journal save can lose its last state at quit (HIGH),
the opt-in 128 MiB heap has no gate, and several failure paths and tests are thin. Fix the HIGH and
the MEDIUMs below before Part B relies on `save_json_async` for the journal.

## HIGH

### H1. The save worker is never joined or flushed, so the last journal state is lost at quit
`json_save.rs:96` spawns a detached thread and drops the `JoinHandle`. `impl Drop for Service`
(`json_save.rs:53`) only sets `closed` and notifies; it does not wait. `Runtime::shutdown`
(`runtime.rs:440`) and `Manager::shutdown_inner` (`manager.rs:925`) call the mod's `disabling`
hook, then retire the VM, and the event loop then returns from `exiting` and the process ends.
Detached Rust threads die with the process.

Failure scenario: the journal mod saves on `disabling` or on logout (the natural flush point). The
call returns in about 4 ms (snapshot only). Encoding 14 MiB, `sync_all` and the rename take roughly
0.1-0.5 s on the worker. The player closes the window; the process exits mid-write. The temp file
`.addon-<pid>-<n>.tmp` is left behind and the previous journal stays. The newest state is gone,
silently, with no `resource.failed`. Atomicity holds (no corruption); durability does not.

The report and spec say "accepted writes drain when the VM is dropped". That holds only while the
process lives. No test exercises drain-on-drop or exit (grep for `drain`/`drop(host)` in
`json_save.rs`, `file_io.rs`, `async_json.rs` finds none).

Fix: keep the `JoinHandle`; in `Drop for Service` (or a `flush()` called from
`Manager::shutdown_inner` and `disable`) wait for pending plus active work with a bounded timeout
(for example 3 s). Add a test: queue a near-14 MiB save, drop the host, assert the file holds the
latest value afterwards, and a negative control without the join.

## MEDIUM

### M1. Any mod gets a 128 MiB Lua heap with one call; no gate
`async_json.rs:197` (`load_json`) and `json_save.rs:279` (`save_json_async`) call
`lua.set_memory_limit(JSON_HEAP)` after only path validation. Opt-in is "make one valid call"; there
is no manifest permission or player-visible capability, and the raise is permanent for the VM.
`MAX_INSTANCES = 16` (`manager.rs:29`), so the worst case is 16 x 128 MiB = 2 GiB of Lua heap from
ordinary mods, each of which can then allocate freely up to the cap. A mod abuses it by calling
`eqoa.files.load_json('resources/map-entities.json')` once.

Memory outside the Lua limit is also unbounded by it. The worker parses up to 16 MiB into a
`serde_json::Value` before the 2M-node quota runs (`async_json.rs:75`, validate after parse). A
16 MiB file of small values (`[0,0,...]`, about 8M elements at about 32 bytes each) transiently
costs a few hundred MB per request, two in flight. Each save snapshot (`snapshot()`) is bounded by
14 MiB of string bytes but by node count (2M) for structure, and one active plus one pending copy
exist per VM.

Fix: require a manifest declaration (or limit the raise to a smaller figure such as 48-64 MiB), and
check the node quota while parsing or cap the file at a size consistent with the node quota.

### M2. A Lua memory error during materialisation faults the whole mod instead of `resource.failed`
`async_json.rs:152`: `from_json(lua, &v)?` propagates a `MemoryError` through `poll` and
`native_events::dispatch` (called with `?` in `host.rs:557`), so `frame_entry` fails and
`Manager::fail("frame")` retires the mod. A file inside the caps (16 MiB, 2M values) can still
exceed 128 MiB of Lua tables: 2M values at about 40-80 bytes each is 80-160 MiB, and the heap
already holds the mod's own data. The mod never receives a `resource.failed` event, and it is
disabled.

Fix: catch the error from `from_json` and deliver `json_resource(..., None, Some("exceeds Lua memory"))`.
Lower `NODES` or compute a Lua-size estimate. Add a test with a node-heavy file under a small
memory limit.

### M3. Materialisation cost on the VM thread is unmeasured and not sliced
The real catalogue has 70,579 JSON values (counted from `mods/resources/map-entities.json`). The
whole tree is built in one `poll()` call during `dispatch`, plus `set_metatable` with a registry
lookup per array. Estimate for release: roughly 5-20 ms, i.e. one dropped frame at 60 fps when the
mod first loads it (debug builds are several times slower). The report gives no measurement ("not a
guarantee that materializing a large table fits one frame"), and the brief's gates wanted measured
frame costs. The worst case under the caps (2M values, two completions polled in the same frame)
is hundreds of ms to seconds.

Fix: time `from_json` on the real 1.7 MB file in a test and record it; consider polling at most one
completion per frame and chunking big arrays/objects across frames, or document the first-load hitch
for Part B (load during the loading screen).

### M4. Stale-write race and write after disable across VM generations
Ordinary disable retains the VM and its `Service` (`manager.rs:662`, "Ordinary disable retains the
environment"), so a pending write still lands after disable. That is acceptable as a flush. On reload
or device recovery (the old runtime is dropped, a new one started) each VM has its own `Service`
and its own detached worker, with nothing serialising writes per path. The old worker can finish
the older pending value after the new VM's newer save, so older data overwrites newer. Disable followed
by a "clear mod data" action can also have the old worker recreate the file.

Fix: a process-global per-path writer (or a generation counter checked before rename), together
with the join/flush from H1.

### M5. Item 7's test is partly vacuous and the new draw/key paths are untested
`round4_addon_input_and_render_continue_over_map_but_not_chat_or_search`
(`api_capture_tests.rs:1982`):
- The `render` event assertion (`native_events::render` with `once_mode`) does not depend on
  `map.open`; it passes with the feature removed. Only the `is_plain_play` asserts are RED-capable.
- Nothing tests that a key typed while a mod holds text capture is consumed and does not reach the
  map (the double-handling case P, M, O, G). The routing is correct by reading
  (`runtime.rs:231-258`, `input.rs:270` `wants_key_capture`: `is_text_capture_active()` consumes every
  key, and the release follows the press owner), but no test calls `wants_key_capture`/`capture.press`
  with `map.open` plus text capture, and none checks that an unclaimed key still reaches the map.
- The new `prepare_addons` branch (`report_viewer.rs:168-200`: take `ui.quads`, zero `ui.shift`,
  extra render pass with `g.depth` cleared, restore) has no pixel or unit test. Only a source-text
  ordering test exists. A mod HUD that silently fails to draw over the map would pass the suite.

Behaviour I verified by reading: the stock `p.hud()` path no longer sees mod commands on map-open
frames (the overlay drains them first, `report_viewer.rs:171`), `wants_ui()` minus `map.open` equals
`addon_input_available` (`play.rs:2276`), search/chat/dialog/typing blockers are retained, and the
wasm path is unchanged. One consequence worth a test: bundled Wereoxxs claims Enter, and now that
claim also applies with the map open, taking Enter from the map.

### M6. Save/load asymmetry blocks the journal's persistence
`load_json` refuses private data (`async_json.rs:101-115`, `key.starts_with("data")`), and the
synchronous private reader is capped at 1 MiB. A journal saved above 1 MiB (the brief's 14 MiB
case) cannot be read back after a restart. The report states this as a known Part B limit. Record it
as a blocking design decision before the journal mod is built (for example a private-mount
`load_json`, with its own cap), since `save_json_async` alone does not deliver a working journal.

### M7. Missing negative controls for the new limits
Tests do not cover: the 16 MiB read cap, the two-in-flight `quota` (`async_json.rs:187`), depth/node
quotas, non-object root, the `resources/` shared mount (only `package_resources/` is exercised), a
save that fails on the worker (quota exceeded) surfacing `resource.failed{kind=...}`, and drop/exit
drain (H1). In `round4_async_json_loads_large_readonly_resource_and_reports_failure` the failure
callback does not check `e.data.request==req`, and `assert!(scratch.0.exists())` proves nothing.
In `round4_async_json_save_is_private_and_latest_value_wins` the final `stage==3` check would also
pass under a FIFO queue; only the separate queue unit test proves coalescing.

## LOW

1. `json_save.rs:112`: `done` is a single slot, so an unpolled earlier failure is overwritten by a
   later success, and coalesced ids never complete (documented). A mod that waits on every id
   hangs; the spec should tell mods to track only the latest id.
2. `json_save.rs:280` takes the synchronous snapshot (about 4 ms for 14 MiB) before the different-path
   rejection (`:290`), so a rejected call pays the full cost.
3. The 14 MiB Lua baseline (`async_json.rs:237`) runs in a bare `Lua::new()` using `io.open`, with
   no 8 MiB heap, no instruction budget and no sandbox. A real mod cannot run that code (no `io`;
   `files.write` is 1 MiB). The decision (async needed) is right, but the "1794 ms" is not a
   real mod's cost. It also costs about 2 s of every suite run and has no time threshold.
4. `round4_shared_catalogue_retains_forge_pin`: the byte-flip control is tautological (any flip
   changes SHA-256). It only checks the shared copy against the native pin, not the native copy or
   a loader refusing a mismatch.
5. `mods/resources/map-entities.json` is not in `tools/build_player_release.py`'s mod allowlist
   (lines 110-111, 341-352), so players never get it. Part B must add it; the file also duplicates
   the native embedded copy (about 1.7 MB in the repo) until then.
6. `async_json.rs` pending counter never recovers if the worker dies (a panic); no recovery or
   diagnostic. `poll` only runs inside `dispatch`, so a paused mod accumulates completions.
7. Event queue eviction (`native_events.rs:716`, 1024 events / 2 MiB) can drop a `resource.ready`
   json event under a combat flood; the mod only learns through `events.dropped`, and the loaded
   table is retired.
8. Entity delta fields (item 10): `position` is `e.pos`, whereas the native journal records the
   dead-reckoned `actor_pos` when nonzero (`map/journal.rs:380`); coordinates can differ slightly.
   The test calls `begin_game_event`/`end_game_event` and mutates the entity directly; it does not
   decode a real packet, so "real packet boundary" is a stretch. The fields themselves are
   correct against the native filter (`kind==TYPE_CREATURE`, `p18&2`, `p255`, `p256` as class_code),
   the fingerprint adds `kind`, `p18` and `p255`, typed deltas fire only when `EntityData` changes
   (`api_updates.rs`), and the zero-addon allocation benchmark covers the gate.
9. `has_queued_commands` (`hud_layer.rs:695`), `Play::addon_input_available` (`play.rs:2276`) and
   `json_save::worked_off_thread` sit after `mod tests` (style lint only; the repo has no clippy gate).
10. `draw_map_addon_hud` returns early when `self.hud` is None or loading is up, but
    `has_queued_commands` stays true, so an empty render pass runs each frame until the commands are
    cleared. Harmless but wasteful.
11. A mod pointer claim (`claim_pointer`) is honoured while the M map is open, so a round-minimap
    mod that keeps claiming its rect would swallow map clicks there. The mod must release it.
12. The circle-mask test (`gpu_test.rs:205`) logs SKIP and returns Ok when no adapter exists (counts as
    a skip on GPU-less CI) and writes a PNG under the repo (`.superpowers/sdd/round4/`), which fails
    on a read-only checkout. The mask assertions themselves are real (corner masked, square
    control keeps the corner, capsule top excluded, background control).

## Checked and found sound

- load_json path control: `relative()` rejects `..`, absolute and drive paths; unmapped names become
  `data/...` and are refused; the file is opened through `read_bounded` -> `lock_resource`
  (`sandbox.rs:190`), which rejects links and reparse points and holds the parent handles on the
  worker, so there is no TOCTOU. 16 MiB cap is enforced by `take(max+1)`. Two-in-flight limit is a
  counter plus a bounded channel. The worker exits when the `Service` drops its sender (VM drop,
  disable-with-retire, device loss); it is not joined, which is harmless for reads.
- save_json_async: private area only (`/`, `\`, `:`, `resources/`, `package_resources/` refused;
  `key.starts_with("data")`); atomic temp+rename with a unique temp name (`storage.rs:585`); quota
  check on the worker; encoder output capped at 14 MiB; cycles, mixed keys, handles and NaN rejected;
  no writes to package paths.
- Entity delta fields come from protocol structs, not from HP; unchanged packets emit no event.
