# EQOA Enhanced

A native Rust client for **EverQuest Online Adventures: Frontiers**, extending
[DabDavis's client](https://github.com/DabDavis/eqoa-rust-client-vanilla) with map
search, guidance and quality-of-life features. This fork keeps upstream engine,
network and gameplay improvements while developing its enhanced features.

## More from LoopRook

<p align="center">
  <a href="https://github.com/LoopRook">
    <img src="https://wsrv.nl/?url=github.com%2FLoopRook.png%3Fsize%3D256&amp;w=160&amp;h=160&amp;fit=cover&amp;mask=circle&amp;output=png" alt="LoopRook's GitHub avatar" width="112">
  </a><br>
  <strong>Hi, I'm LoopRook.</strong><br>
  A few more things I've made:
</p>

<p align="center">
  <a href="https://looprook.github.io/Multivitamin/"><img src="https://img.shields.io/badge/Multivitamin-6750A4?style=for-the-badge&amp;logo=discord&amp;logoColor=white" alt="Multivitamin - Discord bot"></a>
  <a href="https://looprook.github.io/tidepool/"><img src="https://img.shields.io/badge/Tidepool-147D82?style=for-the-badge" alt="Tidepool - artificial life simulation"></a>
  <a href="https://github.com/LoopRook/EQOA-Parser"><img src="https://img.shields.io/badge/EQOA%20Parser-9B5C39?style=for-the-badge" alt="EQOA Parser - combat parser"></a>
</p>

## License

GNU Affero General Public License, version 3 or later ([LICENSE](LICENSE)). If
you distribute a modified copy or host a modified version, players must be
offered its complete source. Code published through 0.4.5 and up to upstream
commit 1e23e80 was released under MIT and remains available under MIT at those
commits; `NOTICE` keeps that record. Third-party code retains its own license.
Independent addons using the addon interface keep their authors' own licenses.
The license grants no rights in EverQuest Online Adventures itself; the game and
its data belong to their owners, and the client reads a disc you own.

## Play on Windows

**[Get the latest Windows release](https://github.com/LoopRook/eqoa-enhanced/releases/latest)**

1. Under **Assets**, download `EQOA-Enhanced-<version>-Windows-x64.zip`.
2. Right-click the ZIP and select **Extract All**, then choose a folder you can
   write to. Keep `map-entities.json` and the other included files beside the game.
3. Double-click **eqoa.exe** and select your locally owned EverQuest Online
   Adventures: Frontiers disc image when prompted.
4. Choose a server and sign in with an account for that server.

The download does not include a disc image or game account. Account registration
depends on the server you choose. You do not need Rust, Python or a terminal to play.

Official releases check for updates automatically. The updater replaces the
game executable; if release notes say supporting files have changed, download
and extract the latest ZIP as well.

## Support

Need help getting started or using the client? Join the
**[LoopRook Support Server on Discord](https://discord.gg/K7wJUYGYj)**.

You can also use **F3** in game to save a problem report to your Desktop. The
report is saved locally and is not uploaded automatically.

## Features

- World map and circular minimap with NPC, group and location search.
- Live sightings, recorded spawns, hover details and waypoint guidance. A
  creature marked **Not detected** is not necessarily dead or despawned.
- Quest guides, coaches, XP camps and community map suggestions.
- Optional HUD features and native addon support, with Wereoxxs UI, ARPG UI
  and Avoid's Modern UI included. The combat parser is a separate optional
  companion; see [companion setup](docs/COMPANION-LAUNCH.md).
- Upstream fishing, GM windows, character shadows and protocol improvements.
  Features requiring server support depend on the connected server.

Included mods start off. To enable one, open **Options > Extended > Mods &
Addons > Addon Manager** in game.

## Developers

See the [developer guide](docs/DEVELOPMENT.md) and
[contributor guidelines](AGENTS.md) for building and testing from source.
`Start-EQOA.bat` is a source-checkout shortcut that prefers a local release
build; local source builds do not update automatically.

The [roadmap](docs/MODDING-AND-PARSER-ROADMAP.md),
[map handoff](docs/MAP-RELEASE-HANDOFF.md),
[playtest checks](docs/MAP-PLAYTEST.md), and
[upstream sync report](docs/UPSTREAM-SYNC.md) cover work in progress.
Full Forge parity and production companion packaging remain unfinished.
