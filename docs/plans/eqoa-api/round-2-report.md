# Round 2 implementation report

Branch: `feat/eqoa-api-r2`, from `develop` at `413d1ee`. Implementation is committed in stages; no push, merge, tag, release or owner acceptance is claimed. The approved `EQOA-API-SPEC.md` remains authoritative. This report distinguishes automated evidence from live-game acceptance.

## Changes by module

| Area | Implementation |
|---|---|
| Native API and capabilities | `native.rs` installs `eqoa/1`, module imports, callable later-round stubs, guarded records, error tuples, capability queries and descriptors. Runtime availability remains separate from capability support. Unknown fields and unsupported calls diagnose even inside `pcall`. |
| Player, target, entities, group | `api_capture.rs` reads private Play state into the owned `native_core::View`. Native strings, signed stats, full entity IDs and complete lists are preserved. Opaque references include generation and incarnation. HP/effect presence is distinct from zero. Group leadership follows the current roster. Forge projections retain its fixed limits and conventions. |
| Abilities, hotbar, cooldowns | Canonical decimal i64 IDs, native carousel contents, seconds, server display keys and one-based positional slots. Event boundaries retain explicit and whole-record lockout edges with receipt-time identity. Observed cooldowns track start/update time; entering with an already active lockout has no invented timestamp. |
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
| Focused native behavior checkpoint | 32 passed, 0 failed, 0 ignored; no SKIP lines. Includes Play-owned source fixtures and actual live controls adapter, not just synthetic Lua records. |
| Additional quota/Forge source negative control | 30 passed, 2 expected failures before implementation. The restored/fixed native suite passed 32/32. Settings/file quota aggregation and Forge delivery of native whole-record cooldown edges now pass. |
| Deliberate negative controls | 23 passed, **8 expected failures**, 0 ignored, no SKIP lines, Cargo exit 101. Restored all five production files automatically. Removing real effect projection, logical chat capture, live report capture, tick gating, VM queue cancellation, whole-record cooldown edges, shared instruction budgeting and fatal heap propagation each failed its corresponding behavior test. |
| Required addon suite | Committed checkpoint verified: 310 passed, 0 failed, 23 ignored; 6 SKIP lines. A final run after subsequent fixes remains pending. |
| Existing pinned Forge reference suite | Pending explicit opt-in against the local pinned checkout. |
| Bundled installer/source hooks | Final focused run pending. |
| wasm32 check | Pending. |
| Footprint | PASS at checkpoint; final repeat pending. |
| Release build after final commit | Pending. |

The unchanged bundled Wereoxxs, ARPG and Modern UI scripts pass load, frame, Save/Load and disable fixtures. Tests exercise actual embedded Lua bytes, real fonts and the gear image, private migration of Modern UI's legacy files, tombstone persistence across VMs, slash aliases and neighbor isolation. The native hearts example draws through the managed GPU fixture and releases its health claim on disable/reload.

A passing test that prints SKIP proves no behavior for its skipped branch. Final SKIP and ignored counts will be recorded after the final gates. Oracle/decomp parity tests cannot run on this machine. No live game, server play, audio listening, physical controller interaction or visual owner acceptance is claimed; the owner must live-test all three bundled UIs and the native example.

## Deferred scope and contract limits

Items, inventory, bank, loot, merchant, gems, quests, camera, map, mastery, dialogs/windows and general actions remain rounds 3-4. Native symbols are callable `not_yet` stubs with scheduled rounds and visible diagnostics. Forge behavior already present outside Round 2 remains compatibility behavior; it is not advertised as implemented native Round 3/4 support.

The following require explicit review before calling Round 2 spec-complete:

- The descriptor inventory is implementation-driven for many Forge members and ImGui methods. A full per-object/constant/field conformance audit remains Round 5 work; no claim of exhaustive descriptor parity is made.
- Event loss markers and reset envelopes are allocated at delivery time, while retained source envelopes retain their source sequence. Source order is preserved, but the synthetic marker's sequence can exceed the following retained event's sequence. Resource completions have a separate bounded staging queue, so the combined source/resource boundary needs the spec's single 256-event retention audit.
- Secure rooted file operations are implemented for Windows; equivalent non-Windows rooted operations are not implemented. Their failure must be reported as unsupported; a native Linux run was not performed. wasm is a compilation gate, not a browser mod-runtime acceptance test.
- The native monotonic clock begins with first core initialization, and the presentation frame count advances at Play iteration. Pregame-to-world elapsed/frame continuity needs owner/reviewer confirmation against the client-lifetime wording.

These are reported limitations, not accepted design changes. The API stays experimental until independent review, fixes and the Round 5 audit. No stability or live-game parity promise is made here.
