# Round 2 performance-fix review (independent, read-only)

Reviewer: independent code review of `git diff 1db5c2d..HEAD` (branch `feat/eqoa-api-r2`).
No code was changed, no build/test was run (read-only mandate). Every finding below was
located in the source and is reproducible by reading the cited lines; nothing is inferred
from the worker's own report.

## Verdict

**APPROVE WITH CHANGES**

The delta architecture is sound and the measured regression mechanism (a full JSON-style
native view rebuilt and published per packet and per frame with no consumer) is genuinely
removed: with no consumer the per-frame and per-packet source hooks allocate nothing, the
`native_view` JSON build happens only on bootstrap, and the queue/journal/drain paths are
all bounded. The zero-addon path is genuinely allocation-free (verified by reading
`forge_snapshot`'s early return, `observe_text`'s guard, `defer`'s guard and
`health_hidden`/`health_redraw`'s scalar predicates).

The changes I require before merge are behavioural, not architectural: one event-rate
change that silently degrades the bundled Forge adapters, one gate that keeps the
expensive path alive with no live consumer, and three measurement/loss-reporting gaps that
make the report's headline numbers narrower than they read.

Counts: **0 HIGH, 5 MEDIUM, 6 LOW**.

---

## MEDIUM

### M1 - `player.changed` now fires per frame instead of per packet and evicts the events the bundled Forge adapters read

`crates/eqoa/src/fork_ext/api_updates.rs:342-407` (`refresh`) patches the player on every
frame in which the frame key changes (`:373-406`), and `patch_player`
(`crates/eqoa/src/fork_ext/api_updates.rs:112-145`) emits `player.changed` with a full
`before`/`after` player record (`:137-144`) whenever the record differs. The frame key
contains the walker position (`api_updates.rs:288-297`), so a moving player produces one
`player.changed` per rendered frame.

Previously (`1db5c2d`, `api_capture.rs` `end_game_event`) the diff events were emitted only
from `Core::publish(view, true)`, i.e. once per network event.

Failure scenario: the Core journal is a single shared bounded ring
(`crates/eqoa/src/fork_ext/addons/native_core.rs:139-171`, cap 2048 events / 4 MiB at
`:146`). A mod subscribed to `player.changed` (so `interested("player.changed")` is true)
floods that ring with ~1-2 KB before/after player payloads at frame rate. The bundled Forge
adapters read the *same* ring, not the per-VM queues:
`crates/eqoa/src/fork_ext/addons/ability.rs:748-774` filters `c.events` for
`e.name.starts_with("cooldown.")` and computes a `dropped` counter from the journal's first
sequence, and `crates/eqoa/src/fork_ext/addons/combat.rs:361-370` filters it for
`combat.hit`. Once the ring's byte cap evicts the older `cooldown.started` /
`cooldown.ended` / `combat.hit` entries, the Forge ability-list adapter reports dropped
lockout edges and stops delivering `OnLockoutStart`/`OnLockoutEnd`, and the Forge combat
adapter stops delivering damage events - for a mod that only asked for `player.changed`.
Net effect: one mod's subscription degrades another VM's bundled-UI behaviour.

Required change: rate-limit or coalesce `player.changed` (for example emit it only when a
non-positional field changed, or keep a separate coalescing slot for it), or give the Forge
adapters their own per-stream retention instead of sharing the 4 MiB ring.

### M2 - The demand gate is "addon intent", not "live consumer", so the expensive path stays on with nothing running

`crates/eqoa/src/fork_ext/addons/manager.rs:248` sets
`native_core::manager_demand(entries.iter().any(|e| e.enabled_intent && e.status != Status::Failed))`,
and `capture_requested()`
(`crates/eqoa/src/fork_ext/addons/native_core.rs:420-427`) ORs that with
`native_events::has_consumers()` (`crates/eqoa/src/fork_ext/addons/native_events.rs:722-731`).

An entry that is *enabled in intent* but has no running VM therefore keeps
`capture_requested()` true: the full Forge snapshot builder (`play.rs:217-845`), the
`native_projection::apply` projection (`crates/eqoa/src/fork_ext/addons/native_projection.rs:23-185`,
O(entities + abilities)) and the per-frame `patch_player` JSON rebuild all keep running.

Concrete case: `Manager::profile_waiting_world()`
(`crates/eqoa/src/fork_ext/addons/manager_profiles.rs:113-115`) blocks the enabling loop
(`manager.rs:253-258`) while a named profile waits for the world; during that window the
demand is true, no VM exists, and the client pays the full snapshot cost every frame -
exactly the class of cost this round set out to remove. The same applies to an addon whose
entry is enabled but whose instance was retired without `Status::Failed`.

Required change: derive the demand from live consumers (`has_consumers()`) plus an explicit
"about to instantiate" window, not from `enabled_intent` alone.

### M3 - The measured "enabled frame" case never exercises the moving path, so the report's zero-allocation figure does not describe the live game

`crates/eqoa/src/fork_ext/api_capture_tests.rs:522-623`
(`performance_busy_world_enabled_uses_incremental_typed_packet_updates`) measures
`p.forge_snapshot(None)`. With `walker == None` the frame key is constant
(`api_updates.rs:285-297` is skipped), so `refresh()` never enters the incremental branch
(`api_updates.rs:373-406`), `patch_player` never runs, no `player.changed` is emitted, and
the snapshot cache is hit every frame.

In the live client the snapshot is built with `Some(walker)`
(`crates/eqoa/src/viewer.rs:5833`), the key changes every frame while moving, `refresh()`
patches the player, `dirty()` bumps the revision, and the whole Forge snapshot is rebuilt
plus a player JSON before/after clone is emitted. The report's
"Enabled unchanged frames measured 4.398 us with zero allocations" therefore holds only for
a stationary, walker-less fixture.

Required change: add a fixture that drives `forge_snapshot(Some(walker))` with a moving
walker and a subscribed consumer, and report the allocation count for that case (or state
the limitation in the report).

### M4 - Overflow loss is reported only if a flush happens before the queue is discarded

`crates/eqoa/src/fork_ext/addons/native_core.rs:446-477` counts dropped pendings in
`d.lost` and bumps `CAPTURE_EPOCH` (`:462`), and `flush_deferred` (`:479-498`) turns that
into `events.dropped` + `state.reset` (`:493`). But `discard_deferred()` (`:499-506`) sets
`d.lost = 0`, and it is called from `updates::inactive()`
(`crates/eqoa/src/fork_ext/api_updates.rs:319-324`) on every packet while there is no
consumer, and from `invalidate_capture()` (`:517-520`) when the last consumer goes away.

Failure scenario: the queue overflows while a consumer exists, the consumer is disabled
before the next flush, and the loss is never reported to anybody - the report's claim
"Overflow reports lost source capture" holds only for the narrow window in which a flush
runs first. The epoch bump does force the next bootstrap, so the state self-heals; the
*notification* is what is lost.

Required change: keep the lost counter outside the discardable queue (or emit the marker
synchronously when the counter is first incremented).

### M5 - Deltas dropped on a generation change are silent

`crates/eqoa/src/fork_ext/addons/native_core.rs:494` skips every pending whose
`generation` differs from the current one, without incrementing `d.lost` and without
emitting `events.dropped`.

Failure scenario: the player zones (`Event::Teleport` -> `Core::reset("zone")`,
`crates/eqoa/src/fork_ext/api_capture.rs:268`) after a burst of entity packets; the queued
entity deltas, and any chat line captured just before the teleport
(`crates/eqoa/src/fork_ext/addons/native_core.rs:392-398`), are discarded silently. The
consumer does receive `state.reset`, so a well-behaved mod resyncs, but a mod that counts
`entity.changed` events sees an unexplained gap - unlike the overflow path, which at least
emits `events.dropped`.

Required change: count generation-scoped drops in the same `lost` counter (or emit a
dedicated marker) so the two loss paths are distinguishable.

---

## LOW

### L1 - `chat.history_scroll` is emitted immediately while chat messages are deferred, so source order can invert

`crates/eqoa/src/fork_ext/chat_history.rs:44-47` calls `native_core::emit` directly, while
`observe_text` (`crates/eqoa/src/fork_ext/addons/native_core.rs:381-399`) defers
`chat.message`. A scroll that happens between the capture and the flush is published
before the chat line it follows, contradicting the report's "Source order is preserved".

### L2 - The `publish(view, true)` diff branch is now dead production code

`crates/eqoa/src/fork_ext/addons/native_core.rs:281-365` (the `events && view.in_world`
diff block) has no production caller any more: the only `publish` calls are
`crates/eqoa/src/fork_ext/api_updates.rs:354` and
`crates/eqoa/src/fork_ext/addons/manager.rs:224`, both with `events = false`. The
`player.changed` / `group.changed` / `abilities.changed` / `hotbar.changed` /
`effects.changed` / `target.changed` diffs are now produced only by the `patch_*` functions
in `api_updates.rs`. Nothing in the tree exercises the old branch any more, so a future
divergence between the two implementations would go unnoticed. Either delete the branch or
keep a test that asserts the patch_* emitters produce the same envelopes.

### L3 - The zero-addon snapshot is written into retained, disabled instances' hosts

`crates/eqoa/src/fork_ext/play.rs:177-215` returns a minimal snapshot
(`capture_pending: true`, only `character`, `in_game`, `start_menu`, `ui_mode`, `ui_busy`,
`health`), and `crates/eqoa/src/fork_ext/addons/manager.rs:238-244` pushes that snapshot
into *every* instance that has one, including disabled ones whose `active` flag is false.
A retained-but-disabled VM therefore observes an empty `player`/`entities`/`abilities`
snapshot instead of the last real one. The impact is small (a disabled VM does not run),
but a cleanup or settings callback that reads `eqoa.player.get()` outside its own frame
would now see defaults.

### L4 - Enabling is delayed by one frame by the `capture_pending` gate

`crates/eqoa/src/fork_ext/addons/manager.rs:253-258` refuses to enable an entry when the
snapshot was built with `capture_pending == true`, and `manager_demand` is only updated at
the *end* of `manager.frame` (`:248`). The command that enables an addon is processed at
`:245-247`, i.e. after the frame's snapshot was already built with the old demand, so the
addon starts one frame later than before. Correct, but worth stating in the report because
it is observable to a mod that counts frames from the moment it is enabled.

### L5 - `AbilityLockout` events for an ability the client does not know are dropped entirely

`crates/eqoa/src/fork_ext/api_capture.rs:301-314` builds `cooldown_before` for
`Event::AbilityLockout` by looking the display index up in `play.abilities`; if there is no
match, `cooldown_before` is `None`, `Event::AbilityLockout` also falls into
`Kind::Other` (`crates/eqoa/src/fork_ext/api_updates.rs:602-644`), whose fingerprint is
constant (`:718`), so no capture happens and neither the lockout value nor a
`cooldown.*` event is produced. Same for `Event::TargetInfo` when the target is not in the
roster: the disposition change is not published (`api_updates.rs:608-614`).

### L6 - The pinned bundled-UI fixtures bypass the new cached snapshot path

`crates/eqoa/src/fork_ext/addons/wereoxxs_test.rs:526+` builds `Snapshot` values by hand
(`rich_in_game_snapshot`) and feeds them straight to `Manager::frame`, never through
`forge_snapshot`. A regression in the new cached/projection path (a stale player, ability
or entity projection) would therefore not be caught by the pinned Wereoxxs / ARPG / Modern
UI fixtures that the report cites as passing.

---

## Report claims I verified in the code (not taken on trust)

- "The allocation meter is test-only, no global allocator effect in release builds":
  **verified.** `crates/eqoa/src/fork_ext/addons/mod.rs:71-72` declares
  `#[cfg(test)] pub(crate) mod perf_alloc;`, and `crates/eqoa/src/fork_ext/addons/perf_alloc.rs:12`
  is the only `#[global_allocator]` in the workspace (grep across `crates/`), so it exists
  only in the crate's own test binary. Release builds are unaffected.
- "A bounded deferred queue (8,192 deltas / 32 MiB)": **verified**
  (`native_core.rs:446-477`; the byte estimate is a deliberate over-estimate for the
  captured records - a `PlayerData` is ~350 bytes against a 2048-byte charge, so the real
  footprint stays under the cap).
- "Generation changes discard old-world deltas": **verified** (`native_core.rs:494`).
- "Overflow ... forces a current-state bootstrap": **verified** - the overflow bumps
  `CAPTURE_EPOCH` (`native_core.rs:462`) and `refresh()` treats an epoch change as
  `first` (`api_updates.rs:344-346`), which re-publishes the full view. The *reporting*
  half of the claim is qualified by M4.
- "Disabled retained VMs do not count as consumers": **verified**
  (`native_events.rs:722-731` keys off `Bus::active`, which `clear()` clears at
  `native_events.rs:586`).
- "Bootstrap only on first consumer or world transition": **verified**
  (`api_updates.rs:342-367`; `first` is `!LIVE` or an epoch change, and the world
  transition is caught by `view.in_world != play.in_world_now()`).
- "HUD readiness uses scalar native predicates": **verified**
  (`crates/eqoa/src/fork_ext/play.rs:888-899` and `:927-933` now call
  `forge_health.snapshot(...)` directly instead of `forge_snapshot(None).health`).
- "The zero-addon snapshot keeps only the metadata the manager needs": **verified**
  (`play.rs:177-215`), and the metadata parity is covered by
  `api_capture_tests.rs:811-833` for nine menu modes. Note that `health` is not part of
  that comparison (it is set from the same `forge_health.snapshot` call, so it is equal).
- "Existing shared hook call sites are unchanged": **verified** - `play.rs:2329` and
  `play.rs:2865` still call `begin_game_event` / `end_game_event` in the same places.
- Zero-addon source hooks allocate nothing: **verified by reading** the guards -
  `forge_snapshot`'s early return (`play.rs:177`), `observe_text`'s guard
  (`native_core.rs:382-384`), `defer`'s guard (`native_core.rs:447-449`), the
  `ChangeCapture::drop` gate (`api_capture.rs:211-217`) and the scalar readiness
  predicates. No unconditional per-frame clone, scan or full-view build remains in the
  zero-addon path.
- Not verifiable here (no build/test run permitted): the timing and allocation numbers in
  the report's tables, and the CI/gate rows.

---

## What I did not find

- No lost or reordered source event inside the deferred queue itself: the queue is FIFO,
  the source `time`/`tick`/`frame` are captured at enqueue and restored per pending
  (`native_core.rs:492-496`).
- No unbounded growth: the deferred queue is capped at 8,192 entries / 32 MiB, the Core
  journal at 2,048 events / 4 MiB, the per-VM queue at 1,024 events / 2 MiB
  (`native_events.rs:691-709`), the Core chat ring at 1,000 lines, and the entity/ability
  maps are bounded by the roster size. Every one of these drains or drops, and the
  zero-addon path touches none of them.
- No bootstrap/delta race for a consumer that enables mid-session: the demand flips at the
  end of `manager.frame`, the next frame's `forge_snapshot` takes the full path, `refresh()`
  sees `LIVE == false` and bootstraps before the new VM's first frame, and
  `native_events::initial()` (`native_events.rs:538-555`) hands the new VM the
  `world.entered`/`state.reset` pair.
- No stale-handle hazard across a world change: the generation is incremented on every
  reset (`native_core.rs:180` and `api_capture.rs:234-247`) and `resolve()`
  (`crates/eqoa/src/fork_ext/addons/native_state.rs:31-48`) rejects a ref whose generation
  differs.
