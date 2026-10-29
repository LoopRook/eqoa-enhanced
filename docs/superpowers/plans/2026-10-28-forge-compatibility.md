# Forge Compatibility: proposed integration and delivery plan

Status: user-approved compatibility initiative, 2026-10-28. Source research is complete; review the machine-readable API/mod matrix before closing Stage 0. No runtime behavior or compatibility success is claimed by this plan.
Owner request: support old FrontiersForge mods and new native-client mods by dropping their files into the mods folder. User approval includes the native minimap and render-scale steering recorded below.

## Workspace and evidence

- Paseo workspace: Forge Compatibility, wks_12cecc2d0898f767.
- Worktree: C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility.
- Branch: forge-compatibility, based on enhanced at 63cdc77d96ba00de720e50aaffb7b06897479b96.
- FrontiersForge source: [4026d5cebcede24943ece8dc34833e584f46cd90](https://github.com/mmvest/FrontiersForge/tree/4026d5cebcede24943ece8dc34833e584f46cd90).
- UiForge host source: [6f2d125cc686cbc882f8098a939cc03cc23a59a1](https://github.com/mmvest/User-Interface-Forge/tree/6f2d125cc686cbc882f8098a939cc03cc23a59a1).
- Source-only reference download: original checkout's ignored run/forge-compat-reference/, including the pinned UiForge submodule. No downloaded scripts or injectors were executed.
- Read FORK.md, CLAUDE.md, docs/DEVELOPMENT-FREEZE.md, docs/MODDING-AND-PARSER-ROADMAP.md, docs/MAP-RELEASE-HANDOFF.md, docs/FORK-OWNERSHIP.md, and docs/COMPANION-LAUNCH.md.

Current support is manifest discovery and a special companion launcher in crates/eqoa/src/fork_ext/mods.rs. There is no Lua host, ImGui integration, or general addon execution API. The generated starter README overstates support. The parser remains a separate companion with separate ownership.

The user request authorizes a new compatibility initiative; it does not reopen release or unrelated map/parser work.

## User approval and steering

- The native minimap remains the default client experience.
- Forge `world_map` and `mini_map` are capability comparisons and optional compatibility proof cases. Neither is a required port or replacement, and neither blocks generic addon-host completion. The native world map and minimap remain the client's maps.
- Keep compatibility reporting honest: show explicitly unsupported APIs and resulting partial/incompatible status for either Forge map to the user. Do not imply universal Forge parity or hide gaps because the addon host is complete.
- The required runtime target is FF_Example, non-map included mods, and the native mod-author API. Map capabilities may be explored as optional proof cases separately.
- Prioritize the existing render-scale bugfix before the Stage 1 Lua/render feasibility spike. Keep it scoped as a prerequisite bugfix, not a claim of Forge parity.

## Recommended approach and alternatives

Recommend an embedded Lua host, a real Dear ImGui overlay, and native implementations of the high-level frontiers_forge module APIs. Legacy addon entry scripts should remain unchanged wherever their semantics can be supplied faithfully. Native implementations replace the emulator-specific shared modules through the host's module resolver.

Alternatives considered:
1. Rewrite each mod as built-in Rust UI: useful for individual features, but does not deliver drop-in mods or a mod-author ecosystem.
2. Reproduce PCSX2 memory and MIPS hooks: much larger and fragile; the native client's state is not a running EE process.
3. Embed a Lua/ImGui compatibility host with native game-state adapters: best fit for both legacy scripts and new mods; recommended.

This is a recommendation for review, not a claim that an architecture has already been approved or implemented. The first technical gate chooses and pins the Lua and ImGui bindings after testing their actual compatibility with Rust 2024, WGPU 30, and the pinned UiForge calling conventions.

## Compatibility contract

Support two authoring paths on the same runtime:
- Legacy: ImGui and UiForge globals plus require("frontiers_forge.<module>").
- Native: a versioned eqoa API with documented capabilities and no emulator addresses. New packages declare runtime, API version, entry point, and requested capabilities in mod.json; legacy scripts require no new manifest.

Compatibility means matching return shapes, integer/boolean distinctions, nil behavior, enums, coordinate axes/units/headings, indexing, record identity, callbacks, resource paths, and event fan-out. For example, Util.IsInGame() must preserve the numeric convention used by scripts comparing it to 0.

Classify each inspected API as faithful native equivalent, documented adaptation, or unsupported. Unsupported actions must give an actionable capability error, never success or fabricated game values. Imports and enum tables needed during startup must still resolve for a declared partial-support milestone.

Reserve host-owned ImGui/UiForge/frontiers_forge/eqoa APIs. Do not execute the original PCSX2 shared modules or IDE type-hint stubs. Do not expose arbitrary native pointers or pretend that EEmem and MIPS code caves exist. Mods that directly use FFI, pointer arithmetic, or Win32 APIs need specifically reviewed adapters or author changes; universal compatibility is not promised.

## Expected player workflow

1. Open Mods Folder from the client.
2. Drop a loose .lua file or a complete package with its modules/resources into that folder.
3. Refresh the list and enable it. The list shows loaded, disabled, partial, incompatible, or failed, with a useful reason.
4. Settings and window placement survive restart; disable/reload reliably releases input, HUD visibility claims, event subscriptions, and resources.

Load these layouts without per-mod source edits:
- mods/example.lua.
- mods/example/example.lua, falling back to main.lua then init.lua.
- A copied FrontiersForge scripts tree at mods/FrontiersForge/scripts/, with shared modules/resources and its individual addons listed separately.

Do not discover modules, resources, profiles, or companion directories as executable Lua mods. Diagnose duplicate IDs/entry filenames rather than silently choosing a winner. Keep each package's modules and package.loaded state isolated; private modules/resources take precedence over shared paths except reserved host APIs.

Resolve the default mods directory beside the executable regardless of the current working directory; support an explicit EQOA_MODS_DIR override. Store writable profiles/settings under the client configuration directory, scoped per mod and client profile. Test read-only installations, paths with spaces, legacy Windows separators, and restart from an unrelated working directory.

## Runtime boundaries and implementation locations

Proposed fork-owned files under crates/eqoa/src/fork_ext/addons/:
- mod.rs: orchestration and plain-data interfaces.
- discovery.rs: validated manifests, roots, legacy entry precedence, dependency/collision diagnostics.
- host.rs: isolated Lua state, persistent globals, frame execution, callbacks, reload and budgets.
- contract.rs: API versions, capability states, snapshots, events and requested actions.
- forge/: small semantic adapters grouped by API; address-based helpers remain unsupported.
- resources.rs and persistence.rs: scoped handles, package lookup, plain-data profile round trips.
- tests/: pinned script fixtures, semantic assertions and failure controls.

App-private snapshot/input/HUD adapters belong in fork_ext/ under the relevant shared parent, following the current hook pattern. Runtime modules receive plain data; they do not depend on Play internals.

A proposed crates/render/src/addons.rs and shader own the overlay pass. Dear ImGui draw data needs triangles, texture handles, scissor rectangles and correct alpha; the existing PS2 quad layer alone is not a sufficient compatibility backend. Use physical client-area pixels as Forge coordinates and composite at display resolution after game render scaling; test DPI and resize explicitly.

Review the exact new ownership patterns and shared edits before implementation: Cargo.lock, relevant Cargo.toml files, main.rs, play.rs, viewer.rs, input routing, HUD visibility, and render integration. Existing seams do not cover the full addon lifecycle. Document new named hooks and reviewed footprint changes; do not raise budgets just to pass a checker. Fork notes belong in FORK-UPDATE-NOTES.md; UPDATE-NOTES.md remains upstream-identical.

Native snapshots must be consistent for a frame. Entity/item/window handles must become invalid on despawn, logout, reconnect, or zoning. Chat/combat/cooldown events originate from native game events, are queued once, and are delivered independently to all subscribers; repeated legacy Update calls cannot drain another mod's events. Do not use the parser's UDP receiver as the in-process addon bus, or advertise the existing hard-coded melee telemetry label as complete attribution.

## Delivery sequence and model assignments

The lead owns interfaces, sequencing, integration and evidence. Keep at most three research/implementation workers active, with one writer per shared file. Reuse workers for related follow-ups. No Claude workers.

| Stage | Owner | Work | Reviewable deliverable / exit condition |
|---|---|---|---|
| 0. Contract inventory | Terra for ff_example; Flash 3.8 for all included mods; Luna for native inventory; lead reviews | Pin sources, enumerate calls and semantics, map native equivalents and gaps | Source research complete; machine-readable API/mod matrix plus source links still due; every used API classified; no unverified parity claims |
| 0.5. Render-scale bugfix | Lead assigns; implementer TBD | Fix the existing render-scale defect before addon-renderer feasibility work | Focused regression evidence and deterministic capture at affected scale; scope remains independent of Forge parity |
| 1. Lua/render feasibility | Lead or Sol | Small host/backend spike using actual UiForge binding semantics | Unchanged screen_grid renders; representative multireturn widget, texture, mouse/keyboard capture, resize/DPI/render scale and deterministic shot work; dependency/engine decisions recorded |
| 2. Drop-in loading and lifecycle | Luna, with fixed contract | Loose/package/tree discovery, manifests, status UI, settings, reload, isolation and errors | Drop/refresh/enable/restart works from a clean package; companion behavior retained; positive and negative discovery/persistence tests |
| 3. First FF_Example slice | Terra for individual read-only adapters; lead/Sol for host integration | Resolve all 21 imports and required enums; implement unconditional startup/frame paths, real player/entity/camera data and basic lifecycle/profile behavior | Unchanged ff_example entry script loads, draws, saves/loads and shows real supported panels; partial capability report identifies the remaining sections |
| 4. Small complete mods | Terra for HP/HUD adapter; Luna for settings/resource fixtures; Flash verifies | screen_grid and retro_health_hearts, including legacy texture loading aliases and owner-scoped HUD hiding | Both run unchanged together; HP changes correctly; settings persist; disabling one restores only its own claims |
| 5. Full FF_Example | Terra in one-module tasks; lead/Sol for events, UI/input and cleanup | Abilities/hotbar/cooldowns, effects/icons, inventory/items, merchants/loot, quests/groups, dialogs/windows, chat/combat and native actions | Every expanded section and action has a faithful result or an explicitly accepted adaptation; any unsupported required feature blocks a full-compatibility claim |
| 6. Included packages | Flash inventories/verifies; Terra handles constrained adapters; lead/Sol handles complex seams | Required: uiforge_example; avoids_modern_ui, arpg_ui, wereoxxs_ui. Optional map proof cases: world_map and mini_map | Required non-map packages receive per-mod evidence with unchanged/adapted/incompatible status, behavior checks, settings and resource persistence. If assessed, each map gets a transparent capability report; no map port or parity result blocks generic host completion |
| 7. Author SDK and player package | Luna drafts; lead/Sol reviews; Flash verifies clean package | Native API docs, manifest schema, sample addon, migration guide, licenses and complete preview package | New sample mod works by copying it into mods; legacy support/version matrix ships with the runtime; no build tools, emulator or injector needed by players |

Stages 1 and 2 precede production adapters. After shared interfaces are fixed, small adapters can run in parallel. Stage 6 may start API inventory earlier, but its acceptance depends on the corresponding Stage 5 capabilities.

Terra tasks should name one module, exact signatures and fixtures. Luna owns bounded discovery/docs/status work. Flash owns repetitive source classification and acceptance checklists. Sol is reserved for Lua/ImGui semantics, render/input/event boundaries and final review; the lead integrates. Model capability never substitutes for recorded test evidence.

## FF_Example-specific acceptance

The pinned scripts/ff_example.lua is 1,648 lines and directly imports 21 modules:
util, ui, player, entity, entity_list, input, camera, chat, ability, ability_list, ability_bar, group, combat, quest_log, icon, effects, inventory, item, merchant, window and loot.

All imports precede drawing. Before drawing, the unchanged script also subscribes to chat/combat, acquires icon ownership through Icon.Acquire, and updates cooldown/input/chat/combat even while collapsed. Stage 3 must supply functional registration, ownership and update plumbing for these unconditional paths; an unsupported-operation exception there would prevent the supported panels from rendering. Report incomplete event/data coverage explicitly in capabilities. If these paths cannot yet run, use a reduced fixture and label it as such rather than claiming the unchanged example loads. Startup alone cannot establish full compatibility.

Implement its 27 direct ImGui function names against the actual C++ binding overloads/defaults, including InputText's string-plus-entered returns, RadioButton selection returns, Begin/End, tree and wrap stacks, image handles, tooltips and input focus. The autocomplete stub is a reference list, not executable bindings or complete authority.

UiForge callbacks used are DisableScript, Save, Load and OnEject. The example registers DisableScript twice; the pinned host replaces the previous callback. Preserve last-registration-wins semantics and separately perform unconditional host-owned cleanup of all resources/claims. Do not change the legacy API to callback fan-out accidentally.

Test login absent, live player, target/pet, empty/populated group/inventory, merchant/loot open and closed, quests, cooldown lifecycle, buff expiration, chat text/send, damage/healing attribution, zoning and logout. Check custom Enter capture does not swallow normal chat or game-dialog input after disable/failure. Live chat tests use a local fixture/server; do not send messages to other people as part of automated validation.

## Included-mod rollout and special cases

- screen_grid: draw-list geometry, display dimensions and settings/profile baseline. It still imports util, so use the native compatibility module.
- retro_health_hearts: real HP/MaxHP, shared PNGs, old UiForge.IGraphicsApi texture aliases, and reference-counted HUD claims.
- uiforge_example: broad host API test for packaged modules/resources, widgets, drawing, fonts, audio and lifecycle; not just a window smoke test.
- avoids_modern_ui, arpg_ui and wereoxxs_ui: qualify each independently against data, input, cooldown, effects, chat and combat requirements; rendering alone is insufficient.
- world_map: optional capability comparison/proof case, not a required port, replacement or completion blocker. If assessed, report resources, catalog, search, journal/custom-location persistence, coordinates and tracking capabilities, preserving live versus historical data meaning and exposing unsupported features.
- mini_map: optional capability comparison/proof case, not a required port, replacement or completion blocker. Its private map_render.lua, surface.lua and world_geometry.lua use FFI buffers, Win32 timing and EE memory traversal. Replacing shared Forge modules alone does not make it compatible. If assessed or ported, provide specifically versioned native private-module adapters or a documented maintained port and label that adaptation accurately. Keep unsupported capabilities visible in compatibility results; verify caves/ceilings, not only outdoors.

For the required end goal, FF_Example, every required non-map included mod, and the native author API need acceptance results. A partial runtime or the first example does not close the initiative. Forge world_map and mini_map results remain optional proof cases and must not be presented as universal parity. Preserve MIT notices and audit separate art/font/audio provenance before bundling assets; for example retro_health_hearts credits an external source image.

## Tests and release gates

Per task: add behavior tests with negative controls, observe failure before implementation where meaningful, run focused tests, format touched Rust and review the result. Do not write tests that merely restate wrappers.

Required runtime controls:
- Two mods with the same private module name remain isolated.
- A failing mod leaves the other rendering. Recover or reset ImGui Begin/End and Push/Pop state, even after a callback error.
- Instruction/memory/resource limits stop runaway mods; the spike must prove the chosen Lua engine can enforce the promised limits. Do not call an in-process runtime a strong security boundary.
- Missing files, malformed manifests/profiles, unsupported API calls and failed reloads produce actionable diagnostics.
- Disable, reload, load failure, callback failure, logout and client shutdown release textures/fonts/sounds, subscriptions, input capture and HUD claims.
- Frame/update semantics hold at 30 and high refresh rates; no duplicated events/cooldown advancement.
- Empty mods folder leaves existing deterministic game captures unchanged.
- Windows paths, unrelated working directory, resize/DPI, focus, multibox profile isolation and render scaling work from a packaged build.
- Linux/macOS compile and browser build remain intact. Initial runtime acceptance is native Windows; other-platform runtime support is recorded separately.

Validation gates during implementation:
- cargo test --locked -p eqoa <focused_addon_test_filter>
- cargo test --locked --workspace
- cargo build --release --locked -p eqoa
- python tools/check_fork_footprint.py (with the reviewed base/policy)
- sh web/build.sh with the repository's required WASM tooling
- Deterministic shot captures for runtime acceptance. Matched Forge map screenshots are optional evidence only when a map proof case is pursued; they are not generic host completion gates.
- Clean ZIP playthrough using scratch EQOA_CONFIG and a locally owned disc/local test server. Record prerequisite skips explicitly.

A compiled executable, resolved imports, a populated list, or passing synthetic tests cannot replace the live package acceptance. No release, push to master, automatic download of third-party mods, or parser-project changes are part of this planning work.

## Deliverables expected at completion

1. A tested native Lua/ImGui mod runtime with installation/status/settings/reload support.
2. Versioned Forge compatibility modules and a pinned API/mod support matrix.
3. FF_Example acceptance evidence for every section and action.
4. An included-mod results bundle distinguishing unchanged scripts, adaptations and unresolved incompatibilities.
5. A native mod-author guide, schema, runnable example and legacy migration guide.
6. A complete preview package, dependency/license inventory and reproducible verification report.

Research completed this turn: Luna native inventory, Terra FF_Example contract, Flash included-mod/FFI matrix, and Sol architecture review. Source inspection and Git/worktree checks were performed. No Rust build, runtime tests, Forge script execution or visual parity verification has been performed for the proposed host.
