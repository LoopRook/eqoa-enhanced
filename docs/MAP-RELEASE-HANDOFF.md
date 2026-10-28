# Native map release handoff

## Start here

This delivers a playable native map milestone on the **enhanced** line, implementation commit 928868e, based on
`9fbfce3`. It is not a claim of complete FrontiersForge parity. Parser implementation
belongs to another agent; this work changes parser documentation only.

Read `MAP-PLAYTEST.md` for player controls, `MAP-PARITY.md` for remaining scope,
and `MAP-THEMING.md` for presentation boundaries. Review the matched
[before/after evidence](map-parity-captures/README.md) before changing layout.
The HTML in `design/map-parity/` is a historical interaction study with sample
data. The shipped UI is Rust/WGPU; no browser runtime is embedded in the game.

## Delivered behavior

| Area | Behavior and implementation |
|---|---|
| Search | Native temporary NPC / Group / Places panel. Case-insensitive name fragments across the historical Forge catalog and server-visible NPCs. Search closes after choosing a result. |
| NPC guidance | Shared target identity on world map and minimap. Gold historical locations/dashes coexist with green live matches. Historical coordinates never imply a creature is spawned. Native entity observations are limited to what this client receives. |
| Group guidance | Member ID/name plus a valid scene actor position or active channel-40 position. Other-world/unavailable positions have no arrow. Roster departure clears the selected member. |
| Personal pin | Set the normal map waypoint, then **Track waypoint**, or find **Personal waypoint** under Places. Guidance follows pin changes, stops at arrival/deletion, and produces no false arrow in another world. Stop does not delete unrelated waypoints. |
| Circle | Terrain and pre-tooltip minimap UI use the same GPU circle. Far markers clamp to its rim. Popup/tooltip/control ranges remain unmasked. Corner clicks and wheel events do not hit the circle. Border geometry is computed in window pixels because the game's frame x/y scales differ. |
| Layout | Search/track toolbar follows measured title, progress, coordinate and notice chrome. Do not restore a fixed y offset: that caused the reported coordinate overlap. |
| Styling | Map controls, labels, tooltips and XP camp badges use the native theme. Zoomed-out camps use compact neutral diamonds; closer XP badges use dark surfaces, thin borders, white text and a small accent. Coach identity and semantic marker colors remain recognizable. |
| Suggest | Mouse/keyboard keeps the map visible with name, optional note, location, validation, Submit and Cancel. Existing community transport, selected coordinates and approval feedback remain. Controllers retain the existing soft-keyboard flow. No real marker submission was made by automated validation. |
| Input | F2 Search and F4 Stop have persisted Controls rows and optional controller-button assignments. The existing M/N/zoom/group-waypoint rows remain. Labels resolve saved bindings. Typing owns input and releases prior attack/info/aim holds. |

Guidance is direction/distance, not obstacle-aware routing or automatic movement.

## Architecture and safe extension points

- `map/search.rs`: catalog, filtered result cache, target identity and live/group/pin
  guidance. Invalidate relevant cached results for zoning, roster/validity changes,
  despawns and pin changes. Do not rebuild the entire catalog every game frame.
- `play.rs`: supplies actual entity/group observations; `map/mod.rs` synchronizes
  persisted personal pins after arrival/deletion processing.
- `map/theme.rs`: surfaces, borders, text and semantic marker tokens, embedded font
  and logical label-font generation. Controls use a supersampled font separately.
- `map/suggest.rs` and `mapsuggest.rs`: in-map form versus retained controller
  editor; both feed the original community submission API.
- `map/mini.rs`: circle projection, marker/rim placement, pointer hit shape and
  masked-content boundary. Draw tooltips/search after that boundary.
- `render/map.rs`, `render/ui.rs` and their WGSL shaders: explicit opt-in circular
  draw APIs. Default calls remain unmasked. Mask coordinates are normalized so
  render-target scaling does not move the circle.

Do not make themes supply positions, execute commands, hard-code key labels or
alter guide identity. Runtime color/theme-pack loading remains future work;
`EQOA_MAP_FONT` currently accepts an optional local TTF with a bundled fallback.

## Design provenance and adaptations

Primary local reference: **Track Level Companion**, `CLAUDE.md` section **6.E**
and `src/index.css`'s Nothing design tokens, at
`ab6552163187e8caec8963682ad7cf7ee2a41d71`.
Secondary reference: **EQOA Parser**, `docs/research/ui-overhaul.md` and
`eqoa_parser/ui/web/style.css`, at
`9170b6d2385e7cf2f1f3f1a3ab293560f936202d`.

Applied: flat neutral surfaces (`#111111`, `#1A1A1A`), visible neutral borders,
vermilion `#D71921` focus/selection, Space Mono data/labels, no drop shadows.
Use the current project section 6.E rather than stale historical CSS comments
that mention a different card radius. These reference repositories were read only.

Native adaptations: dark desktop game overlay rather than the trackside app's
light default; compact frame-unit controls rather than its phone touch layout;
rectangular text controls; Space Mono throughout the map rather than adding Doto
hero metrics or a new app-wide font system. Map/faction/navigation colors retain
game meanings. The circular minimap is geometry, not a pill-shaped text control.
The normal HUD and unrelated game windows retain their existing artwork.

## Reproduce a player bundle

From a Windows checkout with Rust on PATH:

```powershell
./tools/build-map-preview.ps1
```

This builds `eqoa` with `--release --locked`, verifies the pinned external catalog,
creates a content-addressed executable, two equivalent preview launchers, licenses,
README, manifest and ZIP in ignored `run/map-parity-preview/`. `-CatalogPath` uses
an existing verified catalog; otherwise the script downloads the pinned public
resource. `-SkipBuild` packages an already built executable. `CARGO_TARGET_DIR`
is respected. The ZIP uses an explicit allowlist: it never contains `profile/`,
saved accounts, logs, disc images or arbitrary old preview files.

The launcher uses an isolated `profile/` next to the executable. Select a locally
owned Frontiers disc if prompted. Close the existing game before opening the new
client; updating a launcher does not replace an executable already running.

The NPC sidecar is **required for broad historical NPC search**. A plain executable
without it still has live observations and built-in locations, not the full
reference catalog. Do not publish the normal single-executable updater payload
and assume the sidecar is installed. The preview ZIP is complete; a future
production updater/release must explicitly package, embed or provision this data.
Local integration does not itself publish a release. Program pushes to master
trigger release automation, so review that delivery path before publishing.

Catalog source: FrontiersForge
`4026d5cebcede24943ece8dc34833e584f46cd90`,
`scripts/resources/entity_database.json`, MIT.
SHA256: `37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca`.
Imported locations: **7,180**; malformed positions are skipped instead of being
converted to `(0, 0)`. The reference may differ from the connected server.

Space Mono Regular is embedded under SIL OFL 1.1; its notice lives in
`crates/eqoa/resources/map-ui/OFL.txt`. Forge's notice is in
`docs/licenses/FrontiersForge-MIT.txt`. Both accompany the player bundle.
Disc data and the downloaded catalog remain untracked.

## Validation and review

- Full offline locked workspace suite passed before the final border correction: 1,137 passed, 2 ignored across 85 test/doc-test summaries.
- After that correction: **905 client tests passed; 2 existing FX benchmarks
  ignored** (`fans_cover`, `volumes_cost`).
- Real GPU pixel checks passed on NVIDIA GeForce RTX 3070 Ti. They verify circular
  terrain/UI, unchanged rectangular world views, scaled projection, and controls
  outside the circle. The border regression checks standard/wide/portrait geometry.
- Suggest tests use a loopback HTTP server and verify the name/note/coordinate
  payload and cancellation. No automated request submits to the real community API.
- Native captures cover matched world/search/overview, wide and portrait minimaps,
  plus personal-waypoint guidance. Capture parameters and executable hashes are
  tracked beside the images.
- Fresh source reviews covered input, target validity, caching, font fallback,
  clipping and the final renderer changes. A reviewed border/mask aspect-ratio
  mismatch was reproduced in a failing test and corrected.

Prerequisite-based tests can return early when their local disc, decrypted CLIENT,
2002 disc, oracle or external S2 patch fixtures are absent. The native captures
used the locally owned disc; do not describe the skipped fixture checks as oracle
or live-server validation. The user's earlier live playtest exercised search and
reported the layout defects. Moving group members, zoning/reconnects and real
despawns in this final build still need the in-game checklist in MAP-PLAYTEST.md.

## Remaining full parity and release work

Fuzzy/exact name modes, level filters, persistent discovery journals, named saved
location management, Forge's handmade map art/calibration, minimap rotation,
height-aware indoor views, entity filters/tracking lists and runtime theme packs
remain. Native generated terrain is retained. This milestone provides usable map
search/guidance and the reported UI corrections; do not mark the entire parity
checklist complete or make parser integration a prerequisite.

Integration target is the verified source branch **enhanced**. Review
`git log --oneline codex/forge-map-parity` for implementation and handoff commits.
Pre-integration primary-checkout documentation is retained in a named local Git
stash (`pre-map-integration documentation`) and a local backup if integration
needed to clear older drafts; updated documents include their content. Do not
blindly pop an older draft over this handoff.
