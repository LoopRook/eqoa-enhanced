# Round 2 independent review

Branch: `feat/eqoa-api-r2` (base `develop` 413d1ee). Reviewed `git diff develop...HEAD`
(52 files, +7803/-476) against `docs/modding/EQOA-API-SPEC.md` §§1-5, 9-12 and
`docs/plans/EQOA-MOD-API-PLAN.md` "Round 2: core". Read-only review; no build, no
code change. The builder's `docs/plans/eqoa-api/round-2-report.md` was treated as a
claim list and each item below was re-verified in the code.

## Verdict

**APPROVE WITH CHANGES** — 3 HIGH, 6 MEDIUM, 8 LOW.

The shared-file work is the strongest part of this branch: `play.rs` (+759/-53) and
`viewer.rs` (+255/-15) match the approved budget exactly, every shared seam is a
neutral, unnamed-`Play`/plain-data call with no Lua, Forge, addon-ID or manifest
spelling in the signature, no upstream algorithm was moved or rewritten, and the
combat seam carries the already-parsed RGB without a fourth byte. The sandbox
resolver, tombstone/overlay semantics, event broker retention (1024 events / 2 MiB /
256 per boundary), diagnostic sink and the round-4 `not_yet` stub surface are all
implemented and largely correct. The HIGH findings below are Forge-adapter
behaviour regressions and one spec-contract gap in the new `eqoa` surface; none of
them require a redesign, but three of them change observable behaviour of the
bundled UI path and should be fixed before the owner's live test.

---

## HIGH

### H1 — `Entity.IsPopulated()` drops the `level > 0` test (Forge regression + pinned-reference break)

`crates/eqoa/src/fork_ext/addons/native_projection.rs:103`

```rust
s.is_populated = s.id != 0;
```

`native_projection::apply()` overwrites the whole `s.entities` array at the end of
`Play::forge_snapshot` (`crates/eqoa/src/fork_ext/play.rs:788-790`), so this value
is what the Forge `Entity.IsPopulated` / `entity.is_populated` getters return
(`crates/eqoa/src/fork_ext/addons/entity.rs:284`, `:530`, `:694`).

`develop` computed `is_populated: e.id != 0 && level > 0`
(`crates/eqoa/src/fork_ext/play.rs:447` on the base branch), and the pinned Forge
reference is explicit:

```
forge-compat-reference/scripts/modules/frontiers_forge/entity.lua:145
function Entity.IsPopulated(record)
    return Entity.GetId(record) ~= 0 and Entity.GetLevel(record) > 0
end
```

Failure scenario: `net::entity::Entity` has `creature: Option<Creature>`
(`crates/net/src/entity.rs:136`), so a slot can carry an entity ID before its
creature record arrives. `native_view` maps that to `"level": null`
(`crates/eqoa/src/fork_ext/api_capture.rs:130`), `put()` turns `null` into level `0`,
and `is_populated` becomes `true` because the ID is non-zero. A mod that filters
target/nameplate candidates with `IsPopulated()` now draws a frame for an entity
with no level yet; on `develop` and in the reference it was correctly reported as
not populated. This also breaks the Round-2 spec's §12 regression gate
("all normative ... conversions" / no changed Forge semantics).

Fix: restore `s.is_populated = s.id != 0 && s.level > 0;` in `put()`, and add a
projection-level test that feeds an entity with `creature: None` (the existing
`entity.rs` test at `:950` injects a synthetic `Snapshot` directly and therefore
never exercises `native_projection::apply`, so it cannot catch this).

### H2 — `Entity.GetTargetId()` / `entity.target_id` is narrowed to 16 bits with the wrong mask

`crates/eqoa/src/fork_ext/addons/native_projection.rs:99`

```rust
s.target_id = u(&e["target_id"]) & 0xffff;
```

The spec §11 for `entity` says the Forge-only correction is to the **low byte**:

> `GetTargetId` deliberately returns `native_target_id & 0xff`, matching the pinned
> low-byte read; eqoa Entity.target_id and Player.target_id preserve the full u32.

and the pinned reference confirms a single byte:

```
forge-compat-reference/scripts/modules/frontiers_forge/entity.lua:205
function Entity.GetTargetId(record)
    return Util.ReadFromOffset(record + 0x74, "uint8_t")
end
```

The implementation is wrong twice over: it uses `0xffff` (not `0xff`), and it is a
narrowing that `develop` did not do — the base branch stored `target_id = c.target`
(full `u32`) at `crates/eqoa/src/fork_ext/play.rs` (`record_of`).

Failure scenario: an entity whose server-side target ID exceeds `0xFFFF` (the
native IDs are full `u32`; the branch's own test fixture uses `0xfeed_beef`) now
reports a truncated ID through `Entity.GetTargetId()` and `entity.target_id`. A mod
that compares an entity's target ID against a full-width ID (or looks it up in a
full-width table) gets a wrong match. The bundled Wereoxxs/ARPG/Modern UI read
`Player.GetTargetEntityId()` (unmasked, `native_projection.rs:33`) and
`EntityList.GetEntityById` (which matches on `id`, not `target_id`), so the shipped
UIs are not hit today — but the adapter is the compatibility layer and is wrong.

Fix: use `& 0xff` to match the reference and the spec, and add a projection test
with a target ID above `0xFF` and one above `0xFFFF`.

### H3 — `eqoa.entities.named` is case-sensitive, exact-only, and has no `exact` parameter

`crates/eqoa/src/fork_ext/addons/native_state.rs:149-157`

```rust
read(lua, "eqoa.entities.named", "eqoa.entities.Entity", |v| {
    json!(v.entities.iter().filter(|e| e["name"].as_str() == Some(&name)) ...)
})
```

Spec §4 is explicit:

> `eqoa.entities.named(name:string,exact:bool=true)` | W: `Entity[]`, literal
> **case-insensitive** UTF-8 name match (substring when false); no regex/pattern
> execution.

Failure scenario: a mod calls `eqoa.entities.named("assistant recruiter")` for the
NPC the server named "Assistant Recruiter" and gets `{}` instead of the entity; a
mod that wants a substring match has no way to ask for one, because the function
takes one argument. Passing the documented second argument
(`eqoa.entities.named("wolf", false)`) raises an mlua arity error inside the
wrapper, which `native.rs:495` converts into `nil, Error{code="invalid_argument"}`
— a misleading message for a call that is valid per the spec.

There is **no test at all** for `eqoa.entities.named` anywhere in the tree
(`grep entities.named` matches only the implementation), so nothing catches this.

Fix: accept the optional `exact` flag, compare case-insensitively, and substring
match when `exact == false`; add a test with mixed case and a substring query.

---

## MEDIUM

### M1 — A queued Forge chat line can be silently dropped while the send already reported success

`crates/eqoa/src/fork_ext/play.rs:25-28`

```rust
for msg in crate::fork_ext::addons::chat::take_outgoing_chat() {
    if ready && _play.mode != 60 && !crate::fork_ext::addons::chat::report_route(&msg) {
        _play.chat_line(&msg);
    }
}
```

`take_outgoing_chat()` drains the queue unconditionally; the guard then decides
whether to send. The `mode != 60` test (native mode 60 = the combat-record window,
spec §7) is new in this branch — the base branch called `play.chat_line(&msg)`
unconditionally from `health_redraw`.

Failure scenario: a mod calls `frontiers_forge.chat.SendChatText("hello")` while
the combat-record window is open. `chat.rs:441-490` accepts it and returns
`(true, nil)` — the mod is told the line was queued. On the next game step the
line is drained and discarded because `mode == 60`. No diagnostic is emitted, no
error is returned, and the message never reaches the server. This is exactly the
"fake success / swallowed error" class: the API reported success and then threw
the work away. The same silent-drop path exists for `ready == false` (a world
loss between queue and drain).

Note that the discard-on-world-gap is *tested* and therefore intentional
(`crates/eqoa/src/fork_ext/api_capture_tests.rs:130-133`), but the `mode == 60`
case is not tested at all, and no diagnostic accompanies either.

Fix: keep the queue entry when the guard fails and emit a visible diagnostic
(`busy` / `cancelled`), or return `busy` from `SendChatText` when a modal window
owns the input. At minimum, drain only the entries that were actually sent and
report the rest.

### M2 — `entryMode` is neither validated nor recorded in discovery

`crates/eqoa/src/fork_ext/addons/host.rs:344-350`

```rust
pub(crate) fn once_mode(&self) -> bool {
    self.addon.as_ref()
        .and_then(|a| std::fs::read(a.package_root.join("mod.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .is_some_and(|v| v["entryMode"] == "once")
}
```

Spec §2:

> Optional manifest `entryMode` is `"frame"` (default, preserving every existing
> manifest) or `"once"` (new event-driven authors). **Round 2 validates this field
> and records it in discovery.**

`crates/eqoa/src/fork_ext/addons/discovery.rs` has no `entryMode` handling at all
(grep for `entryMode` matches only `host.rs` and the bundled example's `mod.json`).
The field is therefore read a second time, at VM-install time, straight off disk,
and any invalid value (`"ONCE"`, `123`, `null`) silently degrades to frame mode
with no diagnostic.

Failure scenario: a mod author writes `"entryMode": "Once"`; the mod silently runs
in frame mode, its `eqoa.events.on("render", …)` registration is rejected with
`invalid_argument` ("frame-mode entry is already the render callback"), and the
author has no way to find out why from the manifest. Worse, for a loose package
`package_root == script_root`, so an unrelated `mod.json` at the mods root would
be read.

Fix: parse and validate `entryMode` in `discovery.rs` (reject values other than
`"frame"`/`"once"` with a manifest diagnostic), store it on `DiscoveredAddon`, and
have `Host::once_mode()` read the recorded value.

### M3 — Sandbox traversal is reported as `io_error` instead of `permission_denied`

`crates/eqoa/src/fork_ext/addons/file_io.rs:573-588` (native `eqoa.files` mapping)

```rust
if e.contains("not found") { "not_found" }
else if e.contains("invalid") || e.contains("outside") { "permission_denied" }
else if e.contains("quota") || e.contains("exceeds") { "quota" }
else { "io_error" }
```

`relative()` rejects `..` with the message `"parent traversal in addon resource
path"` (`file_io.rs:210-212`), which contains neither `"invalid"` nor
`"outside"`, so it falls through to `io_error`.

Failure scenario: a mod attempts `eqoa.files.read("../other_mod/secret.cfg")`. It
is correctly refused (the sandbox holds), but the returned `Error.code` is
`io_error`, which tells the author "the disk failed" rather than "that path is
not permitted". Spec §9 classifies these as rejected boundary inputs, and §1's
taxonomy has `permission_denied` for exactly this. The same misclassification hits
`"invalid addon resource path"` (a reserved device name or an over-long path) and
`"file path outside issued addon mounts"` is caught only because it contains
"outside".

Fix: map the resolver's rejection reasons to `permission_denied` explicitly
rather than by substring.

### M4 — Native API failures are all reported as `invalid_argument`

`crates/eqoa/src/fork_ext/addons/native.rs:495`

```rust
Err(e) => error(lua, &symbol, "invalid_argument", &e.to_string()),
```

Every `bind()`-wrapped function funnels *any* Lua/Rust error through this arm.
When the wrapped function is the native implementation (not a stub), an internal
failure is reported to the mod as a caller mistake.

Failure scenario: `eqoa.textures.load("resources/x.png")` reaches
`UiForge.LoadTexture`, which fails inside the texture bank with a GPU/upload
error; the mod receives `nil, Error{code="invalid_argument", message="<internal
GPU error>"}` and will typically retry or report "bad argument" instead of
surfacing the real cause. The same applies to `eqoa.files.write` (an atomic
replace failure that is not one of the recognized strings becomes `io_error`
inside the closure, but any *other* Rust error raised inside the closure reaches
this arm as `invalid_argument`).

Fix: use the per-function error mapping already present in `file_io.rs` /
`native_settings.rs` for the native implementations, and reserve `invalid_argument`
for genuine argument checks.

### M5 — `eqoa.hud.hidden_by` does not see Forge-owned claims

`crates/eqoa/src/fork_ext/addons/visibility.rs` (`install_native`, `hidden_by`
closure) resolves owners through `registry.identities`, which is populated only
inside `install_native` (`visibility.rs`: `identities.insert(lease.token,
host.addon_id())`). Forge `UiForge.DisableUI` claims use the anonymous/legacy
owner path and never enter `identities`.

Failure scenario: a Forge mod calls `UiForge.DisableUI("health")` and a native
mod calls `eqoa.hud.hidden_by("health")`; the native call returns an empty list
even though the health bar is hidden by the Forge owner. Spec §5 says
`hidden_by` returns "the sorted deduplicated union of addon IDs claiming any
expanded leaf"; §11 keeps only the *legacy owner-name* form scoped per VM, so the
union should still include the Forge owner.

Fix: register the Forge claim owner in `identities` as well (the registry already
keys by the VM lease token), or document the native/Forge split explicitly.

### M6 — The full native view is rebuilt on every game frame just to compute a readiness flag

`crates/eqoa/src/fork_ext/play.rs:18-21`

```rust
if _game_step {
    let ready = _play.native_view(None).in_world;
```

`native_view()` assembles the whole `View` — every entity, ability, group member,
hotbar slot and effect, all as `serde_json::Value` (`crates/eqoa/src/fork_ext/api_capture.rs:79-184`)
— once per 30 Hz step, purely to read one boolean. The result is then discarded;
the real view is rebuilt again at the next frame boundary in `forge_snapshot`.

This is the cost the round-2 report lists under "Deferred scope" ("Post-event
comparison still assembles a complete R2 view … throughput at high entity/packet
rates is not live-profiled"), but the report does not say that the *iteration
seam* itself pays this cost on every game step.

Failure scenario: on a busy server with ~40 entities and a full spellbook, the
client performs an extra full JSON projection per 33 ms on the game thread,
purely to decide `ready`. The same projection then runs again in
`forge_snapshot` and once more per pumped event in `end_game_event`
(`api_capture.rs:253-254`), which is the real risk to frame pacing.

Fix: expose a cheap `Play::in_world_now()` predicate for the iteration seam and
use it instead of `native_view(None).in_world`.

---

## LOW

### L1 — `eqoa.entities.distance` uses `ee::*` while the Forge equivalent uses plain `f64`

`crates/eqoa/src/fork_ext/addons/native_state.rs:379-389` computes the distance
with `ee::sub/ee::mul/ee::add/ee::sqrt`, while the Forge `Entity.GetDistanceTo`
(`crates/eqoa/src/fork_ext/addons/entity.rs:228-236`) uses ordinary `f64`
arithmetic. Spec §4 calls `eqoa.entities.distance` "`A/entity.rs:226`
equivalent". The EE operators chop toward zero (PS2 semantics,
`crates/ee/src/lib.rs:86-103`), so the two can differ in the last ulp for the same
inputs. Low impact, but the two APIs should agree on a "pure calculation".

### L2 — `eqoa.player.experience_for_level` uses the next-level threshold

`crates/eqoa/src/fork_ext/addons/native_state.rs:306-309` returns
`hud::exp_for_level(level + 1)`, which is the game's own "exp needed to reach the
next level" convention (`crates/eqoa/src/hud.rs:321` uses `exp_for_level(s.level +
1)`). Spec §5 says "uint threshold … outside 1..59", which reads as the threshold
*for* the given level. The off-by-one is ambiguous in the spec but should be
stated in `GETTING-STARTED.md` either way.

### L3 — `eqoa.capabilities.list` matches the prefix textually, not as a dotted path

`crates/eqoa/src/fork_ext/addons/native.rs:613` filters with
`p.starts_with(&prefix)`, so `list("eqoa.player")` also matches a hypothetical
`eqoa.playerXYZ` path. Spec §2 says "Paths are exact dotted symbols". Harmless
today (no such path exists) but the filter should be path-segment aware.

### L4 — `eqoa.icons.info` / `SurfaceInfo` unknown fields are diagnosed as `unsupported`

`crates/eqoa/src/fork_ext/addons/icons.rs` — the `SurfaceInfo` metatable
`__index` diagnoses any missing field with code `unsupported`. Spec §11 says
"palette/format only if genuine decoder metadata exists, otherwise nil +
per-field unsupported diagnostics", so this is correct as written; noted only
because `unsupported` for a *known* field is a stronger claim than the spec's
"optional missing resource probes use one informational manager entry per path".

### L5 — `eqoa.client.get().generation` starts at 1, not 0

`crates/eqoa/src/fork_ext/addons/native_core.rs:196-198` and `:253-255` set
`generation = 1` on first world entry. The spec (§1) says the generation changes
on entry/respawn and does not fix the origin value, but `generation = 0` is the
natural "never entered" value and the code special-cases it in two places
(`if entered && self.generation == 0`). Worth pinning in the round-5 audit.

### L6 — The report's ImGui call-budget deviation is acceptable but should be a spec amendment

The report discloses that `CALL_BUDGET` remains 131,072
(`crates/eqoa/src/fork_ext/addons/bridge.rs:112`) against spec §9's
"4,096 bridge calls/invocation". The branch's own test
(`crates/eqoa/src/fork_ext/addons/wereoxxs_test.rs:2147-2193`) demonstrates that
the spec's 4096 would **fail** the bundled Wereoxxs UI. This deviation is
**acceptable**: §12 requires the bundled UIs to keep working, and the spec text is
the thing that is wrong here. It should be written back into
`EQOA-API-SPEC.md` §9 (or a lead-approved amendment) rather than carried as a
report-only deviation, so round 5 does not "fix" it back to 4096.

### L7 — The report's descriptor-inventory deviation is acceptable

The report states that "the descriptor inventory is implementation-driven for many
Forge members and ImGui methods". That matches the code: `native.rs:270-286`
enumerates the registered Forge modules at runtime and marks their members
`supported`, and `native_media.rs:17-24` marks every bound ImGui member
`supported`. This cannot detect a *missing* member, so the round-5 per-symbol
audit (§12) is still required — which the report says. Acceptable as a declared
scope limit, provided round 5 does not inherit it as a permanent gap.

### L8 — Non-Windows rooted file operations report `unsupported`, which is correctly disclosed

`crates/eqoa/src/fork_ext/addons/sandbox.rs:178-185` returns
`"unsupported: secure rooted file operations are not implemented on this platform"`
for every non-Windows parent lock, and `native.rs:451-465` maps the affected
modules to the `unsupported` capability on non-Windows. Spec §9 requires rooted
operations to "reject if platform cannot enforce", and the report discloses that
no native Linux run was performed. This is **acceptable** for round 2, with the
caveat that the fork's normal CI target is Linux — the whole `eqoa.files` /
`eqoa.textures.load` / `eqoa.sound.load` surface is therefore `unsupported` on the
project's primary build platform, and the wasm32 check does not exercise it.

---

## Report claims checked

Verified as accurate:

- Footprint: `play.rs +759/-53`, `viewer.rs +255/-15` against
  `8435f2c8…` — matches `git diff --numstat` exactly, and the `docs/fork-footprint.json`
  budget raise 755→759 matches.
- The shared seams are neutral: `fork_hooks::begin_iteration/begin_game_event/end_game_event/observe_text`
  and `fork_ext::prepare_controls` name no Lua, Forge, addon ID, callback or manifest
  in their signatures or payloads (`crates/eqoa/src/play.rs:2320,2329,2865,4783`;
  `crates/eqoa/src/viewer.rs:2754,3163`).
- The combat seam passes the already-parsed RGB with the `damage_rgb && len >= 3`
  gate intact and reads exactly three bytes (`crates/eqoa/src/play.rs:5834,5847`).
- No upstream algorithm was moved into Lua and no upstream file was deleted or
  rewritten — the whole shared diff is 6 added lines in `play.rs` and 2 in
  `viewer.rs`.
- Event broker retention is 1024 events / 2 MiB per VM with a 256-event dispatch
  budget, and the byte cap and the backlog are separately tested
  (`native_events.rs:685-703`, `native_tests.rs:202-263`).
- The diagnostics sink deduplicates by (symbol, code) per VM, rate-limits new keys
  at 16/s with a burst of 32, keeps a 256-key cache, rotates the log at 5 MiB and
  makes a log-write failure visible in the manager (`native.rs:169-210`,
  `manager.rs:378-461`).
- The sandbox holds against traversal, reserved device names, absolute/drive/UNC
  paths, cross-package access and sibling-addon data, with real negative controls
  (`file_io.rs:662-705,761-809`, `wereoxxs_test.rs:165-210`). Tombstones, private
  overlays, shared-bundle read-only fallback and per-addon isolation are exercised
  with the real bundled Modern UI migration (`wereoxxs_test.rs:192-203`).
- The three bundled UIs are exercised with their real embedded Lua bytes
  (`wereoxxs_test.rs:212-217`), and the 30 Hz tick / presentation-frame split is
  tested (`api_capture_tests.rs:234-256`).

Verified as inaccurate or incomplete:

- "Forge cooldown Update consumes native journal edges in production, with a
  compatibility fixture fallback" — true, but the fallback (`ability.rs:786-801`)
  is selected by `view.in_world`, not by "production", and the fixture path is the
  one the *unit* tests exercise. Both paths are covered; the wording is loose.
- "Native sound reports backend rejection" — true (`native_media.rs` / `sound.rs`),
  but the `unsupported` platform path is decided at `bind` time
  (`native.rs:451-465`), i.e. per platform, not per backend.
- The report does not mention the `Entity.IsPopulated` / `GetTargetId` changes
  (H1, H2), the `eqoa.entities.named` contract (H3), the `mode == 60` chat drop
  (M1), the unvalidated `entryMode` (M2), or that `eqoa.files` / `eqoa.textures.load`
  / `eqoa.sound.load` are `unsupported` on the project's primary Linux CI target
  (L8). These are the substantive gaps between the report and the code.
- The report's "Deliberate negative controls" row lists eight mutations; none of
  them targets `native_projection::apply` (which is why H1 and H2 survived), the
  `entities.named` matcher, or the `mode == 60` chat guard.

## Notes for the lead

- The bundled-UIs live test still owns the decision on H1/H2 (no bundled script
  reads `entity.target_id` or `IsPopulated`, so the owner will not see them, but
  third-party Forge mods will).
- The round-2 gate in the plan ("the existing addon tests are the regression
  gate") passed, but the addon tests never exercise `native_projection::apply`
  with a partially-populated entity, so the gate cannot detect H1/H2. A
  projection-level fixture would close that hole for round 3 as well.
- Round 3/4 work should not build on `native_projection::apply` until H1/H2 are
  fixed: `items`, `bank`, `loot` and `merchant` Forge adapters are specified to
  keep the same "no fabricated values" discipline, and the projection is the
  shared place where that discipline is currently broken.
