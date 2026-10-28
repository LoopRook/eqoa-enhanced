# EQOA Enhanced

A native Rust client for **EverQuest Online Adventures: Frontiers**, extending
[DabDavis's client](https://github.com/DabDavis/eqoa-rust-client-vanilla) with map
search, guidance and quality-of-life features. This fork keeps upstream engine,
network and gameplay improvements while developing its enhanced features.

## Features

- World map and circular minimap with NPC, group and location search.
- NPC/player hover details, live sightings, recorded spawns, and waypoint/group
  guidance. Spawn status is confirmed only within data received from the server;
  **Not detected** does not mean dead or despawned.
- Quests, coaches, XP camps, community suggestions, remappable controls and
  reusable map presentation tokens.
- HUD enhancements, companion/addon entry points and native combat telemetry.
  The parser is an external companion, maintained separately.
- Upstream fishing, GM windows, character shadows and protocol improvements.
  Features requiring server support depend on the connected server.

## Run on Windows

You need a locally owned Frontiers disc image. No disc or decrypted game data is
distributed here. With Rust installed, build a complete map bundle:

```powershell
./tools/build-map-preview.ps1
```

Run **Play Map Preview.cmd** in the generated folder. It includes the pinned NPC
catalog and notices, and uses an isolated profile. A prepared checkout can also
use **Start-EQOA.bat**, which prefers target/release/eqoa.exe, then root eqoa.exe.
The historical NPC catalog must sit beside the executable. A source merge alone
does not rebuild installed executables.

## Development and status

Start with [development](docs/DEVELOPMENT.md) and [contributor guidelines](AGENTS.md).
See the [map handoff](docs/MAP-RELEASE-HANDOFF.md), [playtest checks](docs/MAP-PLAYTEST.md),
[roadmap](docs/MODDING-AND-PARSER-ROADMAP.md), and [upstream sync report](docs/UPSTREAM-SYNC.md).
Full Forge parity and production companion packaging remain unfinished.

## License

The repository currently carries the upstream **MIT** license; copyright and
third-party notices remain intact. No licensing change has been selected.
Compare the [license options](docs/LICENSE-OPTIONS.md) before deciding.
