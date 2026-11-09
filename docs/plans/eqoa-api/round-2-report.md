# Round 2 implementation report

Branch: `feat/eqoa-api-r2`, from `develop` at `413d1ee`. Implementation is committed in stages; no push, merge, tag, release or owner acceptance is claimed. The approved `EQOA-API-SPEC.md` remains authoritative. This report distinguishes automated evidence from live-game acceptance.

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
| Focused native behavior final gate | 40 passed, 0 failed, 0 ignored; no SKIP lines. Includes Play-owned source fixtures and actual live controls adapter, not just synthetic Lua records. |
| Combat/retention negative control | 32 passed, 2 expected failures before implementation. The combat case is valid. The original retention assertion confused the dispatch cap with retention; it was corrected to the normative 1,024 events/2 MiB buffer and 256 dispatch budget. Corrected backlog/overflow/byte-cap cases pass. |
| Final contract controls | 32 passed, 6 expected failures: disabling the corrected retention/byte cap and presentation hook fails three broker cases and the clock case; before their fixes, disabled Forge delivery and saved hearts layout also fail. Mutated production files restored automatically. The restored source/lifecycle suite passed 38/38, and the addon gate passed 314/314 selected nonignored tests. |
| Heap/context negative control | 34 passed, 2 expected failures before implementation: nested pcall swallowed heap exhaustion, and a revoked packet capture leaked group-chat context. Both pass after the fixes. |
| Additional quota/Forge source negative control | 30 passed, 2 expected failures before implementation. The restored/fixed native suite passed 32/32. Settings/file quota aggregation and Forge delivery of native whole-record cooldown edges now pass. |
| Deliberate negative controls | 23 passed, **8 expected failures**, 0 ignored, no SKIP lines, Cargo exit 101. Restored all five production files automatically. Removing real effect projection, logical chat capture, live report capture, tick gating, VM queue cancellation, whole-record cooldown edges, shared instruction budgeting and fatal heap propagation each failed its corresponding behavior test. |
| Required addon suite | 316 passed, 0 failed, 23 ignored; 6 SKIP lines (four symlink privilege branches, screen-grid opt-in and pinned-hearts opt-in). |
| Pinned Forge reference opt-in | 24 passed, 0 failed, 0 ignored, no SKIP lines (`fork_ext:: --ignored`, including the legacy bundled update test). Uses the local approved reference tree. |
| Bundled installer | 23 passed, 0 failed, 1 ignored; no SKIP lines. The four old three-package assertions were updated to require the new fourth package; installation itself was correct. |
| wasm32 check | PASS: `cargo check --locked -p eqoa --target wasm32-unknown-unknown` (two pre-existing unused-variable warnings in report_core). |
| Footprint | PASS after final code changes; `git diff --check` also passed. |
| Pinned GPU opt-ins | Grid 1/1 and unchanged hearts 1/1 passed, no SKIP lines. Their old hard-coded `target/` screenshot paths initially failed after rendering; test output now honors `CARGO_TARGET_DIR`. Full/partial/death hearts pixels: 1,395 / 690 / 0. |
| Neutral fallback negative control | The new default-byte test failed at 127 before the fix; both Forge raw axis defaults now use 128, matching real reports and the approved convention. |
| Sound failure/retry | Negative control 0 passed / 1 expected failure; fixed sound suite 24 passed, 0 failed, no SKIP lines. Native release returns failed when backend close fails and permits a successful idempotent retry. |
| Release build after final commit | Pending. |

The unchanged bundled Wereoxxs, ARPG and Modern UI scripts pass load, frame, Save/Load and disable fixtures. Tests exercise actual embedded Lua bytes, real fonts and the gear image, private migration of Modern UI's legacy files, tombstone persistence across VMs, slash aliases and neighbor isolation. The native hearts example draws through the managed GPU fixture and releases its health claim on disable/reload.

A passing test that prints SKIP proves no behavior for its skipped branch. The six SKIP lines in the plain required addon command are distinct from its 23 ignored tests. The two GPU opt-ins subsequently ran without SKIPs; four symlink branches remain unverified because Windows denied symlink privilege. Oracle/decomp parity tests cannot run on this machine. No live game, server play, audio listening, physical controller interaction or visual owner acceptance is claimed; the owner must live-test all three bundled UIs and the native example.

## Deferred scope and contract limits

Items, inventory, bank, loot, merchant, gems, quests, camera, map, mastery, dialogs/windows and general actions remain rounds 3-4. Native symbols are callable `not_yet` stubs with scheduled rounds and visible diagnostics. Forge behavior already present outside Round 2 remains compatibility behavior; it is not advertised as implemented native Round 3/4 support.

The following require explicit review before calling Round 2 spec-complete:

- The existing ImGui call budget remains 131,072 calls per VM/ImGui frame, preserving the bundled UI regression baseline. The opt-in 10,000-message Wereoxxs test measured 30,388 calls, 536,000 instructions and 72,024 geometry units. Section 9 lists 4,096 calls/invocation; this is an explicit limit deviation requiring lead/audit review. No lower limit was silently imposed on existing mods.
- The descriptor inventory is implementation-driven for many Forge members and ImGui methods. A full per-object/constant/field conformance audit remains Round 5 work; no claim of exhaustive descriptor parity is made.
- Event loss markers and reset envelopes are allocated at delivery time, while retained source envelopes retain their source sequence. Source order is preserved, but the synthetic marker's sequence can exceed the following retained event's sequence. Source and resource completions now share the specified 1,024-event/2 MiB per-VM retention limit; a boundary delivers at most 256 envelopes and preserves its backlog. Tests cover mixed/completion-only overflow and the byte cap.
- Secure rooted file operations are implemented for Windows; equivalent non-Windows rooted operations are not implemented. Their failure must be reported as unsupported; a native Linux run was not performed. wasm is a compilation gate, not a browser mod-runtime acceptance test.
- Post-event comparison still assembles a complete R2 view for coherent state publication. The unnecessary full pre-event view was removed. Throughput at high entity/packet rates is not live-profiled; the section 10 copy-cost target needs review before performance parity is claimed.
- The monotonic clock is initialized when the addon runtime is constructed at client startup. Presentation frames now advance through the existing live and pregame controls seam; game ticks advance only through the game-step seam.

These are reported limitations, not accepted design changes. The API stays experimental until independent review, fixes and the Round 5 audit. No stability or live-game parity promise is made here.
