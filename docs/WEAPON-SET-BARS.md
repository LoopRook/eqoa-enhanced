# Native weapon-set bars

Owner-approved scope: add a Wereoxxs weapon-set bar, then safely update bundled
packages, in separate commits. Assess Modern UI and ARPG UI afterward; patch each
only when its own renderer/settings can be reused in about 60 lines.

The native source is `gear.rs` (four sets plus bare hands, server availability,
icons and badges), `hudmenu.rs` menu 3 (selection), and `Play::use_gear_set`
(existing 0x0022 request). The owner explicitly chose **carousel selection**,
not an inferred equipped set. All Lua indices are zero based, including bare
hands at 4. Empty saved sets remain empty slots, as in the native menu.

`eqoa.WeaponSets` provides `IsAvailable()`, `GetCount()`, `GetSelected()`,
`GetIcon(i)`, `GetBadge(i)`, `GetName(i)`, `IsEmpty(i)`, and `Use(i)`.
Texture getters return the same texture/width/height tuple as Forge Icon.GetTexture.
GetSelected is the native carousel's selected entry, even when another menu is up;
it does not promise that the server equipped anything. Use returns whether a
bounded request was queued; gameplay revalidates it and calls use_gear_set.
Use neither confirms equipment nor invents cooldown/combat rules.

Implementation plan:

1. Add failing native-data/API/dispatch and bundled Wereoxxs on/off/absent tests.
2. Add a fork-owned snapshot/API and dispatch through the existing play hook.
   Reuse Wereoxxs bar config, renderer, click regions and saved layout. Preserve
   native keyboard/controller input and PCSX2 behavior without eqoa.
3. Run focused tests, update source hashes and player notes, commit the bar.
4. Add failing installer update/edit/new-file/deletion/settings tests. Extend the
   per-folder registry with shipped hashes; preserve legacy deletion records.
   Bound reads, reject links, use atomic writes and never delete player files.
   Commit separately after tests.
5. Assess and, if small, test/patch Modern UI and ARPG UI separately.
6. Run plain eqoa, reference/include-ignored, wasm, footprint and Python suites.
   Commit all changes; run the release build after the final commit. Report
   exact summaries, skips, commits and source references. No push/merge/tag.
