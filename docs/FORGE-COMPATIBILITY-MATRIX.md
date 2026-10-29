# FrontiersForge Compatibility Matrix & Assessment

This document accompanies [`docs/forge-compatibility-matrix.json`](forge-compatibility-matrix.json) (`schemaVersion: 1`). It defines the dependency baseline, static classification, and acceptance requirements for drop-in legacy FrontiersForge mod compatibility and the planned native mod author contract in the EQOA Enhanced client.

---

## 1. Upstream Pinned Revisions & Provenance

The compatibility matrix was generated from read-only inspection of the authoritative source reference located at `run/forge-compat-reference`:

- **FrontiersForge**: [`4026d5cebcede24943ece8dc34833e584f46cd90`](https://github.com/mmvest/FrontiersForge/tree/4026d5cebcede24943ece8dc34833e584f46cd90) (Tag `v0.3.2`, commit date: `2026-08-13 21:19:50 -0400`).
- **UiForge Submodule**: [`6f2d125cc686cbc882f8098a939cc03cc23a59a1`](https://github.com/mmvest/User-Interface-Forge/tree/6f2d125cc686cbc882f8098a939cc03cc23a59a1).
- **sol2_ImGui_Bindings Submodule**: [`ec6f9ef7bab73e60f7ed32ef1e95dbee3b7b348e`](https://github.com/mmvest/sol2_ImGui_Bindings/tree/ec6f9ef7bab73e60f7ed32ef1e95dbee3b7b348e).
- **License**: MIT License (Copyright 2025–2026 mmvest / wereox). Distributed under [`docs/licenses/FrontiersForge-MIT.txt`](licenses/FrontiersForge-MIT.txt) and bundled in release packages.

---

## 2. Analysis Methodology: Lexical Extraction vs. Runtime Semantics

> [!CAUTION]
> **Discovered function and import names do not prove semantic compatibility.**

### Extraction Methodology
The dependencies in [`docs/forge-compatibility-matrix.json`](forge-compatibility-matrix.json) were produced using **lexical regex token extraction** over Lua source files combined with manual code review. An AST parser was **not** used. The scanner detects:
- `require("frontiers_forge.<name>")` and relative module requires.
- `ImGui.<method>` and `UiForge.<method>` invocations.
- LuaJIT `ffi` usages (`ffi.cdef`, `ffi.cast`, `ffi.new`, `ffi.string`, Win32 API declarations, and references to emulated PS2 Emotion Engine RAM `EEmem`).
- Resource path literals ending in image, audio, or JSON extensions.

A standalone reproducible scanner script is provided at [`docs/scan_forge_compatibility.py`](scan_forge_compatibility.py):
```sh
python docs/scan_forge_compatibility.py "run/forge-compat-reference"
```

### Limits & Semantic Distinctions
Syntactic presence of a symbol does not guarantee runtime functional compatibility:
1. **Memory Addressing vs. Native State**: In FrontiersForge on PCSX2, modules such as `player.lua`, `entity.lua`, and `inventory.lua` read raw byte offsets relative to emulated PS2 RAM (`ffi.C.EEmem`). In the native client, game logic resides in native Rust structures ([`crates/vi`](../crates/vi)). Exposing a matching function name (`Player.GetHealth`) requires mapping to native entity state, validating value ranges, units, and handling null references safely.
2. **Code Caves vs. Event Subscriptions**: Modules such as `combat.lua` and `chat.lua` patch MIPS instructions in emulated PS2 memory using dynamic code caves (`cave.lua`). In the native client, these must be replaced with event publisher/subscriber streams backed by native hooks ([`crates/eqoa/src/fork_ext`](../crates/eqoa/src/fork_ext)).
3. **Execution Model**: UiForge executes top-level script files once per render frame. Scripts rely on `require` table caching to preserve module state across frames. The native Lua runner must match this lifecycle.

**Native Baseline Status**: All 9 cataloged mods are explicitly marked as `"nativeBaselineStatus": "unimplemented"`. No mod is represented as functioning until verified against the strengthened acceptance criteria.

---

## 3. Subsystem Defaults: Native Maps Maintained

- **Both Native World Map and Minimap Stay in Place**: The native world map (with search and guidance) and native circular north-up HUD minimap delivered in 0.1.0 remain the active, supported game client subsystems.
- **Forge Maps are Optional Compatibility Proof Cases**:
  - Neither Forge `world_map` nor Forge `mini_map` is a required port, replacement, or blocker for the client.
  - They serve strictly as optional proof cases for evaluating legacy script rendering and data loading.
  - **Active Rollout Focus**: Compatibility work is focused strictly on **non-map addons** (HUD, frames, hotbars, combat telemetry, utilities).

---

## 4. Included Mod Rollout & Classification

The 9 included mods from the pinned reference are classified into focused non-map rollout phases, followed by the optional map proof cases:

### Rollout Order (Non-Map Focus)
1. **Phase 0: Diagnostic Benchmark**
   - **`ff_example`**: Living test harness. Directly imports 21 `frontiers_forge.*` modules (4 modules—`bank`, `cave`, `gems`, and `quest`—are not directly required). Used as the living contract for the bridge API surface.
2. **Phase 1: Pure Rendering & Host Validation**
   - **`screen_grid`**: Alignment grid overlay using ImGui background draw list (`dl:AddLine`). No game memory dependencies.
   - **`uiforge_example`**: Validates packaged directory structure (`modules/click_tally.lua`), texture loading (`gear-icon.png`), and full `UiForge` lifecycle callbacks (`Settings`, `Save`, `Load`, `DisableScript`).
3. **Phase 2: Simple Data & HUD Suppression**
   - **`retro_health_hearts`**: Reads player HP/MaxHP and suppresses native health bar via `UI.DisableHealthBar()`.
4. **Phase 3: Complex Resource HUDs**
   - **`arpg_ui`**: Liquid health/power orbs, 10-slot ability tray, buff/debuff rows, and 20+ bundled PNG textures.
5. **Phase 4: Event-Driven UI Suites**
   - **`avoids_modern_ui`**: 12 private sub-modules (health, mana, ability bar, group frames, quest log, chat).
   - **`wereoxxs_ui`**: Total conversion UI suite (ability bars, casting bar, party frames, damage meter, compass). Requires native combat telemetry and chat event dispatching.

### Optional Map Proof Cases (Subsystems Unchanged)
- **`world_map`** *(Optional Proof Case)*: Standalone Tunaria/Odus viewer with `entity_database.json` search and custom waypoints. Evaluates multi-MB JPEG loading. Native world map remains active default.
- **`mini_map`** *(Optional Proof Case)*: Software rasterized radar with height slicing. Contains heavy direct FFI dependencies on raw PCSX2 EE RAM (see Section 5). Native minimap remains active default.

---

## 5. `mini_map` Direct FFI Incompatibility Analysis

Static inspection of [`scripts/mini_map/modules/`](https://github.com/mmvest/FrontiersForge/tree/4026d5cebcede24943ece8dc34833e584f46cd90/scripts/mini_map/modules) reveals heavy reliance on raw PCSX2 memory structures:

1. **[`surface.lua`](https://github.com/mmvest/FrontiersForge/blob/4026d5cebcede24943ece8dc34833e584f46cd90/scripts/mini_map/modules/surface.lua)**:
   - Reads the PS2 `VIRaster` structure at absolute guest address `0x1FAFAB0` (surface table `+0x4BBC`).
   - Uses `ffi.cast` against `Util.EEmem() + guest_addr` to decode raw PS2 CLUT palettes and mipmapped surface buffers into RGBA8.
2. **[`world_geometry.lua`](https://github.com/mmvest/FrontiersForge/blob/4026d5cebcede24943ece8dc34833e584f46cd90/scripts/mini_map/modules/world_geometry.lua)**:
   - Traverses the live scene graph in guest RAM relative to client singleton `0x1FAF730`:
     - `VIScene`: `0x1FB6CC0` (singleton + `0x7590`)
     - `VICollide`: `0x1FB5890` (singleton + `0x6160`)
     - `VIRaster`: `0x1FAFAB0` (singleton + `0x380`)
   - Reads `VIZone`, `VIZoneRoom`, `VIActor`, and `VIPrimBuffer` structs directly via FFI pointer arithmetic.
   - Decodes 16-bit fixed-point vertex coordinates and polygon strips into dynamic `MapTri` FFI arrays.
3. **[`map_render.lua`](https://github.com/mmvest/FrontiersForge/blob/4026d5cebcede24943ece8dc34833e584f46cd90/scripts/mini_map/modules/map_render.lua)**:
   - Implements a CPU software triangle rasterizer allocating `uint32_t` framebuffers and `float` depth buffers via `ffi.new`.
   - Directly links to Win32 `QueryPerformanceFrequency` and `QueryPerformanceCounter` via FFI.
   - Pushes rendered frames to UI using `UiForge.CreateTextureFromMemory`.

**Conclusion**: Guest address `0x1FAF730` does not exist in the native Rust client's host address space. Running Forge's `mini_map` would require either generating a 32MB emulated EE RAM mirror containing ported C++ structs or substituting native geometry streams from [`crates/vi/src/world.rs`](../crates/vi/src/world.rs). Because native maps remain active defaults, this remains an optional proof case and does not block non-map addons.

---

## 6. Strengthened Acceptance Criteria

Rendering without error is insufficient. Acceptance requires rigorous behavioral verification:

| Mod ID | Acceptance Requirements |
| :--- | :--- |
| **`ff_example`** | **Data Validation**: Player name, level, HP/power, coordinates, active target, and entity list entries populate with correct live values.<br>**Action Dispatch**: Text entered in chat box transmits; UI toggles hide/show game elements.<br>**Event Reception**: Combat damage/heals stream into buffer; incoming chat lines append.<br>**State Lifecycle**: Profile save captures settings; reload restores state; disabling script unhooks listeners cleanly.<br>**Negative Controls**: Validated graceful degradation (no crash or hang) when not logged into game or when dereferencing nil entities. |
| **`screen_grid`** | Grid lines render across full client resolution; sliders adjust spacing and thickness in real time; centers color correctly; unhooks cleanly on script disable. |
| **`uiforge_example`** | Package-private module `click_tally` increments persistently across frames; `gear-icon.png` displays; bouncing balls simulate physics with interactive inputs; profile save/load restores ball positions; audio functions handle missing files gracefully. |
| **`retro_health_hearts`** | Native health bar is suppressed while active; heart count and fill percentages scale dynamically with current and maximum HP; damage/heal events update fills immediately; native health bar restores on disable. |
| **`arpg_ui`** | All 20+ PNG assets load without placeholder errors; liquid orb fills dynamically track live player health/power depletion and regeneration; ability icons display with cooldown shading; nameplates track target entity. |
| **`avoids_modern_ui`** | All 12 private sub-modules toggle independently in settings without cross-contamination; group frames display live party member names and health; chat tabs filter channel messages; quest log renders active quests. |
| **`wereoxxs_ui`** | Live combat telemetry feeds damage meter with accurate DPS calculation and attribution; casting bar syncs with spell windup/channeling; compass tape rotates with player yaw; window grid-snapping and locks persist across restarts. |
| **`world_map`** *(Optional)* | If toggled, renders Tunaria map image with player pin reflecting current coordinates without impacting native map, guidance, or client frame rate. |
| **`mini_map`** *(Optional)* | Capability assessment only: documents direct FFI memory barrier. Not required for native client release. |

---

## 7. Native Mod Author Contract Principles

1. **Clean High-Level API**: Native mods must interact solely through high-level `UiForge` and `frontiers_forge.*` interfaces.
2. **Prohibition of Raw Pointer Dereferencing**: The `EEmem` global and direct guest address casting (`0x1FA...`) are prohibited for native mods.
3. **Event-Driven Telemetry**: Combat events, chat messages, and ability lockouts are delivered via event callbacks rather than MIPS memory patching or memory ring polling.
4. **Self-Contained Packages**: Addons reside in `mods/<name>/` with assets placed under `resources/` and loaded via portable relative paths (`UiForge.LoadTexture("icon.png")`).
