# Performance: mods read creature changes on demand (2026-11-14)

Branch `perf/entity-deltas`, from `feat/eqoa-api-r4` at 56f3f05d (969739e4 plus the Terra check). Five commits, one per step, not pushed, merged or tagged. No bundled mod file, no Forge UI file and no shared (upstream) file was touched: ten fork-owned files under `crates/eqoa/src/fork_ext/`.

## Result

**Not at the 2 ms target.** All seven mods over no mods, same scene and harness (release, alone, one test thread, 300 frames, 25 entity updates a frame, interleaved A/B, the PC idle as the owner said):

| | Before (56f3f05d) | After (head) |
|---|---:|---:|
| All seven over no mods, run 1 / run 2 | +5.79 / +4.69 ms | +3.15 / +3.30 ms |
| (earlier A/B pairs, same builds each time) | +4.84, +4.60, +4.87, +4.60 | +3.31, +3.31, +3.23, +3.31 |
| Creature journal alone | +3.49 / +2.60 | +1.13 / +1.15 |
| Map search alone (the journal's data loaded) | +3.60 / +2.97 | +1.46 / +1.80 |

So about **-1.4 ms (-30 %)** on the same builds run back to back; the floor is about +3.2 ms. The before figures move between 4.6 and 5.8 ms across runs (the machine is not perfectly still even when idle); only the paired comparison counts, and the after figures are steady at 3.2-3.3.

What was asked (typed deltas read on demand, no change to what a mod can observe) was done where it can be done without changing what a mod sees; the rest of the gap is in places the brief says to leave alone or that need a bigger store change (see "Why not 2 ms").

## Steps

Each step: the change, the evidence it changes nothing a mod can see, the timing. Timings are from `round4_seven_mods_in_a_real_shaped_scene`, release, interleaved with the before binary, "all seven over no mods".

### 1. Player stat view memoised (commit d45c28f7)

`player_stats::view_json` keeps the last view and the flattened numbers it was made from (the stat channel's totals and bases, the masteries, the stat cap and its bonuses, every worn item's stats; a carried item counts for nothing, as in `view`). The same numbers give the same view (`view` is pure), so the stat view is built again only when one of them moves. This is the exact inputs, not a packet counter: the brief suggested a revision bumped by a stat-changing packet, but the inventory is also changed by local paths that no packet capture sees (fishing, looting), so a packet revision could serve a stale view; comparing the inputs cannot. `PlayerData` holds the view as an `Rc` and the record stores it only when it differs from what the view already holds (it used to deep-copy about 100 values every frame). Upstream's `net` types are untouched.

- Test `the_stat_view_memo_gives_what_a_direct_build_gives`: through a total, a base, a mastery, a bonus, the cap, a worn item's stat, wearing and taking off an item, a carried item (a hit, nothing built), and no stat channel yet (no view), the memo gives what a direct build gives; hit/miss counted. Negative control inside it: a key that leaves the worn items out serves a stale view after an item's stat changes, and the test asserts that staleness, so it fails against that key (it first failed on my own control, which used a stat under the trainable cap: fixed to use armour).
- Timing: `player.stats_view` 0.163 to 0.001 ms a frame; `api.refresh.player` 0.43 to 0.14 ms (profile, before/after, same scene). Whole-frame: A/B with step 2 below (4.89 / 4.63 against 5.87 / 5.17 for the before binary in the same minutes).

### 2. Cheaper native patch and emit (commit 8ac4f6fe)

- `Core::emit` counted every event's size by writing the whole payload (`serde_json::to_vec`, two full records for an entity event). `json_len` computes the same number without writing (numbers through serde_json's own `Display`, which is what the serializer calls; string escapes counted). The queue limits (2048 events / 4 MiB, 1024 / 2 MiB per VM) see the same size.
- `changed_fields` no longer builds and sorts a list of every key of both records; it lists the keys whose value differs (a missing key reads as null), sorted.
- The entity event's data is built by moving the two records in (`json!` copied each a second time); `patch.event json` 0.23 to 0.025 ms.
- `patch.group` copied the whole group for every creature update when a mod listened to `group.changed`; it now copies only when the creature is a group member (otherwise the group cannot change and nothing was emitted).
- Tests: `json_len_is_the_length_serde_json_writes` (fixed cases and 4000 generated values, escapes, unicode, extreme numbers, `-0.0`, against `to_vec`; control: a count that ignores escapes is shown to differ); `changed_fields_names_what_the_old_walk_named` (the old walk copied into the test, fixed cases and 3000 generated pairs).
- Timing: steps 1+2 together A/B 4.89 / 4.63 (before 5.87 / 5.17, noisy run): about -0.3 to -0.5 ms.

### 3. Change-event data built when read; one protected call for the event; no work for a VM that has no handler (commit 07fb4ab0)

- A change event's data (`ref`, `changed`, `before`, `after`) is a read-only record whose fields are built from the native event on first read (`lazy_data`). Before, `ref` (a userdata) and `changed` were built for every delivery, then `before`/`after` lazily. The journal reads `after.id` and returns.
- The event table and its read-only guard are made in one raw protected call (`record_raw::event_proxy`), and the lazy data's tables in another (`lazy_object`); the old path took about twenty protected calls per delivery (`deliver.envelope` 0.60 to 0.25 ms a frame; `env.data` 0.33 to 0.15, `env.readonly` 0.20 to 0.09). Names whose data the guard also wraps (`timer`, `resource.ready`, `resource.failed`) and events delivered with their data already made keep the old path.
- `deliver_data` now returns right after counting an event against the frame's delivery budget when no handler of this VM is subscribed to it (it built a player scope and the rest for nothing, 150 times a frame with seven mods). The budget is still spent, so `events.dropped` and the 256-a-frame limit behave exactly as before: test `an_event_nobody_listens_for_still_spends_the_delivery_budget`; control run by hand: skipping the budget count makes it fail (`left: 1`).
- Brief item "batch events per frame into one call per addon where the API already allows it": it does not (each event is its own callback with its own envelope), so no batching; and the events still reach every VM's queue (filtering them out would change the budget and the overload accounting a mod can observe: stopped, see below).
- Test `delivered_events_read_as_the_eager_build_reads_for_every_shape` (extended in the fix round below): ten sampled event shapes (`entity.added/changed/removed` with all 21 entity fields, `group.changed` with nested members and references, `map.changed`, `player.changed`, `target.changed` with references, `hotbar.changed` with arrays, `state.reset`, `cooldown.started`) are delivered through the real dispatch and read at every path of the event, against the old eager build (`record` whole, then the guard): the same values; references equal (`==`) and resolving to the same thing through `eqoa.entities.get`; the same `pairs`/`ipairs` result; the same table on a second read; the same write error at every table. The ten shapes are samples, not every event kind: the other kinds go through the same lazy code, and only these are asserted. The existing `change_event_before_and_after_are_built_only_when_read` still passes; it counts raw builds, and since `ref`, `changed` and nested records are now also built on first read, its "no further builds" block now reads those once first (same intent, one line changed).
- Timing (interleaved A/B): 3.49 / 3.31 against 4.84 / 4.60.

### 4. `before` and `after` read a field at a time (commit 5aa3d854)

The two records are lazy objects too (`lazy_fields`): reading `after.id` builds `id`; reading `after.position` builds the position record, cached so it is the same table each time. Same equivalence test (it reads every path of both records). `Core::emit` counts a stream without copying its name each time. Timing: 3.31 / 3.23 against 4.87 / 4.60; the journal's own Lua 0.58 to 0.47 ms.

### 5. Sections for the manager's per-frame work (commit b89a1312)

`mgr.prologue`, `mgr.drains`, `dispatch.polls` (under `EQOA_PERF` only) show that the manager's per-frame work apart from delivering events is small: 0.03 + 0.03 + 0.006 ms for seven addons. No change was needed there ("skip dispatch work for addons with no handler" is the budget-preserving early return in step 3).

## The other benchmarks (before 56f3f05d / after head, release, alone)

| | Before | After |
|---|---:|---:|
| Guide cost (own time + native, guide minus no guide; asserted under 0.5 ms) | 0.184 ms | 0.199 ms (noise: the harness's second decimal) |
| Wereoxxs UI over no mods | +2.20 | +2.11 |
| ARPG UI over no mods | +1.97 | +1.92 |
| Modern UI over no mods | +2.28 | +2.02 |

## Why not 2 ms

Profile of the head, all seven (per frame; sections nest): entity patch for 25 updates 0.57 ms (the new record 0.10, the copy for the event 0.12, changed list 0.07, emit 0.21), the player's record 0.14, delivering to the journal 25 times about 0.45 ms (envelope 0.25, handler about 0.2), the other six mods' own render work (draw callbacks, `hud.text`, map reads) about 0.85 ms, the shared capture and snapshot about 0.3 ms. What each remaining piece needs:

- The entity store is `Vec<serde_json::Value>` read by the whole API, the guides and over a hundred tests that assign `view.entities` directly. A typed store (a struct per creature, JSON made only when a call reads it) would remove the 0.10 + 0.12 + 0.07 + part of 0.21 (about 0.35 ms), but it is the larger change the brief's design points at and it changes a public test seam; this round kept the store and made everything around it lazy or exact. That is the next step if the 2 ms target stands.
- Event retention: `Core::events` keeps the last 2048 events and nothing but tests read it; dropping an evicted event frees its two records (about 4 us of the 8 us emit). Only a move to typed payloads avoids that cost; deleting the queue would only move the free.
- The mods' own callbacks (0.85 ms) are immediate-mode drawing (`hud.text` 32 times a frame in the stats box, 3 in the numbers mod); no mod rebuilds unchanged data per frame (stats box caches its rows behind its `dirty` flag), so no mod file was changed, as the brief said.
- Stopped, not done: a VM that has no handler for an event kind still has the event queued. Not queueing it would change what a mod can observe in overload (the 256-a-frame budget the unheard events spend, and the `events.dropped` counts), so only the delivery work was skipped.
- Stopped, not done: caching `interested(name)` per subscription epoch (about 1 us a creature update): ten mutation sites, a stale answer would drop events silently, 0.03 ms.

## Fix round (2026-11-14, tests only, no behaviour change)

Terra's check (`round-4-check-perf-deltas.md`): PASS WITH CHANGES. Two MEDIUM and one LOW finding, all on test strength and the report's wording.

- **Event comparison extended** (`perf_delta_event_tests.rs`). Each of the ten shapes is now delivered to two handlers of one VM and one handler of a second VM, and is not read until the next frame: in between the world's creature changed (new name and position) and a newer event of every kind arrived (every number and name moved). Each handler's first event must still read as the eager build of its own event (emit-time values), and the newest event as the eager build of the newest. Compared at over 600 places: references by `==` (kind, key, generation, incarnation) and by what `eqoa.entities.get` resolves them to now (one event holds a reference to a creature that is gone: both say `stale_handle`); `pairs` and `ipairs` results in order at every table; the write error at every table, nested ones included; identity on a second read; the two handlers get distinct event tables. Controls inside the test: against another event's data, with every flag also turned over, every shape reports a difference; against the same event with the reference one incarnation on, it reports exactly `ref ref`.
- **Negative controls run for real** (local edits, restored with `git checkout`, never committed; `grep MUTATION` finds nothing in the tree):

| Reverted line | Test | Failure |
|---|---|---|
| stat key without the masteries (`player_stats.rs`) | `the_stat_view_memo_gives_what_a_direct_build_gives` | `memo and direct build differ` |
| `json_len`: control characters counted 5, not 6 | `json_len_is_the_length_serde_json_writes` | `left: 36 right: 38` on the escapes string |
| `changed_fields` without the keys only in `before` | `changed_fields_names_what_the_old_walk_named` | `left: [actor_position, hp_fraction, position] right: [actor_position, extra, hp_fraction, position]` |
| delivery budget not counted for an unheard event | `an_event_nobody_listens_for_still_spends_the_delivery_budget` | `the budget went on the 256 before it: left: 1 right: 0` |
| lazy fields read the newest event, not their own (live state) | `delivered_events_read_as_the_eager_build_reads_for_every_shape` | `vm 1 first entity.changed: kept across a frame: left: Some("type after")` |
| lazy `ref` built for the next incarnation | same | `left: Some("ref after/ref")` |

- **Report wording** (the LOW): the claim in step 3 now says what is asserted (above in step 3: references by equality and resolution, iteration, write errors, ten sampled shapes) and that the shapes are samples.
- **Gates.** Addon suite (`fork_ext::addons::`, one thread, scratch `EQOA_CONFIG`, `EQOA_ISO` set, `R4C_SAMPLES=5`, `--nocapture`), run once at the end: 425 passed, 0 failed, 23 ignored, 9 lines with `SKIP` (symlink privilege, pinned Forge grid and hearts, picture goldens). The timing tests are skipped with `--skip round4_ --skip frame_time --skip benchmark --skip scrollback_10000 --skip measure` (they run by default in the suite, so the earlier 503 count included them); the guide's 0.5 ms assertion is one of them. Footprint (456 fork-owned, 31 shared) PASS and `check_no_local_paths.py` clean.

## Gates

| Gate | Result |
|---|---|
| Addons suite, single thread, scratch `EQOA_CONFIG`, `EQOA_ISO` set | 503 passed, 0 failed, 25 ignored (run before the last commit; the last commit adds one test and diagnostic sections) |
| Full client suite, ONE run, single thread, `R4C_SAMPLES=5`, `--nocapture`, `EQOA_ISO` and `run/eqoa.iso` | 2116 passed, 0 failed, 32 ignored (1414 s). 49 lines contain `SKIP` in the captured output: 4 symlink-privilege tests, the pinned Forge grid, pinned hearts, 3 UI picture comparisons (no reviewed goldens), no live S2 or save fixtures, no `run/client.bin`, no 2002 disc, the decomp not beside the repository. The same kinds as the Terra check's list; none is in a test this round touched. |
| New tests this round | 5 (stat memo, `json_len`, `changed_fields`, event equivalence, delivery budget; 2111 to 2116) plus one extended existing test; negative controls as listed |
| Footprint (`--base 11aa5f08`) | PASS (454 fork-owned, 31 shared, no shared file touched) |
| `check_no_local_paths.py` | clean |
| wasm check, release build, `test_player_release.py` | wasm check (`cargo check -p eqoa --bin eqoa --target wasm32-unknown-unknown`) PASS; release build PASS (1m36s, run last, after the final commit); `test_player_release.py` 20 passed |
| Oracle tests (the decomp and eqoa-port oracle are not on this machine) | skip here, as in every round |

The Forge UIs (Wereoxxs, ARPG, Modern UI) have no changed file and their suites pass unchanged (inside the counts above).
