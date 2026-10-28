# EQOA Enhanced Client & Parser Modding Roadmap

This document tracks the architecture, implementation phases, and deliverables for the **EQOA Enhanced Client** and the companion **EQOA Combat Parser**.

---

## Project Status: All Core Deliverables Completed

| Phase | Description | Status | Commit / Reference |
| :--- | :--- | :---: | :--- |
| **Phase 1: QoL Restoration** | Restored World Map (`M`), Minimap (`N`), Quest Tracker, Quest Markers, Stat Box (`B`), and resolved upstream merge conflicts with render scale and HUD filtering. | **DONE** | Commit `bd50b49` |
| **Phase 2: Live Combat Telemetry** | Non-blocking UDP telemetry broadcast from client (`127.0.0.1:28015`) and `ClientSocketSource` in EQOA Parser. Completely eliminates Windows Admin elevation, packet sniffing, and PINE memory scanning. | **DONE** | Client `2007dad`<br>Parser `0dd87f4` |
| **Phase 3: Drag-and-Drop Mod System & UI Enhancements** | Created `crates/eqoa/src/mods.rs` and `./mods/` engine. Added in-game **Options → Extended → Mods & Addons** menu. Implemented **Target HP %**, **XP Details**, and **HUD Numbers** overlays. | **DONE** | Commit `5e64004` |
| **Phase 4: Parser as a First-Class Mod** | Built `./mods/eqoa_parser/mod.json` companion descriptor. Added in-game 1-button launch and explorer folder shortcut. | **DONE** | Commit `5e64004` |
| **Phase 5: Release Binaries & 1-Click Launchers** | Compiled standalone optimized `target/release/eqoa.exe` (22.9 MB). Created `Start-EQOA.bat` and `Start-EQOA-With-Parser.bat` for zero-configuration, non-coder execution. | **DONE** | Commit `c2ac4aa` |

---

## System Architecture

```mermaid
flowchart TD
    subgraph Client ["EQOA Enhanced Client (eqoa.exe)"]
        Engine["Core 3D Engine (Upstream Master)"]
        QoL["Restored QoL (Map, Minimap, Quests, Stats)"]
        UIEnhancements["Enhanced UI (Target HP %, XP Details, HUD Numbers)"]
        Menu["Options > Extended > Mods & Addons"]
        ModEngine["Mods Engine (crates/eqoa/src/mods.rs)"]
        Telemetry["UDP Combat Telemetry (127.0.0.1:28015)"]
    end

    subgraph ModsDir ["mods/ Directory"]
        Readme["mods/README.txt"]
        ParserMod["mods/eqoa_parser/mod.json"]
        UserMods["mods/<custom_mod>/"]
    end

    subgraph ParserApp ["EQOA Combat Parser Companion"]
        ClientSource["ClientSocketSource (eqoa_parser/net/sources.py)"]
        StatsEngine["Fight Segmenter & Stats Engine"]
        MeterUI["Live DPS/HPS Meter & Details Window"]
    end

    Engine --> QoL
    Engine --> UIEnhancements
    Menu --> ModEngine
    ModsDir --> ModEngine
    Engine --> Telemetry
    Telemetry -->|Direct UDP (No Admin Required)| ClientSource
    ClientSource --> StatsEngine
    StatsEngine --> MeterUI
```

---

## Detailed Deliverables Summary

### 1. Enhanced In-Game Options (`Options → Extended → Mods & Addons`)
Users can access all new features directly in the graphical options menu:
* **Target HP %:** Shows exact percentage (e.g. `100%`, `45%`) beside the target gauge with colored status and shadow outline.
* **XP Details:** Shows current XP, total XP needed for next level, and exact percentage (e.g. `14,250 / 22,000 (64.7%)`) centered over the XP gauge.
* **HUD Numbers:** Overlays exact numerical percentages and values directly on player HP and Power gauges.
* **Open Mods Folder:** One click opens the `mods/` folder in Windows Explorer so users can simply drag and drop mods in.
* **Combat Parser:** Displays `Launch` or `Running` status, allowing the player to spawn the companion meter with a single button press.

### 2. Zero-Admin Combat Meter Hook
* **Rust Client:** Broadcasts structured combat events (attacker, target, damage amount, ability, pet status) over local UDP `127.0.0.1:28015`.
* **Python Parser:** Uses `ClientSocketSource` via `python -m eqoa_parser.app --client`. No `pktmon`, no Npcap, no PINE memory scanning, and zero Administrator elevation required. Full test suite passing (1,023 tests).

### 3. Drag-and-Drop Mod Structure
```text
c:\Users\hocka\Github Projects\EQOA Native Rust\
├── eqoa.exe                        <-- Pre-compiled standalone release executable (22.9 MB)
├── Start-EQOA.bat                  <-- 1-click game launcher
├── Start-EQOA-With-Parser.bat      <-- 1-click game + live combat parser launcher
└── mods/
    ├── README.txt                  <-- Instructions for community modding
    └── eqoa_parser/
        └── mod.json                <-- Combat parser companion descriptor
```

---

## Upstream Synchronization Guide
To pull in future engine updates, bug fixes, or optimizations from DabDavis:
```bash
git checkout enhanced
git fetch origin master
git merge origin/master
```
All enhanced features (options, QoL map/minimap, HUD text, mods engine, telemetry socket) are decoupled and modularized to minimize merge friction.
