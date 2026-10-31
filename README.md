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
  The parser is an external companion, maintained separately; see
  [companion setup](docs/COMPANION-LAUNCH.md).
- Upstream fishing, GM windows, character shadows and protocol improvements.
  Features requiring server support depend on the connected server.

## Play on Windows

**[Download EQOA Enhanced 0.2.0 for Windows x64](https://github.com/LoopRook/eqoa-enhanced/releases/download/v0.2.0/EQOA-Enhanced-0.2.0-Windows-x64.zip)**


1. Extract the prepared **EQOA Enhanced Windows build** into a folder.
2. Double-click **eqoa.exe**. Preview builds may give the executable a versioned name.
3. On first launch, select your locally owned Frontiers disc image.

Keep the included map catalog and supporting files in the extracted folder.
The game requires no Rust installation, Python, terminal or build commands.
No disc image or decrypted game data is distributed here.

BAT/CMD files are optional shortcuts: Start-EQOA.bat selects a local development
build; Play Map Preview.cmd also selects an isolated test profile. The EXE is
the normal player entry point. The external parser has its own setup for now.

Release builds update from [this fork's releases](https://github.com/LoopRook/eqoa-enhanced/releases),
with size/SHA256 checks before replacement. Local source builds do not update
automatically. Releases use MAJOR.MINOR.PATCH versions, starting with 0.1.0.

## Development and status

Start with [development](docs/DEVELOPMENT.md) and [contributor guidelines](AGENTS.md).
See the [map handoff](docs/MAP-RELEASE-HANDOFF.md), [playtest checks](docs/MAP-PLAYTEST.md),
[roadmap](docs/MODDING-AND-PARSER-ROADMAP.md), and [upstream sync report](docs/UPSTREAM-SYNC.md).
Full Forge parity and production companion packaging remain unfinished.

## License

The repository currently carries the upstream **MIT** license; copyright and
third-party notices remain intact. No licensing change has been selected.
Compare the [license options](docs/LICENSE-OPTIONS.md) before deciding.
