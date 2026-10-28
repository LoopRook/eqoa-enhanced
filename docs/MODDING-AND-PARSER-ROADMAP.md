# EQOA Enhanced Client & Parser Modding Roadmap

This document outlines the architecture, implementation phases, and technical blueprints for developing the **EQOA Enhanced Client** and integrating the **EQOA Combat Parser** as a first-class mod.

---

## Project Vision & Core Principles

1. **Keep the Fun Stuff (QoL):** Restore the complete world map (`M`), minimap (`N`), quest tracking/guide, and stat boxes that were removed from upstream `eqoa-rust-client-vanilla` in commit `6fc3ec6`.
2. **User-Friendly UI (No Command Lines):** All features and mods are configured through in-game graphical menus (`Options → Extended → UI` and `Options → Mods`).
3. **Frictionless Combat Parser Integration:** Eliminate the need for Windows Administrator permissions, packet sniffing (`pktmon`), and PINE memory scanning. The client directly streams rich combat events to the parser.
4. **Drag-and-Drop Mod Platform:** Provide a standard `mods/` directory where community members can drop asset, UI, sound, or data mods without compiling code.
5. **Clean Upstream Sync:** Maintain our enhanced features on a dedicated Git branch (`enhanced`) so improvements and engine optimizations from DabDavis can be merged with minimal conflict.

---

## Phased Implementation Roadmap

### Phase 1: Establish "EQOA Enhanced" & Restore QoL Features
**Goal:** Create the project branch, restore the deleted map/minimap/questguide systems, and verify clean compilation.

- [ ] **Branch Setup:** Branch `enhanced` off `master` (tracking `origin/master`).
- [ ] **Feature Restoration:**
  - Restore `crates/eqoa/src/map/` (world map `M`, markers, POIs, zoom/pan).
  - Restore `crates/eqoa/src/questguide/` (quest journal, tracker under minimap, quest markers).
  - Restore `crates/eqoa/src/statbox.rs` (on-screen stats HUD `B`).
  - Restore `crates/render/src/map.rs` and `crates/render/src/map.wgsl`.
- [ ] **Resolve Merge Points:** Re-integrate the 6 files modified during the upstream purge:
  - `crates/eqoa/src/options.rs`: Restore UI checkboxes under `Options > Extended > UI`.
  - `crates/eqoa/src/settings.rs`: Retain persistence for map/minimap/statbox toggles.
  - `crates/eqoa/src/viewer.rs` & `touch.rs`: Wire up hotkeys (`M`, `N`, `B`) and touch map gestures.
  - `crates/render/src/lib.rs` & `ui.rs`: Re-register map rendering passes.
- [ ] **Build & Verification:** Ensure cargo builds cleanly on Windows without warnings.

---

### Phase 2: Live Combat Telemetry (No-Admin Parser Hook)
**Goal:** Stream rich, decoded combat events directly from the Rust client over local IPC, eliminating all Administrator and PINE requirements in the Python parser.

#### Rust Client Changes:
- [ ] **Telemetry Broadcaster:** Implement a lightweight local socket broadcaster (UDP on `127.0.0.1:28015` or WebSocket):
  - Hook into `crates/eqoa/src/damage.rs` (`0x00DB` HP changes) and `crates/eqoa/src/play.rs` (`0x00B1` cast/swing events).
  - Broadcast payload format (JSON/binary): timestamp, actor, target, amount, ability/spell, is_pet.
- [ ] **In-Game Toggle:** Add `Options > Extended > UI > Combat Data Stream: [On / Off]` (default: On).

#### Python Parser Changes:
- [ ] In `C:\Users\hocka\Github Projects\EQOA Parser\eqoa_parser\net\sources.py`:
  - Implement `ClientSocketSource` that connects to `127.0.0.1:28015`.
  - Directly yield `(ts, Event)` objects into `eqoa_parser/pipeline.py`.
- [ ] **Verification:** Run `python -m eqoa_parser` as a standard non-Admin user and verify real-time combat tracking with instant mob names.

---

### Phase 3: The Drag-and-Drop `mods/` Engine & FrontiersForge Compatibility
**Goal:** Allow users to install community mods simply by unzipping files into a `mods/` folder, ensuring seamless compatibility with existing EQOA mods from [FrontiersForge](https://github.com/mmvest/FrontiersForge).

#### 1. Compatibility with Existing FrontiersForge Mods:
The dominant EQOA modding framework is **FrontiersForge** (`mmvest/FrontiersForge`), which powers existing community mods like `mini_map`, `world_map`, `wereoxxs_ui`, `arpg_ui`, `avoids_modern_ui`, and `retro_health_hearts`.
- **How FrontiersForge Works:** Lua scripts rendered via Dear ImGui (`UiForge`), accessing game data via a standard `frontiers_forge.*` Lua API (`Player`, `EntityList`, `Combat`, `Group`, `Inventory`, `Chat`, etc.).
- **Rust Client FrontiersForge Bridge:**
  - Embed a lightweight Lua runtime (e.g. `mlua`) and Dear ImGui rendering layer into the client.
  - Implement a native Rust compatibility shim that exposes the `frontiers_forge.*` Lua modules. Instead of reading raw PCSX2 EE emulator memory via pointer offsets, the shim returns clean native Rust client data.
  - **Drop-In Compatibility:** Existing FrontiersForge scripts can be dropped directly into `mods/<mod_name>/` or `scripts/` and execute as-is!
  - **Conversion Path for Advanced Mods:** For mods utilizing raw EE memory reads outside the standard `frontiers_forge` modules, provide a simple conversion guide and API documentation showing how to query the client's native data.

#### 2. Folder Structure Convention:
```text
eqoa/
├── eqoa.exe
├── mods/
│   ├── wereoxxs_ui/           <-- FrontiersForge Lua + ImGui mod
│   │   ├── mod.json
│   │   └── wereoxxs_ui.lua
│   ├── high_res_ui/           <-- Texture / asset replacement mod
│   │   ├── mod.json
│   │   └── textures/
│   └── custom_poi_maps/       <-- Data / markers mod
│       ├── mod.json
│       └── markers/
```

- [ ] **Mod Manifest (`mod.json`):** Standard metadata (name, author, version, description, entrypoint). If missing, generate a default entry from the folder or `.lua` name.
- [ ] **Asset Override Hook:** Modify file loaders (`crates/eqoa/src/` and `crates/esf/`) to inspect active mods in `mods/` before loading default assets from the disc or extracted bundles.
- [ ] **In-Game Mod Manager UI:**
  - Create a dedicated **Options → Mods** tab.
  - Dynamically scan `mods/`, list discovered mods, and provide interactive `[X]` toggles to activate/deactivate mods on the fly.
  - Persist active mod choices to `settings.opt`.

---

### Phase 4: Parser as a First-Class Mod
**Goal:** Package the parser directly as an installable mod in the `mods/` system.

- [ ] **Option A: Auto-Launched Companion Mod:**
  - Placing `mods/eqoa_parser/` installs the parser.
  - When the client starts with the mod enabled, it automatically launches the parser's compact overlay window in the background and closes it when the game exits.
- [ ] **Option B: Native In-Game Overlay (Long-term):**
  - Port `eqoa_parser/stats.py` calculation logic into a native Rust module (`crates/eqoa/src/meter.rs`).
  - Render a small, floating DPS/HPS bar directly inside the game's UI layer (`render::ui`), toggled by a hotkey like `F4`.

---

### Phase 5: Distribution & 1-Click Installer
**Goal:** Deliver an effortless installation experience for non-technical players.

- [ ] **Release Packaging:** Automate packaging into `EQOA-Enhanced-v<version>.zip` containing:
  - `eqoa.exe`
  - Pre-configured `mods/` folder with default QoL features.
  - `README.txt` with simple instructions.
- [ ] **1-Click Installer:** Build a lightweight Windows installer (using Inno Setup or WiX) that detects or asks for the user's game directory and installs/updates the executable and mods in one click.
- [ ] **Upstream Sync Procedure:** Document the command for future agent sessions to pull upstream fixes:
  ```bash
  git fetch origin master
  git merge origin/master
  ```
