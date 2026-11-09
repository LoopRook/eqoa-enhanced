# Round 2 implementation report

Branch: `feat/eqoa-api-r2`, from `develop` at `413d1ee`. Implementation is committed in stages; no push, merge, tag, published release or owner acceptance is claimed. The approved `EQOA-API-SPEC.md` remains authoritative. This report distinguishes automated evidence from live-game acceptance.

## Changes by module

| Area | Implementation |
|---|---|
| Native API and capabilities | `native.rs` installs `eqoa/1`, module imports, callable later-round stubs, guarded records, error tuples, capability queries and descriptors. Runtime availability remains separate from capability support. Unknown fields and unsupported calls diagnose even inside `pcall`. |
| Player, target, entities, group | `api_capture.rs` reads private Play state into the owned `native_core::View`. Native strings, signed stats, full entity IDs and complete lists are preserved. Opaque references include generation and incarnation. HP/effect presence is distinct from zero. Group leadership follows the current roster. Forge projections retain its fixed limits and conventions. |
| Abilities, hotbar, cooldowns | Canonical decimal i64 IDs, native carousel contents, seconds, server display keys and one-based positional slots. Event boundaries retain explicit and whole-record lockout edges with receipt-time identity. Pre-event capture stores only the relevant prior cooldown ID/value, not a complete view. Observed cooldowns track start/update time; entering with an already active lockout has no invented timestamp. |
| Effects | Actual effect state and names replace the default Forge snapshot. Before the first effect channel reply the native query reports unavailable; an explicit empty reply returns an empty list. |
| Chat and combat | Source logical chat journal preserves bursts beyond the eight-line presentation ring. Known group opcode context is retained; no native channel is guessed from red color. Combat preserves signed damage/heal meaning, optional parsed RGB including black, full IDs and player/pet direction. Forge payload shapes remain in adapters. Forge cooldown Update consumes native journal edges in production, with a compatibility fixture fallback. |
| Input | The live merged Report is captured before native rotation, including active-high named buttons, normalized axes and neutral raw bytes 128. No controller yields nil. Claims, suppression, text capture and rotation are scoped to a VM; disable/drop/focus/world cleanup revokes ownership. Forge owner names are scoped internally and presented without the scope prefix. |
| HUD and drawing | Opaque native claims share the existing HUD suppression service. Compound leaves and `hidden_by` use native names. Claims release on disable and world loss. ImGui remains the drawing API; native aliases use the same bridge. State/timer subscribers cannot draw outside render/settings callbacks. |
| Textures, icons, fonts, sound | Native loaders use the same banks and sandbox resolver as Forge. TextureInfo returns actual dimensions; native icon loads use the real icon cache. Mandatory bundled fonts and image assets are exercised from issued resource aliases and bare names. Deferred font completion publishes real outcomes, released fonts are revoked, default fonts remain immortal. Native sound reports backend rejection and excludes unsupported platforms. Resource completion events cover file/memory textures, fonts and uncached sound requests. |
| Settings | Native callbacks and explicit load/save use the manager's active addon/profile slot. Save succeeds after atomic replacement. Validation rejects non-finite values, functions, handles, metatables, cycles, sparse/mixed tables, depth overflow and oversized serialization. Forge callbacks retain precedence when native callbacks are absent. |
| Lifecycle and events | Both manifest entry modes work. Once-mode entry registers subscriptions; ordinary disable/re-enable retains globals but repeats registration. Initial load is separate from re-enable. Subscriptions support stable order, once, removal during dispatch, next-boundary timers and callback isolation. Heap exhaustion is terminal and entry/callbacks share the iteration instruction budget. |
| File sandbox | Round 2 includes section 9. Shared bundled resources and own package files are read-only backing stores; each mod has a private overlay and durable tombstones. Both slash forms and exact host-issued aliases resolve through the same service. Copy/remove/rename never mutate backing files or another mod. Rooted Windows operations hold directory handles and reject reparse paths; unsupported platforms reject operations. Writes are atomic and bounded. The per-addon aggregate includes private files, tombstones, default settings and values in every named profile. |
| Diagnostics and logging | One native sink deduplicates by VM/symbol/code, retains counts and timestamps, bounds new-key rate, emits overflow and final repeat summaries, and feeds the Addon Manager and rotating `mods/addons_error.log`. The manager shows warning counts and structured diagnostic details; log-write failure is visible in the manager. |
| Examples and docs | Bundled `eqoa_example_hearts` has `api: eqoa/1`, once-mode registration, real-health drawing, cleanup and profile callbacks, with no Forge imports. The bundled installer includes it. `GETTING-STARTED.md` explains authoring, conventions, errors, resources and persistence. Update notes mark the API experimental. |

## Hook footprint

The owner explicitly approved raising only the `play.rs` added-line budget from 755 to 759. These are the iteration, begin/end game-event and logical-text calls. The existing combat invocation now passes parsed RGB. The iteration signature supplies mutable Play to the private consumer so existing Forge chat/weapon requests drain on the 30 Hz game step instead of render frames; it remains one neutral invocation. Viewer captures live/pregame input through the approved neutral controls seam.

Measured against `8435f2c8a00c631d716771f8189fa0011a1fbf50`: `play.rs +759/-53`, `viewer.rs +255/-15`. The footprint checker passed at the last checkpoint. No other shared hook sites were added, no upstream algorithms were moved or rewritten, and no further budget changes were made.

## Verification

All Cargo commands use `C:/Users/hocka/Paseo/targets/eqoa-api-r2`, the shared mkdir lock with timeout, and a scratch `EQOA_CONFIG` under the system temporary directory. Tests use one thread. No game assets, credentials, run directory or Testing Resources content are committed.

| Gate | Result |
|---|---|
| Initial native registration negative control | Both initial tests failed before registration and passed after it. |
| Focused native behavior final gate | 40 reported passed, 0 failed, 0 ignored; 1 SKIP line from an unrelated disc-reader test selected by the broad `native` filter (39 exercised tests). Includes Play-owned source fixtures and actual live controls adapter, not just synthetic Lua records. |
| Combat/retention negative control | 32 reported passed, 2 expected failures, 1 unrelated disc-reader SKIP before implementation. The combat case is valid. The original retention assertion confused the dispatch cap with retention; it was corrected to the normative 1,024 events/2 MiB buffer and 256 dispatch budget. Corrected backlog/overflow/byte-cap cases pass. |
| Final contract controls | 32 reported passed, 6 expected failures, 1 unrelated disc-reader SKIP: disabling the corrected retention/byte cap and presentation hook fails three broker cases and the clock case; before their fixes, disabled Forge delivery and saved hearts layout also fail. Mutated production files restored automatically. The restored source/lifecycle suite passed 38/38, and the addon gate passed 314/314 selected nonignored tests. |
| Heap/context negative control | 34 reported passed, 2 expected failures, 1 unrelated disc-reader SKIP before implementation: nested pcall swallowed heap exhaustion, and a revoked packet capture leaked group-chat context. Both pass after the fixes. |
| Additional quota/Forge source negative control | 30 reported passed, 2 expected failures, 1 unrelated disc-reader SKIP before implementation. The restored/fixed native suite passed 32/32. Settings/file quota aggregation and Forge delivery of native whole-record cooldown edges now pass. |
| Deliberate negative controls | 23 reported passed, **8 expected failures**, 0 ignored, 1 unrelated disc-reader SKIP, Cargo exit 101. Restored all five production files automatically. Removing real effect projection, logical chat capture, live report capture, tick gating, VM queue cancellation, whole-record cooldown edges, shared instruction budgeting and fatal heap propagation each failed its corresponding behavior test. |
| Required addon suite | 316 passed, 0 failed, 23 ignored; 6 SKIP lines (four symlink privilege branches, screen-grid opt-in and pinned-hearts opt-in). |
| Pinned Forge reference opt-in | 24 passed, 0 failed, 0 ignored, no SKIP lines (`fork_ext:: --ignored`, including the legacy bundled update test). Uses the local approved reference tree. |
| Bundled installer | 23 passed, 0 failed, 1 ignored; no SKIP lines. The four old three-package assertions were updated to require the new fourth package; installation itself was correct. |
| wasm32 check | PASS: `cargo check --locked -p eqoa --target wasm32-unknown-unknown` (two pre-existing unused-variable warnings in report_core). |
| Footprint | PASS after final code changes; `git diff --check` also passed. |
| Pinned GPU opt-ins | Grid 1/1 and unchanged hearts 1/1 passed, no SKIP lines. Their old hard-coded `target/` screenshot paths initially failed after rendering; test output now honors `CARGO_TARGET_DIR`. Full/partial/death hearts pixels: 1,395 / 690 / 0. |
| Neutral fallback negative control | The new default-byte test failed at 127 before the fix; both Forge raw axis defaults now use 128, matching real reports and the approved convention. |
| Sound failure/retry | Negative control 0 passed / 1 expected failure; fixed sound suite 24 passed, 0 failed, no SKIP lines. Native release returns failed when backend close fails and permits a successful idempotent retry. |
| Release build after final commit | PASS: `cargo build --release --locked -p eqoa`. Initial isolated optimized build completed in 4m 12s with 56 warnings; the same command is rerun after the final report commit. |

The unchanged bundled Wereoxxs, ARPG and Modern UI scripts pass load, frame, Save/Load and disable fixtures. Tests exercise actual embedded Lua bytes, real fonts and the gear image, private migration of Modern UI's legacy files, tombstone persistence across VMs, slash aliases and neighbor isolation. The native hearts example draws through the managed GPU fixture and releases its health claim on disable/reload.

A passing test that prints SKIP proves no behavior for its skipped branch. Every broad `native` run, including its negative controls, selected the unrelated BGM disc-reader test and printed one `SKIP: no run/eqoa.iso`; the reported pass total includes that early return. Counts above were checked across stdout and stderr. The six SKIP lines in the plain required addon command are distinct from its 23 ignored tests. The two GPU opt-ins subsequently ran without SKIPs; four symlink branches remain unverified because Windows denied symlink privilege. Oracle/decomp parity tests cannot run on this machine. No live game, server play, audio listening, physical controller interaction or visual owner acceptance is claimed; the owner must live-test all three bundled UIs and the native example.

## Deferred scope and contract limits

Items, inventory, bank, loot, merchant, gems, quests, camera, map, mastery, dialogs/windows and general actions remain rounds 3-4. Native symbols are callable `not_yet` stubs with scheduled rounds and visible diagnostics. Forge behavior already present outside Round 2 remains compatibility behavior; it is not advertised as implemented native Round 3/4 support.

The former ImGui-budget deviation is resolved by the lead-approved section 9 amendment in the fix round below. The following still require explicit review before calling Round 2 spec-complete:

- The descriptor inventory is implementation-driven for many Forge members and ImGui methods. A full per-object/constant/field conformance audit remains Round 5 work; no claim of exhaustive descriptor parity is made.
- Event loss markers and reset envelopes are allocated at delivery time, while retained source envelopes retain their source sequence. Source order is preserved, but the synthetic marker's sequence can exceed the following retained event's sequence. Source and resource completions now share the specified 1,024-event/2 MiB per-VM retention limit; a boundary delivers at most 256 envelopes and preserves its backlog. Tests cover mixed/completion-only overflow and the byte cap.
- Secure rooted file operations are implemented for Windows; equivalent non-Windows rooted operations are not implemented. Their failure must be reported as unsupported; a native Linux run was not performed. wasm is a compilation gate, not a browser mod-runtime acceptance test.
- Post-event comparison still assembles a complete R2 view for coherent state publication. The unnecessary full pre-event view was removed. Throughput at high entity/packet rates is not live-profiled; the section 10 copy-cost target needs review before performance parity is claimed.
- The monotonic clock is initialized when the addon runtime is constructed at client startup. Presentation frames now advance through the existing live and pregame controls seam; game ticks advance only through the game-step seam.

These are reported limitations, not accepted design changes. The API stays experimental until independent review, fixes and the Round 5 audit. No stability or live-game parity promise is made here.

## Fix round

Review: [round-2-review.md](round-2-review.md), supplied by the lead. The approved play.rs +759 budget is unchanged. Review fixes are recorded below; the original verification above remains historical evidence for the pre-review checkpoint.

- H1/H2: restored pinned Forge populated semantics (nonzero ID and positive level) and low-byte target IDs (`& 0xff`); native IDs retain all 32 bits. Tests enter through the actual Play projection, including missing creature records.
- H3: `entities.named` accepts the optional boolean exact flag, defaults to exact, compares Unicode lowercase strings and performs literal substring matching when false. Wrong flag types fail `invalid_argument`.
- H1-H3 negative control: 0 passed / 3 expected failures before the fixes; 3 passed after, no ignored tests or SKIP lines.

- M1: accepted Forge chat remains queued during mode 60 and sends on the next eligible game step. World loss cancels the queue through a weak, owned diagnostic sink; no Lua executes in the native consumer, and the manager/error-log sink receives `cancelled` without message contents.
- M2: discovery rejects non-string, null and unknown `entryMode` values, records a typed mode, defaults existing/loose packages to frame, and the host uses recorded metadata. Editing mod.json after discovery cannot silently change the selected entry mode.
- M3: native file operations classify lexical resolver rejection before I/O, returning `permission_denied` for traversal and invalid boundary paths; explicit reparse denials have the same code. Missing resources remain `not_found` and actual I/O failures remain separate.
- M4: native binding preserves explicit per-function error tuples. Unexpected runtime/backend errors become `failed`; typed I/O errors preserve `not_found`, `permission_denied` or `io_error`; argument conversions remain `invalid_argument`, revoked userdata remains `stale_handle`, and fatal heap errors still propagate.
- M6: iteration readiness calls a scalar predicate shared with native capture instead of constructing a JSON view. A test-only counter verifies zero view builds at the iteration seam both in-world and during world loss.
- L2/L3: experience thresholds now use the requested level, with boundary/native level-51 coverage and author documentation. Capabilities prefixes match complete dotted segments, including exact leaf queries and the empty/all prefix.
- L6: amended the approved spec section 9 to 131,072 bridge calls/VM/ImGui frame, as explicitly requested by the lead. The 10,000-message Wereoxxs fixture's measured 30,388 calls, 536,000 instructions and 72,024 geometry units explain why 4,096 was incompatible with the bundled-UI gate.

Findings left unchanged after checking evidence:

- M5 is incorrect for the production managed path. `manager::prepare` calls `install_managed_health_adapter` after `Host::for_addon` has installed eqoa for every VM, including Forge manifests. That installer calls `install_native`, which registers the lease token/addon ID. The new two-VM test proves native `hidden_by(health/compass/hotbar/all)` sees the Forge ID, deduplicates arbitrary Forge owner labels and clears after disable. Removing registration makes this test fail; no production HUD change was required.
- L1: `native_state::distance` already uses `ee::sub/mul/add/sqrt`. Keep it: switching it to Forge's historical f64 arithmetic would violate the fork arithmetic rule; changing Forge would alter pinned compatibility semantics. The native implementation already meets the requested ee discipline.
- L4: section 11 explicitly requires nil plus per-field `unsupported` for decoder palette/format metadata that is not retained. Keep those diagnostics. Native guarded record misses already diagnose `unknown_symbol`; no fabricated metadata or relaxed diagnostic was introduced.
- L5: the spec fixes generation transitions, not an origin of zero for an entered world. `Core::default` reports zero before entry; first entry advances to one, then reset advances the generation. Keeping that behavior distinguishes never-entered from first-world handles. No change is warranted.

Review controls: H1-H3 were 0 pass / 3 expected failures before their fixes, then 3/3 passed. M2/M3/M4 and the readiness counter failed before their fixes; the M1 initial setup error was corrected and is not counted as behavior evidence. The corrected modal-send control, readiness control, removed HUD identity registration and unfixed L2/L3 yielded 6 passed / 5 expected failures, 0 ignored and no SKIP lines. Both temporarily mutated production files were restored automatically. All six MEDIUM cases passed before that mutation run; final combined gates follow below.

The restored combined review suite passed 11/11, 0 failed, 0 ignored and no SKIP lines. Footprint and diff whitespace checks pass; the play.rs/viewer.rs measured budgets remain +759/-53 and +255/-15.
