# Wereoxxs UI frame cost

The live `EQOA_PERF` log reported 10–15 ms per frame for `wereoxxs_ui` in a busy zone. One sample attributed 12.29 of 12.62 ms to Lua.

## Reproduction

Run from the repository root in PowerShell with the local disc in `run/eqoa.iso`.

```powershell
$env:EQOA_ISO = (Resolve-Path 'run/eqoa.iso').Path
$env:EQOA_WEREOXXS_PROFILE = '1'
python tools/run_gates.py --only round --filter wereoxxs_busy_zone_frame_cost
```

The test uses the bundled package, the real addon manager, Play's snapshot projection, and the ImGui GPU fixture. It enables all eight Wereoxxs panels and fills all 14 slots projected into the three Forge bars, five party rows and eight effects. The player and 200 nearby creatures move, and the entity-update path sees one eighth of the creatures each frame. The native chat adapter fills the default 2,000-line scrollback with a mix of channels, then delivers one party message every four frames.

The first pass measures 40 warmed frames with the same addon timers used by `EQOA_PERF`. The optional second pass wraps the panel draws and Forge update calls with `os.clock` inside a scratch copy of the entry function. Those timings include calls made by each Lua function, so they complement the live-style Lua, API and bridge split. The legacy timer registers some widgets, including `TextWrapped`, after it installs its bridge wrappers. Their time appears under Lua.

Set `EQOA_WEREOXXS_REFERENCE_CHAT` to a scratch copy of the original `modules/chat_window.lua` to run both versions in one test process. The fixture compares GPU readbacks of the busy All tab and of a tell tab after changing its color and turning timestamps off. It fixes the chat clock at `12:34` for the comparison.

## Baseline

The debug fixture measured 51.015 ms per frame, split into 37.58 ms Lua, 2.70 ms API and 10.74 ms bridge. `DrawChat` took 41.225 ms per frame in the separate function profiling pass. `DrawAbilityBarWindows` took 2.700 ms, `Chat.Update` 2.200 ms and `DrawCompass` 2.125 ms. The remaining panels each took under 2 ms. The Lua instruction counter reported 124,625 per frame and a peak of 134,000.
