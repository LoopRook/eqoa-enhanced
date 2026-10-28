# Development freeze — EQOA Enhanced 0.1.0

The owner requested a final distribution check and a development freeze. The
current published client remains 0.1.0; no further game change or release is part
of this closeout. The package audit found no missing required game-runtime files.

## Frozen release

- Tag v0.1.0: c7fddfc35a2f2d4185793af95b2e595a12e6a0e9.
- Windows EXE SHA256:
  b35ed060520e512cc875d4b7c95019b3e7c1bcae260aee673565444bbddefb25.
- Release/verification: RELEASE-0.1.0-VERIFICATION.md and RELEASING.md.
- Release-control recovery preserves the tag and its game code; see the recorded
  successful CI and the original Windows short-path fixture failure.

## Map data sources

The ZIP includes map-entities.json beside eqoa.exe. The packager obtains it from
mmvest/FrontiersForge, scripts/resources/entity_database.json, pinned revision
4026d5cebcede24943ece8dc34833e584f46cd90 and SHA256
37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca.
The Forge MIT notice is included. The client reads this bundled file by default;
it does not fetch the latest catalog on every start. EQOA_MAP_DATABASE is an
explicit optional path override. Historical locations do not prove live spawns.

Terrain/game assets are read/generated from the player's own disc and cached
locally. Camp/coach tables and map/HUD presentation are compiled into the client;
nearby live NPC and group positions come from data received from the server.

## Optional mods folder: actual current behavior

fork_ext/mods.rs::ensure_mods_dir creates a starter README and
mods/eqoa_parser/mod.json when Open Mods Folder or the native parser launcher is
used (the discovery helper also calls it). The parser manifest is embedded into
the EXE at compile time. Existing files are preserved. This creates metadata,
not an installed parser, downloaded mod or map/HUD implementation.

The directory resolver uses a mods folder beside the EXE when already present,
otherwise a relative mods directory in the process working directory. These
usually coincide on a normal double-click; a shortcut with a different working
directory can place it elsewhere. This is a small known optional-companion path
cleanup for a future maintainer, not a missing game asset. The generated starter
README also overstates general drag-and-drop addon support: current UI provides
HUD options, folder access and a companion launch action, not a general UI-theme
or script/plugin loader. Do not infer such support from that README.

The external parser still requires separate installation/dependencies and
EQOA_PARSER_DIR. It is intentionally not bundled, and its separate repository was
not changed. Its own agent owns further parser installation/integration work.

## Other limits retained at freeze

- Automatic desktop updates currently replace only the EXE. A future catalog or
  notice change needs deliberate asset-update support or a complete ZIP refresh.
- Settings/disc/server selections reuse %APPDATA%/eqoa-rs; generated caches use
  %LOCALAPPDATA%/eqoa-rs. The release contains no personal configuration or disc.
- A new player needs their own disc and server/account setup. Existing-profile
  success is not proof of a complete clean-Windows first-run playtest.
- Full Forge parity, runtime themes, live moving-group/zoning/despawn acceptance
  and parser packaging remain documented future work, not 0.1.0 guarantees.
- Existing license notices remain unchanged; alternative licensing and submitting
  the prepared animation branch remain owner decisions.

Do not restart implementation from these follow-ups without a new owner request.
