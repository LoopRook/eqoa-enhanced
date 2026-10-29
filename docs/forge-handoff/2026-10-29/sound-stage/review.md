# Independent review — native addon sound stage (5757646..3ffe587)

Reviewer: MiniMax-M3 (independent provider; named Gemini reviewer slot).
Date: 2026-10-29 (rev. 2, after the implementer's evidence follow-up).
Read-only review: no repository edits, no builds, no test runs, no subagents, no pushes.
Only this artifact was written.

Inputs read: `run/forge-sound-stage/task-1-brief.md`, `run/forge-sound-stage/review-package.txt`
(full diff 5757646..3ffe587), `run/forge-sound-stage/lead-review-notes.md`,
`run/forge-sound-stage/implementation-tests.log` (the preserved run, added in rev. 2),
`docs/FORGE-SOUND-REPORT.md` (worktree), the changed sources in the Forge worktree
(`crates/eqoa/src/fork_ext/addons/{sound,manager,runtime,mod}.rs`), the pinned reference
(`run/forge-compat-reference/UiForge/src/core/audio_manager.{h,cpp}` and `core.cpp` 596-624,
770-835), and the neighbouring policy owners (`addons/package.rs`, `addons/textures.rs`,
`addons/discovery.rs`, `addons/host.rs`), plus `.github/workflows/dev.yml`.

## Verdicts

| Axis | Verdict |
|---|---|
| **Spec compliance** | **FAIL (conditional)** — the API surface, arity, nil-safety, quotas, ID rules, volume rule and the lifecycle matrix match the brief and the pinned C++ on every path I traced, and the preserved log now proves the native MCI path actually executed. Acceptance is still blocked by five items: the change breaks the repo's Linux CI gate (Critical), the absolute-path branch allows a resource-root symlink escape the brief forbids, invalid/negative handles raise a Lua error instead of returning false (lead-named parity check), the real-Windows-MCI test still converts production failures into silent skips and omits the required "query" leg, and the "successful reload closes the old bank" assertion is vacuous. The brief's RED→GREEN requirement remains unevidenced. |
| **Code quality** | **NEEDS WORK** — structure, bounds and lifecycle wiring are clean and idiomatic for this codebase (per-bank quotas, checked IDs, bounded dedup ring, no closure cycle, no production dummy backend). Remaining defects are narrow: swallowed stop/volume/query errors, unbounded diagnostic message length, one lossy path formatting call, a duplicated path resolver with different failure semantics, a redundant never-read field, and platform-gating/coverage gaps in the new tests. |

Blocking for merge: **Critical 1**, **Important 2, 3, 4, 5**. Important 4 is now a hardening/latent-regression
item rather than an unproven claim, because rev. 2 confirms the native path ran on this machine.

---

## Critical

### C1 — Two new sound tests fail on the repo's Linux CI gate (`cargo test --locked --workspace`)

* `crates/eqoa/src/fork_ext/addons/sound.rs:1144-1157` — `forge_sound_monotonic_id_exhaustion_guard` asserts
  `matches!(bank.last_diagnostic(), Some(SoundDiagnostic::IdExhausted(_)))`.
* `crates/eqoa/src/fork_ext/addons/sound.rs:1178-1194` — `forge_sound_handle_and_file_size_limits` asserts
  `matches!(bank.last_diagnostic(), Some(SoundDiagnostic::FileTooLarge { .. }))`.

Neither test is platform-gated, but both assert diagnostics that only the `#[cfg(windows)]` branch of
`load_sound` can produce. On non-Windows, `load_sound` takes the `#[cfg(not(windows))]` arm
(`sound.rs:420-425`) and records `SoundDiagnostic::UnsupportedPlatform`, so both `matches!` assertions fail.

Evidence that these tests run on Linux:

* `crates/eqoa/src/fork_ext/mod.rs:5` — `#[cfg(not(target_arch = "wasm32"))] pub mod addons;` → the module is
  compiled on Linux, and `#[cfg(test)] mod tests` (`sound.rs:1005-1006`) with it.
* `crates/eqoa/src/main.rs:132` — `mod fork_ext;` is unconditional.
* `.github/workflows/dev.yml:75-77` — the Linux job runs `cargo test --locked --workspace`.

The preserved log is a Windows-local filtered run
(`cargo test --locked -p eqoa fork_ext::addons -- --nocapture --test-threads=1`,
`implementation-tests.log:3`, `running 125 tests` at line 110), so this gate was never exercised.
AGENTS.md names that same command as "the workspace suite, matching Linux CI".

The build log corroborates the platform asymmetry from the other side: `implementation-tests.log:61` warns
`variant UnsupportedPlatform is never constructed` on Windows — the mirror image of the failure on Linux.

Secondary fragility in the same test: it depends on `C:\Windows\Media\tada.wav` existing on the machine
(`sound.rs:1150-1151`); on a Windows image without it the diagnostic would be `FileNotFound`, not `IdExhausted`.

**Remediation:** add `#[cfg(windows)]` to both tests (matching the platform-specific tests already at
`sound.rs:1228`, `sound.rs:1324`, `sound.rs:1398`, `sound.rs:1444`, `sound.rs:1472`, `sound.rs:1518`), or make
the assertions platform-aware; make the ID-exhaustion test independent of the Windows Media folder (point it at
a temp file it creates); then run `cargo test --locked --workspace` on Linux before the branch is merged.

---

## Important

### I2 — Absolute-path branch permits a resource-root symlink escape outside `script_root`

`crates/eqoa/src/fork_ext/addons/sound.rs:765-796`, specifically the resource-root loop at `sound.rs:785-791`:

```rust
for res_dir in resource_dirs {
    if let Ok(canon_res) = fs::canonicalize(res_dir) {
        if canonical.starts_with(&canon_res) && canonical.is_file() { return Ok(canonical); }
    }
}
```

The resource root is re-canonicalized **at load time**, and the candidate is accepted if it lies under that
*fresh* canonical target — with no check that the target is still inside the active `script_root`.
`discovery.rs:74-113` (`resolve_support_dir`) only enforces "inside the canonical root" **at discovery time**,
so a resource directory that is replaced by a junction/symlink after discovery now resolves elsewhere and the
absolute branch becomes an escape hatch for files outside `script_root`.

This is exactly the risk the lead named: *"canonical resource roots checked at every load against active
script_root, including roots changed after discovery"*, and it contradicts the brief's *"Resource-root
symlinks must not open escape outside script_root"* and the preflight ruling *"Enforce active script_root
containment for addon resources; only system media exception"*.

The **relative** branch is safe: `sound.rs:813` additionally requires
`canonical_candidate.starts_with(&canonical_script_root)`. The sibling texture path is safe on both counts:
`textures.rs:330-346` requires script-root containment **and** membership in the *discovery-time* resource
directories, so a swapped resource directory cannot widen it.

**Remediation:** in the absolute branch require `canonical.starts_with(&canon_root)` (script_root), the way
`textures.rs:330` does, keeping the system-Media exception as the only other root; or, if resource roots
outside `script_root` must stay usable, require the freshly canonicalized root itself to be contained in the
canonical script/root before using it as an allowlist root. Add a regression test that replaces a resource
directory with a symlink after discovery (explicit privilege skip, matching the repo's existing symlink tests).

### I3 — Invalid/negative handles raise a Lua error instead of returning `false` (parity deviation)

All five mutating bindings take `Option<u32>` (`sound.rs:869-938`: PlaySound 869-897, StopSound 899-907,
IsSoundPlaying 909-917, SetSoundVolume 919-927, ReleaseSound 929-938). mlua 0.12.2 converts integers with
`num::cast` (`mlua-0.12.2/src/conversion.rs:784-812`), so a negative or non-integral Lua number fails the
`u32` conversion with *"out of range"* and the call raises a Lua error.

The pinned reference does the opposite: `core.cpp:814-835` and `audio_manager.cpp:44-48, 87-91, 101-108,
111-124, 127-133` return `false` / no-op for **any** unknown `int`, including negatives, because
`FindSound` simply misses the map.

Impact: `UiForge.PlaySound(-1)` aborts the addon's Lua frame, and the manager then marks the addon `Failed`
(`manager.rs:259` → `fail()` → `disable(Disposition::Retire)` → bank closed), where the reference would have
returned `false` and left the addon running. The lead explicitly asked to *"Cross-check parser negative IDs
and finite Lua number handling instead of trusting uncompiled draft"*; this is that check failing.

**Remediation:** accept `Option<i64>` (or `Value`) and map non-positive / out-of-`i32` values to `None`
(→ `false` / no-op), then add Lua-level cases for `-1`, `0`, `2^31`, `1.5`, and a string handle.

### I4 — Real-MCI test: the native path did run, but production failures are still masked and the "query" leg is missing

**Revised in rev. 2.** The preserved log settles the question the lead raised: the real-MCI test did **not**
skip. `implementation-tests.log` contains **no** `"SKIP: Windows MCI audio device not available or open
failed"` and **no** `"SKIP: Windows MCI play returned false"` line; the only four `SKIP` messages in the whole
file are the pre-existing symlink-privilege ones (lines 141, 143, 145, 201). Both skip branches of the test
(`sound.rs:1237-1246` and `sound.rs:1260-1268`) always print before returning, so their absence means neither
branch was taken. Consequently, in the recorded run:

* the production `load_sound` **succeeded** — MCI accepted the `\\?\`-prefixed canonical path produced by
  `fs::canonicalize` and opened the generated WAV through `type mpegvideo`;
* `play` returned true, and the asserted `set_sound_volume` / `stop` both returned true (`sound.rs:1261-1262`);
* the log also contains no `"SKIP forge_manager GPU prerequisite"` line, so the three `forge_manager_sound_*`
  tests genuinely executed with a GPU — and their Lua fixture's `UiForge.LoadSound("sound.wav")`
  (`manager.rs:1716-1766`) succeeded against the real backend, corroborating the native path a second time.

So the path-form risk I raised in rev. 1 (whether MCI accepts a `\\?\` verbatim path) is **refuted for this
machine and this path form** — that should be recorded as a positive result, not left open.

What still needs fixing:

1. **The masking design remains.** On any machine without an audio device — or after any future change to the
   path form, the MCI command, or the generated WAV — the test silently reports `ok`. The brief requires
   *"Never hide assertions behind blanket skip"* and *"A successful open alone is not proof of playback"*; a
   production `open`/`play` failure must be a hard test failure that prints the exact MCI error, and only a
   genuine "no MCI device" precondition may skip.
2. **The "query" leg is missing.** `is_sound_playing` is never called in the real test, although the brief's
   item 5 lists "native load/cached id/play/**query**/stop/volume/release". A successful `play` command is not
   the same as the device reporting `status … mode` = `playing` (`sound.rs:588-601`).
3. **The report does not state the outcome or cite the log.** A reviewer had to reconstruct "the native test
   did not skip" from the *absence* of skip lines. `FORGE-SOUND-REPORT.md` should state plainly that the real
   MCI open/play/volume/stop executed with no device skip, and cite `run/forge-sound-stage/implementation-tests.log`.

**Remediation:** probe for an MCI device explicitly and skip only for that reason; make a production `open` or
`play` failure a hard failure with the MCI error text; assert `is_sound_playing` after `play`; document the
supported MCI path form (`\\?\` accepted on this host) in the report.

### I5 — "Successful replacement must close old bank" is a vacuous assertion

`crates/eqoa/src/fork_ext/addons/manager.rs:1843-1853`:

```rust
fs::write(scripts.0.join("a/main.lua"), "count=(count or 0)+1").unwrap();   // replacement loads no sound
...
assert!(
    manager.entries[0].instance.as_ref().map_or(true, |i| { i.sound_bank.borrow().handles() != old_handles }),
    "successful replacement must close old bank"
);
```

The replacement script never loads a sound, so the new bank has zero handles; `[] != [SoundId(1)]` is trivially
true, and `map_or(true, ...)` also passes when the instance is absent. The assertion therefore passes even if
the old bank's MCI aliases were never closed — i.e. it does not test the property named in its message. The
`old_handles` `Rc` captured at `manager.rs:1826-1831` is only used for the failed-reload comparison.

**Remediation:** keep the old `Rc<RefCell<SoundBank>>` and assert on it after the successful reload —
`old_bank.borrow().len() == 0` and `old_bank.borrow().is_empty()` — and additionally assert that the new
instance's bank is a different object than the old one.

---

## Minor

### M6 — Stop / volume / query errors are swallowed without a diagnostic

`sound.rs:573-587` (`stop_sound`) returns `self.backend.send(&format!("stop {alias}")).is_ok()` and discards the
error text; `sound.rs:588-601` (`is_sound_playing`) and `sound.rs:603-626` (`set_sound_volume`) do the same.
`stop_all` (`sound.rs:652-664`) *does* record one, so the behaviour is inconsistent. The lead named this:
*"Stop/close errors need diagnostics"*. Remediation: record `SoundDiagnostic::MciCommandFailed` (or a dedicated
variant) on failure, as `stop_all` already does.

### M7 — Diagnostic messages are unbounded in length

`sound.rs:398-407` bounds the ring to 32 deduplicated entries but not the message size; an entry embeds the
whole MCI command — including the absolute path — plus up to 256 chars of MCI error text
(`sound.rs:486-495`). The lead named this: *"Bank diagnostics bounded in count and message size"*.
Remediation: truncate each recorded message to a fixed cap (e.g. 512 bytes).

### M8 — One lossy path formatting call remains in the path pipeline

`sound.rs:488` formats the canonical path with `canonical.display()`, i.e. `to_string_lossy`. Today this is
lossless because the canonical path is already proven UTF-8 (`sound.rs:768-772`, `sound.rs:806-811` reject
non-unicode canonical paths), but it is the single lossy conversion left in a pipeline the brief describes as
"no lossy paths". Remediation: build the command from `canonical.to_str()` (already validated) and keep the
UTF-16 conversion in one place.

### M9 — The relative-path resolver duplicates `package::resolve_resource` with different failure semantics

`sound.rs:798-822` vs `addons/package.rs:73-93`. The package resolver **fails closed** on the first existing
candidate that escapes the script root (`ResourceError::EscapesScriptRoot`), and rejects empty path segments and
leading separators. The sound resolver keeps scanning the remaining resource roots and finally reports
`FileNotFound`, so a private-root escape can silently fall through to a shared root; it also accepts empty
segments. Remediation: reuse `package::resolve_resource` for the relative branch (as `textures.rs:311` does)
and keep only the sound-specific absolute branch separate — one resolver, one containment policy.

### M10 — Redundant field and dead code the build already flagged

`SoundRecord.id` (`sound.rs:288`) duplicates the `HashMap` key and is never read — the compiler says so
(`implementation-tests.log:72-73`, "field `id` is never read"). The same build reports `SoundId::as_i32`
(`sound.rs:73`) and `RecorderBackend::push_response` never used, and `with_backend`, `owner_id`,
`clear_diagnostic`, `is_empty` never used outside tests (`implementation-tests.log:80-81`, `:98-99`). The
report acknowledges the dead-code warnings but does not list them; the fix is to drop the unused accessors and
the redundant field, or to keep only what the public API needs.

### M11 — Report accuracy: "failed reload", the head sha, the native-test outcome, the skip inventory, build logs

`docs/FORGE-SOUND-REPORT.md`:

* **Lines 50-51** — *"Terminal events (retire, failed reload, eject, shutdown) call `close_all()`"* is wrong for
  the case the tests cover; the implementer has now admitted this in the follow-up. See the resolution below.
  The wording must be corrected before the report is published.
* **Line 12** — *"Head: local commit on `forge-compatibility` after this report"*; the actual head is `3ffe587`
  (the diff header). State the sha.
* **Lines 66-75** — the skip inventory is now *confirmed* by the preserved log (exactly four symlink-privilege
  skips; the real-MCI test did not skip) — good — but the report should say so explicitly and cite
  `run/forge-sound-stage/implementation-tests.log`, and it should state that the native MCI open/play/volume/stop
  actually executed rather than leaving the reader to infer it.
* **Lines 84-88** — the build lines carry no captured transcript: there is no saved log for
  `cargo build --locked -p eqoa` or for the `wasm32-unknown-unknown` check (exit 0 is reported; the WASM target
  directory was removed afterwards). The preserved log covers only the filtered test run. AGENTS.md asks for
  "exact commands and every SKIP"; the build evidence should be captured the same way.
* **Lines 89-98 (Concerns)** — does not state the limitation the brief demands: *"Explicitly report inability
  to guarantee OS release if MCI itself fails, do not claim all resources reclaimed based on mock."* `close_all`
  can fail (`sound.rs:666-690`), the failed record is retained, and `Drop` (`sound.rs:692-698`) makes a final
  silent attempt. The report should say so instead of implying full reclamation.
* **RED→GREEN is absent.** Nothing in the report or the package records a RED observation. The implementer
  describes the negative cases as "controlled negatives"; those are invalid-input unit tests (path sanitization,
  NaN volume, quota rejection before the OS call) — legitimate negative *controls* in the AGENTS.md sense, but
  they are not the brief's item 6 *"observe meaningful RED before activating implementation"* and not an oracle
  against a deliberately broken implementation. Treat RED→GREEN as **not evidenced** and say so in the report.

### M12 — Handle namespace differs from the reference

The reference keeps one process-wide map and counter (`audio_manager.cpp:20-22`), so handles are globally unique.
Each bank here restarts at 1 (`sound.rs:818`, `sound.rs:483`); only the MCI alias is unique per bank
(`sound.rs:485`). Handle `1` from two addons therefore denotes different sounds. Not reachable today (each addon
owns its Lua state and bank — `host.rs:47-51`, `host.rs:121`), but it is a parity deviation the brief asked to
be reported ("report any mismatch").

### M13 — New-test coverage gaps and one timing-sensitive assertion

* The aggregate 64 MiB budget (`sound.rs:463-470`) is never exercised by a test.
* The `close_all` **retry** path (`sound.rs:666-690`: a failed close below `MAX_CLOSE_RETRIES` increments
  `close_attempts` and retains the record) is never exercised; the existing test only covers the
  already-at-`MAX_CLOSE_RETRIES` early-out (`sound.rs:1196-1225`).
* `manager.rs:1716`'s test name says "…reenable_plays…" but no playback is attempted after re-enable.
* `sound.rs:1261-1262` asserts `stop_sound` returns `true` immediately after a 100 ms clip
  (`generate_minimal_pcm_wav(4410)` at 44.1 kHz). In the recorded run it passed, so MCI's `stop` on this device
  succeeded; the assertion remains timing-dependent if the clip has already ended on a slower host.

---

## Requested resolution: report says "failed reload closes", the test says "preserves"

**Actual behaviour (resolved from source, and now corroborated by the preserved log):**

* A **failed compile** reload preserves the old bank and its playback state.
  `manager.rs:496-517`: `Command::Reload` calls `self.prepare(&entry.metadata)`; on `Err` it only records a
  diagnostic and **does not touch the existing instance** — no `disable`, no `set_active(false)`, no `close_all`.
  This matches the brief (*"Failed compile reload preserves old bank/playing state"*) and the test
  `forge_manager_sound_failed_reload_preserves_old_bank` (`manager.rs:1805-1841`), which is green in the
  preserved log (`implementation-tests.log:181`). The pre-existing base test
  `forge_manager_reload_compile_failure_keeps_old_vm_and_rescan_retires_only_removed`
  (`implementation-tests.log:180`) encodes the same VM-preservation rule for the surrounding machinery.
* A **successful** reload closes the old bank: `manager.rs:510` runs `disable(..., Disposition::Retire)`, which
  reaches `manager.rs:437-441` → `sound_bank.close_all()`, before `entry.instance = Some(new)`.
* A reload that **compiles but then fails at runtime** does close: `fail()` (`manager.rs:448-452`) calls
  `disable(..., Disposition::Retire)` → `close_all()`.

So the code is correct and brief-compliant; the report sentence at `FORGE-SOUND-REPORT.md:50-51` is inaccurate
and (per the implementer's own follow-up) already acknowledged as such. It must be reworded to "a reload that
fails to compile preserves the old bank; a successful or runtime-failed reload closes it".

---

## Evidence assessment (rev. 2 — what the preserved log proves and what it does not)

Preserved artifact: `run/forge-sound-stage/implementation-tests.log` (252 lines, 37,858 bytes) — the filtered
test run from the worktree. Shape: `Compiling eqoa` (line 1) → `Finished test profile [unoptimized +
debuginfo] target(s) in 16.55s` (line 108) → `Running unittests src\main.rs` (line 109) → `running 125 tests`
(line 110) → `test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 987 filtered out; finished in
19.14s` (line 251). The command line is recoverable from the PowerShell error record at line 3:
`cargo test --locked -p eqoa fork_ext::addons -- --nocaptu…`.

**Proven by the log**

* All 14 `forge_sound_*` and 3 `forge_manager_sound_*` tests ran and passed (14 + 3 matches the report).
* The **native** Windows MCI path executed: no MCI skip line, therefore production `open` succeeded and `play`
  returned true, and the asserted `set_sound_volume` / `stop` succeeded (`sound.rs:1261-1262`). The
  `\\?\`-prefixed canonical path is accepted by MCI on this host.
* The GPU prerequisite was met (no `SKIP forge_manager GPU prerequisite`), so the three manager lifecycle tests
  genuinely exercised the production integration, including a successful `UiForge.LoadSound` inside their Lua
  fixtures.
* Exactly four skips, all the pre-existing Windows 1314 symlink-privilege ones (lines 141, 143, 145, 201) —
  the report's skip inventory is confirmed. The 125 = 108 baseline + 14 + 3 arithmetic is consistent.
* The build emitted 12 warnings, 5 from `sound.rs` (lines 53, 62, 73, 81, 99), corroborating the report's
  dead-code note and adding the never-read `SoundRecord.id` field and the unused `as_i32`
  (see M10).

**Not proven by the log (evidence gaps, stated exactly)**

1. **No RED observation.** The log is GREEN-only. There is no saved pre-implementation run and no
   deliberately-broken-implementation oracle. The negative cases that exist are invalid-input unit tests
   ("controlled negatives"), which satisfy the AGENTS.md negative-control habit but not the brief's item 6
   "Focused RED→GREEN … observe meaningful RED before activating implementation". RED→GREEN: **not evidenced**.
2. **No pinned-C++ oracle run.** No test compares the Rust behaviour against the pinned UiForge C++ on the same
   inputs; parity in this review is established by reading both sources side by side (as I did for I3, M12 and
   the command forms), not by an executed comparison.
3. **No Linux workspace run.** The log is Windows-only and filtered to `fork_ext::addons`. C1 is therefore
   *untested on Linux*, not disproven — and by inspection the two ungated tests will fail there.
4. **No build transcripts.** There is no saved log for `cargo build --locked -p eqoa` (the log's line 1/108 are
   the *test* profile compile) nor for the `wasm32-unknown-unknown` check; exit 0 is reported and the WASM
   target directory was removed afterwards. The build verification is **asserted, not evidenced**.
5. **No audible-playback proof.** MCI returning success for `play` is not proof that sound was rendered; the
   brief itself defers user listening to a later manual gate, so this is not a defect — but the report should
   not read as if playback was heard.
6. **The 987 filtered-out tests** in the run are outside `fork_ext::addons`; the report's "125 passed" is a
   filtered figure, and the brief asked for one pinned serial addon suite, which this satisfies.

---

## What I verified as correct (so the lead can rely on it)

Traced in source, no assumption of test results:

**Lifecycle.** Banks start inactive and are activated only for enabled entries (`manager.rs:242-243`, after
`prepare` installs the bindings, so bindings exist before the first entry). Ordinary disable
(`manager.rs:410-446`, `Disposition::Retain`) deactivates the bank (`sound.rs:352-357` → `stop_all`), stops
playback and **retains** handles and the same VM; re-enable restores playback *eligibility* only —
`set_active(true)` never sends a `play`, so nothing is auto-replayed. Playback from a disabled callback is
blocked because `play_sound` requires `self.active` (`sound.rs:523-526`) and the bank is deactivated *before* the
cleanup callback runs (`manager.rs:425-427`). Removal / change rescan close (`manager.rs:559`, `manager.rs:587`).
Shutdown/OnEject closes twice — before the eject callback (`manager.rs:608` → `437-441`) and again after it
(`manager.rs:619-625`) — so a throwing or asset-allocating final callback cannot leak an alias. `Drop` makes a
final close attempt (`sound.rs:692-698`). Failed closes keep the record counted (`sound.rs:666-690`,
`sound.rs:628-648`). Runtime pause/error stops audio and blocks playback until a genuinely successful frame
re-activates (`runtime.rs:208-214`, `runtime.rs:291-299`, `runtime.rs:564-567`; the managed branch is gated on
`self.error.is_none()` at `runtime.rs:265`, so no update — and no re-activation — happens while suspended).

**Quotas, IDs, nil, volume.** Quotas are enforced before any OS call, in the order
dedupe → handle limit → regular file → ≤16 MiB → aggregate ≤64 MiB → ID exhaustion → MCI open
(`sound.rs:437-497`); the recorder test asserts a quota rejection never calls the backend
(`sound.rs:1472-1516`). IDs are positive and ≤ `i32::MAX` (`sound.rs:59-66`), the owner token uses a checked
atomic (`sound.rs:47-50`), and exhaustion is detected before the OS allocation (`sound.rs:472-483`). A canonical
duplicate load reuses the id (`sound.rs:437-439`). Volume is clamped to `[0,1]` with NaN/Inf rejected
(`sound.rs:101-106`) and MCI truncation `volume*1000` (`sound.rs:543`, `sound.rs:622`) matches
`ToMciVolume` (`audio_manager.cpp:51-56`). All five mutators are nil-safe (`sound.rs:869-938`); `ReleaseSound`
returns nothing (`sound.rs:929-938`), matching `core.cpp:829-835`.

**Parity with the pinned C++.** Command forms match one-for-one: `open "…" type mpegvideo alias …`
(`sound.rs:486-490` vs `audio_manager.cpp:73`), `setaudio … volume to N` (`sound.rs:544`),
`seek … to start` then `play …[ repeat]` (`sound.rs:552-570` vs `audio_manager.cpp:93-97`), `stop`,
`status … mode`, `close`. `Play` ignores setaudio/seek failures for the boolean but records diagnostics and
returns the real play outcome — exactly the reference behaviour (`audio_manager.cpp:85-98`) and the brief's.
`LoadSound` returns a positive integer or `nil` (never 0) (`sound.rs:865` vs `core.cpp:775-792`).
Option keys match (`volume`, `loop`) (`sound.rs:876-893` vs `core.cpp:804-810`).

**Containment / UTF-16.** Quotes, NUL, control characters, drive-relative paths and `..` segments are rejected
before resolution (`sound.rs:705-737`) and the canonical path is re-sanitized afterwards (`sound.rs:773`,
`sound.rs:812`). The relative branch requires canonical containment in the active `script_root`
(`sound.rs:813`). The system-Media exception compares **canonical** paths on both sides
(`sound.rs:826-843`) — it is not a candidate-directory-name exception, which was a named lead risk. Files must be
regular and within the size bounds (`sound.rs:448-461`). The command text is converted with `encode_utf16()`
(`WindowsMciBackend::send`), so UTF-16 code units are preserved for the command; the preserved log confirms MCI
accepts the `\\?\` path form this code produces on Windows.

**No dummy backend in production, no cycles.** `default_backend()` returns `WindowsMciBackend` on Windows and
`load_sound` short-circuits with an explicit `UnsupportedPlatform` diagnostic elsewhere
(`sound.rs:276-285`, `sound.rs:412-425`); `UnsupportedBackend` is never reached by a production call. The Lua
closures hold the bank's `Rc` and the bank holds no Lua state, so there is no ownership cycle; the manager drains
the bank's diagnostics each frame into its own channel (`manager.rs:262-271`).

---

## Required follow-ups before this stage is accepted

1. Fix C1 (platform-gate the two tests, drop the Windows-Media dependency) and run the Linux workspace suite.
2. Fix I2 (script-root containment for absolute paths) and add the symlink-escape regression test.
3. Fix I3 (negative/invalid handles → `false`, not a Lua error) and extend the Lua contract test.
4. Fix I4 (production MCI open/play failure must fail the test, add the `is_sound_playing` assertion) and state
   in the report that the native MCI open/play/volume/stop executed with no device skip, citing the preserved log.
5. Fix I5 (assert the old bank is emptied after a successful reload).
6. Correct `FORGE-SOUND-REPORT.md`: the "failed reload" sentence, the head sha, the native-test outcome and the
   log citation, the missing build transcripts, the RED→GREEN gap, the MCI-close limitation, and the C++
   mismatches actually present (negative handles, per-bank handle namespace).
7. Optional but cheap: M6-M10 (stop/volume/query diagnostics, message-size cap, `to_str()` instead of
   `display()`, reuse `package::resolve_resource`, drop the redundant field and unused accessors), M12-M13
   (document the handle-namespace deviation; cover the aggregate budget and the `close_all` retry path).

---

# Fix round 1 re-review (3ffe587 → ada9092) — scoped, read-only

Reviewer: MiniMax-M3. Date: 2026-10-29 (rev. 3). No source edits, no builds, no test runs, no subagents.
Inputs: `run/forge-sound-stage/review-fix1.txt` (full diff 3ffe587..ada9092), the appended rulings in
`run/forge-sound-stage/task-1-brief.md` (lines 48-52), the post-fix worktree sources, the worktree
`docs/FORGE-SOUND-REPORT.md`, and the preserved logs `forge-addons-fix1.log`,
`forge-sound-native-required.log`, `forge-native-build-fix1.log`, `forge-wasm-check-fix1.log`
(plus `forge-sound-pre-fix.log` in the temp staging directory).

## Overall verdict

**Fix round 1 does not clear the round.** Two of the five blocking findings (C1, I2) are untouched, and the
round **introduced new breakage**: a third non-Windows-breaking test and a containment regression in the
relative-path branch that 3ffe587 did not have. I3 and I5 are properly fixed; I4 is partially fixed.
Of the lead's "low-cost existing-scope" items, only the lossy-path formatting, the aggregate/close-retry
assertions and part of the report work landed; the stop/query/volume diagnostics and the diagnostic
message-size bound were not touched.

| Prior | Finding | Status in ada9092 |
|---|---|---|
| **C1** | Two ungated tests break the Linux CI gate | **NOT ADDRESSED** — and the round added a **third** Linux-breaking test (N1). The report now asserts the opposite (N7). |
| **I2** | Absolute-path resource-root symlink escape | **NOT ADDRESSED** — absolute branch unchanged; the relative branch **regressed** (N2). The new test does not exercise a resource root outside `script_root`. |
| **I3** | Negative/invalid IDs raised Lua errors | **ADDRESSED** ✓ (`parse_lua_sound_id`, `Option<i64>`, new Lua test) |
| **I4** | Real-MCI masking / missing query leg | **PARTIALLY ADDRESSED** — required-native option added and used ✓, query assertions added ✓, but the default path still blanket-skips production errors and the ruling's "long enough quiet clip or loop" was not applied (N3). |
| **I5** | Vacuous "successful reload closes old bank" | **ADDRESSED** ✓ (`manager.rs` now asserts the old bank is empty) |
| M6 | Stop/query/volume errors swallowed (lead ruling) | **NOT ADDRESSED** |
| M7 | Diagnostic message size unbounded (lead ruling) | **NOT ADDRESSED** |
| M8 | Lossy `Path::display()` in the MCI command | **ADDRESSED** ✓ |
| M9 | Resolver de-duplication | Closed — deferred by the lead's ruling |
| M10 | Unused accessors / redundant field | **PARTIAL** — `with_backend` removed; the rest suppressed by a file-level `#![allow(dead_code)]` (N6) |
| M11 | Report accuracy | **PARTIAL** — several fixes landed, one new false statement (N7) |
| M12 | Per-addon handle namespace | Closed — the lead ruled it a documented adaptation |
| M13 | Coverage gaps | **PARTIAL** — aggregate ✓, close-retry ✓, clip length ✗ (N3) |

## New breakage introduced by this diff (fix-diff-only findings)

### N1 (Critical) — a third test breaks the Linux CI gate

`crates/eqoa/src/fork_ext/addons/sound.rs` — `forge_sound_aggregate_budget_rejects_before_os_open`
(new in this round, `#[test]` with **no** `#[cfg(windows)]`) calls the production `load_sound` and asserts
`Some(SoundDiagnostic::AggregateLimitExceeded { .. })`. On non-Windows `load_sound` short-circuits at
`sound.rs:420-425` and records `UnsupportedPlatform`, so the assertion fails — exactly the failure mode of the
two pre-existing ungated tests (C1). The round therefore moved the Linux-breaking test count from 2 to 3,
while the report states the opposite (N7).

**Remediation:** `#[cfg(windows)]` on that test, or split it so the platform-independent part (budget
arithmetic) and the platform-dependent part (the diagnostic) are separate; and add the "explicit
unsupported-platform expectation" the lead's ruling asks for (e.g. a test asserting `UnsupportedPlatform`
on `#[cfg(not(windows))]`).

### N2 (Critical) — containment regression: the relative branch lost the `script_root` check

The diff **removed** the script-root containment from the relative branch and replaced it with containment in
the freshly canonicalized resource directory:

```diff
-    let canonical_script_root = fs::canonicalize(script_root) ...;
     for res_dir in resource_dirs {
         let candidate = res_dir.join(Path::new(&normalized));
         if candidate.exists() || candidate.is_symlink() {
-            if canonical_candidate.starts_with(&canonical_script_root) && canonical_candidate.is_file() {
-                return Ok(canonical_candidate);
+            if let Ok((canonical_candidate, candidate_str)) = canonicalize_sound_path(&candidate) {
+                let canon_res = fs::canonicalize(res_dir) ...;
+                if is_contained_in(&canonical_candidate, &canon_res) {
+                    return Ok((canonical_candidate, candidate_str));
```

In the post-fix source the relative branch is now (worktree `sound.rs`, `resolve_sound_path`):

```rust
for res_dir in resource_dirs {
    let candidate = res_dir.join(Path::new(&normalized));
    if candidate.exists() || candidate.is_symlink() {
        if let Ok((canonical_candidate, candidate_str)) = canonicalize_sound_path(&candidate) {
            let canon_res = fs::canonicalize(res_dir)...;
            if is_contained_in(&canonical_candidate, &canon_res) {
                return Ok((canonical_candidate, candidate_str));
            }
        }
    }
}
```

`script_root` is no longer consulted in this branch at all. Consequence: a resource directory that is (or is
swapped into) a junction/symlink whose target is outside the addon `script_root` now serves files **by relative
name** — the exact case the brief forbids ("Relative resources … canonically within active script_root";
"Resource-root symlinks must not open escape outside script_root"). 3ffe587 blocked it (the
`starts_with(&canonical_script_root)` check); ada9092 permits it. This is a **narrowing of a security check in
the fix round**, not merely an unfixed item.

The absolute branch is unchanged and still accepts any file under a freshly canonicalized resource root
(`resolve_sound_path`, `for res_dir in resource_dirs { … is_contained_in(&canonical, &canon_res) }`), so the
original I2 escape also remains open.

The new test `forge_sound_absolute_path_outside_roots_rejected` does **not** cover either case: it passes
`resource_dirs = [script_root.clone()]` (the resource root *is* the script root), so it exercises only a plain
out-of-root path and a prefix-trap file (`addonspoof.wav`) — the "superficial prefix trap" the lead asked me not
to accept as coverage. No test uses a resource root outside `script_root`, and none swaps a resource directory
for a symlink after discovery.

**Remediation (single change, satisfies both branches and the lead's "defer dedup unless needed for
containment" note):** keep the script-root requirement in the relative branch
(`canonical_candidate.starts_with(&canonical_script_root)`), and in the absolute branch require the resource root
to be usable *and* contained — i.e. accept an absolute path only when it is under the canonical `script_root`, or
under a canonical resource root that is itself inside the canonical script/root (the system-Media exception
stays). Then add the missing regression test: a resource directory replaced by a symlink/junction after
discovery, with an explicit privilege skip in the repo's established style.

### N3 (Important) — the native clip is still 100 ms while the ruling requires a long-enough clip or loop

The ruling (brief line 51) requires the native test to *"query playing state using a long enough quiet clip or
loop"*. The clip is unchanged: `generate_minimal_pcm_wav(4410)` = 4410 samples at 44.1 kHz = **0.1 s**
(worktree `sound.rs:1250`). The new assertion

```rust
assert!(bank.is_sound_playing(Some(id)), "sound must report playing");
```

is therefore timing-dependent: if the clip ends before the `status … mode` query, the device reports `stopped`
and the test fails. It passed in the recorded run, so this is a latent flake, not a current failure — but the
ruling named the remedy and it was not applied.

**Remediation:** use a longer clip (e.g. ≥2 s of silence) or `play … repeat`, so the "playing" query is
deterministic.

### N4 (Minor) — the close retry cap is now silent

New in `release_sound`:

```rust
if record.close_attempts >= MAX_CLOSE_RETRIES {
    self.sounds.insert(id, record);
    return;
}
```

The terminal retry returns without recording anything, so the last close failure is invisible. The lead's ruling
requires close diagnostics ("stop/query/volume diagnostics"), and the surrounding code already records
`MciCloseFailed` on every failed attempt. **Remediation:** record a diagnostic (e.g. `MciCloseFailed` with
`attempts = MAX_CLOSE_RETRIES`) on the terminal attempt.

### N5 (Minor) — the "stopped" assertion cannot distinguish a stopped device from a failed status command

`is_sound_playing` returns `false` both when the device is stopped **and** when the `status` command errors
(`sound.rs`, `is_sound_playing`: `Err(_) => false`). The new `assert!(!bank.is_sound_playing(...), "sound must
report stopped")` therefore also passes if the status command fails. **Remediation:** assert on the recorded
diagnostic, or query the mode explicitly in the test.

### N6 (Minor) — file-level `#![allow(dead_code)]` hides future dead code

`#![allow(dead_code)]` at the top of `sound.rs` suppresses the module's warnings wholesale; it does not remove
the dead items (`SoundRecord.id` is still never read, `as_i32`, `push_response`, `owner_id`, `clear_diagnostic`
are still unused outside tests) and it will also silence genuinely dead production code added later. The lead's
ruling asked to "fix … unused test-only accessors". **Remediation:** delete the unused items (the field and the
test-only helpers) instead of blanket-suppressing the module.

### N7 (Minor) — the report contains a new false statement

`docs/FORGE-SOUND-REPORT.md:138-140`: *"Linux: not executed locally; code is gated with `#[cfg(windows)]` for
Windows-specific tests/MCI and `#[cfg(not(windows))]` for the unsupported backend path."* The two pre-existing
tests are **not** gated (C1) and the new aggregate test is **not** gated (N1), so the claim is false; only the
MCI test and the new absolute-path test carry `#[cfg(windows)]`. The report also still gives no head sha
(line 15: "local commit on `forge-compatibility` after this report"), cites its logs under
`C:\Users\hocka\AppData\Local\Temp\opencode\` rather than the durable copies in the stage directory, and
contains no fixed-vs-deferred findings table (ruling line 52). The MCI-close limitation ("Explicitly report
inability to guarantee OS release if MCI itself fails") is still absent from Concerns.

## Evidence re-check (logs only; nothing re-run)

* `forge-addons-fix1.log` — `running 130 tests` → `test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured;
  987 filtered out; finished in 19.66s`; 19 `forge_sound_*` tests listed and all `ok`; the only four `SKIP`
  messages are the pre-existing symlink-privilege ones; the real-MCI test reports `ok` with **no** skip line, so
  the native path ran in this suite too. 125 + 5 = 130 is consistent.
* `forge-sound-native-required.log` — `1 passed; 0 failed` for the focused `forge_sound_real_mci` run, no
  `SKIP`. Together with the new assertions in the source, this is real evidence that the required-native option
  (`EQOA_SOUND_NATIVE_REQUIRED=1` → panic instead of skip) **cannot skip on this host** and that
  `open`/`play`/`IsSoundPlaying`=playing/`SetSoundVolume`/`StopSound`/`IsSoundPlaying`=stopped/`release`
  all executed against the generated WAV. The lead's "required-native test, not a superficial prefix trap"
  requirement is **satisfied on this point**.
* `forge-native-build-fix1.log` — `Compiling eqoa` → `Finished dev profile … in 19.55s`, 14 warnings for the bin
  (the sound-module ones now suppressed). Native build evidence exists ✓.
* `forge-wasm-check-fix1.log` — `Checking …` through the wasm target → `Finished dev profile … in 1m 32s`, 1
  warning. WASM check evidence exists ✓.
* `forge-sound-pre-fix.log` — `running 14 tests` → `14 passed; 0 failed` against the **pre-fix** source. This is
  *not* RED evidence; the report describes it honestly ("pre-fix sound run (no failures; evidence preserved)") and
  does not present it as RED ✓.

**Remaining evidence gaps (unchanged or new):**
1. **No RED / source-negative evidence** for containment or negative Lua IDs (ruling line 51). The round was
   completed without it; it can now only be reconstructed by locally reverting the fix, or the gap must be
   recorded as permanent. The report is silent on this — it should say so explicitly rather than omit it.
2. **No Linux run** — recorded honestly as "not executed" (ruling line 50) ✓, but paired with the false gating
   claim (N7).
3. **No fixed-vs-deferred findings table** (ruling line 52).
4. Log citations point at a temp directory instead of the durable stage-directory copies.

## The lead's two named verification asks

* **"verify resource symlink containment … not just superficial prefix trap"** — **not satisfied.** The absolute
  branch is unchanged (escape open), the relative branch regressed (N2), and the new test only covers a prefix
  trap and a plain out-of-root path with `resource_dirs == [script_root]`.
* **"required-native test … not just superficial prefix trap"** — **satisfied for the required-native option**
  (a real env-gated panic path, used in a focused run whose log is preserved and shows `1 passed`, with the new
  query/volume/stop assertions present in the source). Not satisfied for the ruling's clip-length requirement
  (N3), and the default (non-env) suite still blanket-skips a production open/play failure.

## Unresolved issues, ordered

1. **N2/C1+I2 (Critical, blocking):** restore script-root containment in the relative branch and close the
   absolute-branch resource-root escape; add the resource-root-outside-script-root and post-discovery symlink
   regression tests.
2. **C1+N1 (Critical, blocking):** `#[cfg(windows)]` (or an explicit unsupported-platform expectation) for
   `forge_sound_monotonic_id_exhaustion_guard`, `forge_sound_handle_and_file_size_limits` and
   `forge_sound_aggregate_budget_rejects_before_os_open`; correct the report's Linux-gating claim (N7).
3. **I4/N3 (Important):** lengthen the native clip (or loop) so the "playing" query is deterministic; keep the
   default suite from blanket-skipping production open/play failures.
4. **M6 + N4 (Important, lead ruling):** record diagnostics for stop/query/volume failures and for the terminal
   close retry.
5. **M7 (Minor, lead ruling):** bound the diagnostic message length.
6. **N5, N6, N7, M11 leftovers (Minor):** distinguish "stopped" from a failed status query; remove the unused
   accessors/field instead of a module-wide allow; fix the report (head sha, durable log paths, fixed-vs-deferred
   table, MCI-close limitation, explicit RED-evidence gap).

**Round verdict: not accepted.** I3 and I5 are properly done and the native-required evidence is now real; C1 and
I2 remain open and the round made containment weaker in the relative branch while adding a third
Linux-breaking test.

---

# Fix round 2 re-review (ada9092 → 6cab6c1) — scoped, read-only

Reviewer: MiniMax-M3. Date: 2026-10-29 (rev. 4). No source edits, no builds, no test runs, no subagents.
Inputs: `run/forge-sound-stage/review-fix2.txt` (full diff ada9092..6cab6c1), the Round-2 ruling in
`run/forge-sound-stage/task-1-brief.md` (lines 54-65), the post-fix worktree sources, the worktree
`docs/FORGE-SOUND-REPORT.md`, and the durable logs `forge-res-escape-red.log`, `forge-res-escape-green.log`,
`forge-addons-fix2.log`, `forge-sound-native-required-fix2.log`, `forge-native-build-fix2.log`,
`forge-wasm-check-fix2.log`.

## Verdict: **ACCEPTED for round 2** (one minor follow-up required, see R1)

Every blocking item from rev. 3 and every item the Round-2 ruling names is now genuinely fixed, with the
containment fix verified against the actual pre-fix RED log. The round is scoped: the diff touches only
`crates/eqoa/src/fork_ext/addons/sound.rs` and `docs/FORGE-SOUND-REPORT.md` — no manager/runtime/API growth, no
new dependency (`waveOutGetNumDevs` comes from the existing `winmm` link), no architecture change.

| Prior | Finding | Status in 6cab6c1 | Evidence |
|---|---|---|---|
| **C1** | Ungated tests break the Linux CI gate | **ADDRESSED** | `#[cfg(windows)]` on `forge_sound_monotonic_id_exhaustion_guard`, `forge_sound_handle_and_file_size_limits`, `forge_sound_aggregate_budget_rejects_before_os_open`; the `tada.wav` dependency replaced with a generated temp file; new `#[cfg(not(windows))]` test asserting `UnsupportedPlatform` |
| **I2** | Absolute-path resource-root escape | **ADDRESSED** | The `resource_dirs` bypass is removed from the absolute branch: only `canonical_script_root` containment or system Media is accepted |
| **N2** | Relative branch lost `script_root` containment | **ADDRESSED** | The relative branch now requires the canonical resource root to be inside `script_root` **and** the candidate to be inside both |
| **N1** | Third Linux-breaking test | **ADDRESSED** | `#[cfg(windows)]` added to the aggregate test |
| **I4/N3** | Real-MCI masking, missing query, short clip | **ADDRESSED** | `waveOutGetNumDevs()` preflight before `open`; **any** MCI error with devices present is a hard failure; 2-second looping clip; `query_sound_mode` Result-seam assertions for `playing`/`stopped`; explicit `stop` |
| **I3** | Negative/invalid IDs raised Lua errors | still addressed ✓ | unchanged (`parse_lua_sound_id`) |
| **I5** | Vacuous reload-close assertion | still addressed ✓ | unchanged (old bank asserted empty) |
| **M6** | Stop/query/volume errors swallowed | **ADDRESSED** | All three now record `MciCommandFailed`; new test `forge_sound_stop_query_volume_failure_records_diagnostic` |
| **M7** | Diagnostic message size unbounded | **ADDRESSED (with R1)** | `record()` caps each message; new test `forge_sound_diagnostic_message_truncation` |
| **N4** | Silent terminal close retry | **ADDRESSED** | Records `MciCloseFailed` with "max close retries reached; OS release not guaranteed" |
| **N5** | "Stopped" indistinguishable from a failed query | **ADDRESSED** | Private `query_sound_mode -> Result<String,String>` seam; a failed status call is now distinguishable |
| **N6** | Module-wide `#![allow(dead_code)]` | **ADDRESSED** | Removed; `SoundRecord.id`, `SoundId::as_i32`, `RecorderBackend::push_response`, `clear_diagnostic`, `with_backend`, `owner_id()` deleted; remaining test accessors are `#[cfg(test)] pub(crate)`; `UnsupportedPlatform` is now `#[cfg(not(windows))]` |
| **M8** | Lossy `Path::display()` | still addressed ✓ | unchanged (validated `canonical_str`) |
| **M9 / M12** | Resolver dedup / per-VM handle namespace | Deferred / documented adaptation per the ruling | report table ✓ |

## Containment fix verified against the actual RED log (the lead's specific ask)

`forge-res-escape-red.log` is genuine pre-fix RED evidence, not a reconstruction:

```
thread '...forge_sound_resource_root_outside_script_root_rejected' panicked at
assertion failed: bank.load_sound("outside.wav", &script_root, ...)
test ... forge_sound_resource_root_outside_script_root_rejected ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1117 filtered out
```

It fails on the **relative** branch — i.e. the exact N2 widening introduced by round 1 — which is the bypass the
ruling asked to be demonstrated. `forge-res-escape-green.log` shows the same test `1 passed` after the fix.

The post-fix `resolve_sound_path` (worktree `sound.rs`) now reads:

* absolute → system Media, else `is_contained_in(&canonical, &canonical_script_root)` only — no `resource_dirs`
  widening;
* relative → the canonical resource root must itself satisfy
  `canon_res.starts_with(&canonical_script_root)` (a swapped junction/symlink is skipped), and the candidate
  must satisfy `is_contained_in(candidate, script_root) && is_contained_in(candidate, canon_res)`.

Functional compatibility with the discovery layout was checked: every discovered resource root
(`package_root/resources`, `script_root/resources`, …) is inside the addon's `script_root` for all three
discovery layouts, so private-before-shared resolution is preserved and no legitimate resource becomes
unreachable.

## Native-playback evidence (required-native mode, not a superficial trap)

`forge-sound-native-required-fix2.log`: `1 passed; 0 failed` for the focused `forge_sound_real_mci` run, **no
`SKIP`**. The test now preflights `waveOutGetNumDevs() == 0` (skip only for that genuinely environmental
condition, and panic in required mode) and then **fails hard on any** open / play / volume / stop / query /
close error — there is no remaining blanket skip of an operation error. The clip is 2 s and loops, so the
"playing" query is deterministic, and the test asserts `query_sound_mode == Ok("playing")` before
`SetSoundVolume`/`StopSound` and `Ok("stopped")` after, so a failed status call can no longer masquerade as
"stopped". `forge-addons-fix2.log`: `133 passed; 0 failed; 0 ignored; 987 filtered out`, 22 `forge_sound_*`
tests, and the only skips are the 4 pre-existing symlink-privilege ones (the other three "SKIP" hits are test
*names*). Native build and WASM check logs both end in `Finished`.

## Report (worktree `docs/FORGE-SOUND-REPORT.md`)

Now accurate on the points that were wrong: the failed-reload wording is corrected to "**failed-compile** reload
preserves the old bank/playing state"; the Linux statement now names the three `#[cfg(windows)]` tests instead of
claiming a blanket gate; the Fixed/Deferred findings table is present and matches my verdicts; the
MCI-close limitation ("Full OS resource reclamation cannot be guaranteed…") is stated; the native run's
`waveOutGetNumDevs() > 0` outcome is stated; the durable log paths are listed; and the missing historical RED for
negative Lua IDs is **explicitly documented** rather than glossed over.

## Remaining items (not blocking this round)

* **R1 (Minor, new in this diff) — `String::truncate` can panic on a non-char boundary.**
  `sound.rs`, `record()`: `text.truncate(MAX_DIAGNOSTIC_CHARS)` with `MAX_DIAGNOSTIC_CHARS = 2048` is a **byte**
  index (the constant is even named `…CHARS`), and `String::truncate` asserts the index is a char boundary.
  A diagnostic longer than 2048 bytes whose byte 2048 falls inside a multi-byte character (a long non-ASCII
  addon path in the MCI command, or the U+FFFD replacement characters `String::from_utf16_lossy` inserts for
  lone surrogates in MCI error text) panics instead of truncating. mlua converts the panic into a Lua error, so
  the blast radius is "that addon frame fails", not a process crash — but it is an avoidable panic in the code
  path the ruling asked to harden. The truncation test only exercises ASCII.
  **Remediation:** truncate on a boundary, e.g. `let mut cut = MAX; while !text.is_char_boundary(cut) { cut -= 1; }`
  `text.truncate(cut);` (and rename the constant to `MAX_DIAGNOSTIC_BYTES`, or make it a true char cap).
* **R2 (Minor, carried) — report head sha.** `FORGE-SOUND-REPORT.md` still says "Head: local commit on
  `forge-compatibility` after this report"; the head is `6cab6c1`.
* **R3 (accepted, documented) — historical RED for negative Lua IDs** was never captured; the report now says so
  explicitly, which is what the ruling asked for. Only the containment RED exists.
* **R4 (Minor, carried) — resolver duplication** and the per-VM numeric handle namespace remain documented
  scoped adaptations per the Round-1/2 rulings; no action.
* **R5 (Minor, informational) — no Linux run.** Still recorded as "not executed"; the platform gating is now
  real, so the previous CI-breakage risk is closed by inspection, but Linux coverage remains unproven.

**Round 2 verdict: accepted.** C1, I2, N1, N2, I4/N3, M6, M7, N4, N5, N6 are addressed with evidence; R1 should be
folded into the next commit (one-line fix) before the branch is merged, and R2 is a one-line report edit.

---

# Final small-fix recheck (6cab6c1 → 4e4dc73) — scoped to R1/R2 only

Reviewer: MiniMax-M3. Date: 2026-10-29 (rev. 5). Read-only; no source/index/build/test activity, no
subagents, no new stage. Inputs: `run/forge-sound-stage/review-final-small-fix.txt` (diff 6cab6c1..4e4dc73),
the worktree report and `sound.rs` at `4e4dc73`, and `forge-multibyte-truncate-red.log`,
`forge-multibyte-truncate-green.log`, `forge-sound-fix3.log`. Unchanged source was not re-reviewed.

## Final verdict: **ACCEPTED** (no blockers; one cosmetic nit)

### R1 — multibyte-safe diagnostic truncation: **ADDRESSED**

`crates/eqoa/src/fork_ext/addons/sound.rs`, `record()` at `4e4dc73`:

```rust
const MAX_DIAGNOSTIC_BYTES: usize = 2048;
if text.len() > MAX_DIAGNOSTIC_BYTES {
    let mut cut = MAX_DIAGNOSTIC_BYTES;
    while cut > 0 && !text.is_char_boundary(cut) { cut -= 1; }
    text.truncate(cut);
    text.push_str("…[truncated]");
}
```

Correct: the walk-back stops on a char boundary, `cut > 0` guards the degenerate case, `truncate(cut)` is then
safe, and the constant is renamed to `…BYTES` so the unit matches the behaviour. No panic path remains.

Evidence is genuine pre-fix RED, not a reconstruction —
`forge-multibyte-truncate-red.log`:

```
panicked at crates\eqoa\src\fork_ext\addons\sound.rs:401:18:
assertion failed: self.is_char_boundary(new_len)
test … forge_sound_diagnostic_truncation_respects_multibyte_boundary_and_dedup ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1120 filtered out
```

i.e. exactly the naive `text.truncate(2048)` panic I predicted, with the byte-2048 boundary landing inside a
3-byte character. `forge-multibyte-truncate-green.log` shows the same test `1 passed; 0 failed` after the fix,
and `forge-sound-fix3.log` shows `running 23 tests` → `23 passed; 0 failed` (22 + the new multibyte test).

**Test specificity.** The new test builds a diagnostic whose byte 2048 provably falls inside a multi-byte
character (2034 ASCII bytes + the 13-byte `MCI command "` wrapper, then a 3-byte `あ`), asserts the length bound,
the `…[truncated]` suffix, and that two identical long diagnostics **deduplicate to one** entry after truncation
(the dedup compares the stored, truncated text, which is the intended semantics). Two of its assertions are
tautological for a Rust `String` — `text.is_char_boundary(text.len())` and
`std::str::from_utf8(text.as_bytes()).is_ok()` are always true — so they add no coverage; the real teeth are the
panic under the naive implementation (proven by the RED log) and the dedup/length checks. Harmless, worth knowing.
The test is platform-independent (no `cfg` gate needed) and therefore adds **no** new Linux exposure.

### R2 — report head documentation: **ADDRESSED**

`docs/FORGE-SOUND-REPORT.md:12-16` now states:

* **Base:** `ada9092`;
* **Full reviewed code commit:** `6cab6c1` (`fix(addons): review round 2 for Forge sound stage`);
* **Follow-up commit:** the R1/R2 change, with its hash explicitly delegated to the lead's final status.

This is the requested form: the exact reviewed commit is named, the follow-up is distinguished from it, and the
report does not try to contain its own hash (a commit cannot) — the "no self-reference" requirement is met.

**Cosmetic nit (non-blocking):** the Fixed/Deferred table's M7 row still reads "Capped at 2048 **chars**" while
the implementation and the "What Was Implemented" section now say 2048 **bytes**. One-word consistency edit.

## Known limitations carried forward (all documented in the report)

1. **No Linux execution** — recorded honestly; the `#[cfg(windows)]` / `#[cfg(not(windows))]` gating is now real
   and was verified by inspection, so the round-1 CI-breakage risk is closed, but Linux coverage is unproven.
2. **Historical RED for negative Lua IDs was never captured** — explicitly stated in the report; only the
   containment RED and the multibyte-truncation RED exist.
3. **Resolver duplication** (`package::resolve_resource` vs the sound resolver) — deferred by the lead's ruling;
   containment is enforced without unifying them.
4. **Per-VM numeric handle namespace** — a documented adaptation (each addon owns an isolated Lua state and bank).
5. **MCI close cannot guarantee OS release** — a failed close keeps the record counted and the bank makes a final
   attempt in `Drop`; the report states this explicitly.
6. **Audible playback is not proven** — MCI command success is evidence, not hearing; user listening remains the
   later manual gate (per the brief).
7. The report's follow-up commit hash is to be recorded by the lead in the final status.

**No blockers remain.** The sound stage is ready to hand to the lead for integration on the reviewed commit
`6cab6c1` plus the `4e4dc73` follow-up.
