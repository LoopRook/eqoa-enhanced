# Wereoxxs UI frame cost

The live `EQOA_PERF` log reported 10–15 ms per frame for `wereoxxs_ui` in a busy zone. One sample attributed 12.29 of 12.62 ms to Lua.

## Reproduction

Run from the repository root in PowerShell with the local disc in `run/eqoa.iso`.

```powershell
$env:EQOA_ISO = (Resolve-Path 'run/eqoa.iso').Path
$env:EQOA_WEREOXXS_PROFILE = '1'
@'
from pathlib import Path
from subprocess import check_output

source = '3cd45e0b:bundled-mods/wereoxxs_ui/modules/chat_window.lua'
reference = Path('run/wereoxxs-perf/original-chat_window.lua')
reference.parent.mkdir(parents=True, exist_ok=True)
reference.write_bytes(check_output(['git', '-c', 'safe.directory=*', 'show', source]))
'@ | python -
$env:EQOA_WEREOXXS_REFERENCE_CHAT = (Resolve-Path 'run/wereoxxs-perf/original-chat_window.lua').Path
python tools/run_gates.py --only round --filter wereoxxs_busy_zone_frame_cost
```

The test uses the bundled package, the real addon manager, Play's snapshot projection, and the ImGui GPU fixture. It enables all eight Wereoxxs panels and fills all 14 slots projected into the three Forge bars, five party rows and eight effects. The player and 200 nearby creatures move, and the entity-update path sees one eighth of the creatures each frame. The native chat adapter fills the default 2,000-line scrollback with a mix of channels, then delivers one party message every four frames.

The first pass measures 40 warmed frames with the same addon timers used by `EQOA_PERF`. The optional second pass wraps the panel draws and Forge update calls with `os.clock` inside a scratch copy of the entry function. Those timings include calls made by each Lua function, so they complement the live-style Lua, API and bridge split. The legacy timer registers some widgets, including `TextWrapped`, after it installs its bridge wrappers. Their time appears under Lua.

With the reference file supplied, the test runs both versions in one process. The fixture compares GPU readbacks of the busy All tab and of a tell tab after changing its color and turning timestamps off. It fixes the chat clock at `12:34` for the comparison. The optimized comparison used a scratch copy of `tools/run_gates.py` with `--release` added to the `round` test command and the same shared build lock.

## Optimized comparison

The paired release-profile test measured a **40.4% reduction**, from **14.301 to 8.528 ms per frame**. These are addon timings from a synthetic scene on the owner's Windows machine.

| Addon timing, ms/frame | Original | Updated |
| --- | ---: | ---: |
| Lua | 11.09 | 7.15 |
| Forge API | 1.09 | 1.04 |
| ImGui bridge | 2.13 | 0.34 |
| Total | 14.301 | 8.528 |

| Function profile, ms/frame | Original | Updated |
| --- | ---: | ---: |
| `DrawChat` | 10.500 | 4.850 |
| `Chat.Update` | 0.900 | 0.800 |
| `DrawAbilityBarWindows` | 0.850 | 0.825 |
| `DrawGroupFrames` | 0.550 | 0.500 |
| `DrawCompass` | 0.500 | 0.550 |
| `DrawPlayerFrame` | 0.375 | 0.375 |
| `DrawTargetFrame` | 0.075 | 0.175 |
| `DrawMeter` | 0.075 | 0.100 |

Each other draw or update took at most 0.025 ms in the separate profiling pass. Both GPU captures matched their original equivalents byte for byte.

## Baseline

The initial debug fixture measured 51.015 ms per frame before the optimization. `DrawChat` accounted for 41.225 ms in the function profiling pass. The largest remaining costs were `DrawAbilityBarWindows` at 2.700 ms, `Chat.Update` at 2.200 ms and `DrawCompass` at 2.125 ms.

## Change and negative control

The chat module builds each display line with one concatenation. It sets the wrap position once per child and draws each row with one `TextColored` call on hosts that support both operations. Older Forge hosts use the original three calls. `SOURCE.txt` records the changes and the updated checksum.

The regression uses a peak budget of 110,000 Lua instructions so it can run under varying machine load. The isolated run of the new test on the original Lua failed at 134,000 instructions, as expected.

## Paired debug comparison

| Metric | Original | Updated |
| --- | ---: | ---: |
| Addon cost, ms/frame | 46.478 | 21.719 |
| Lua, ms/frame | 34.28 | 17.45 |
| API, ms/frame | 2.64 | 2.78 |
| Bridge, ms/frame | 9.55 | 1.49 |
| `DrawChat`, ms/frame | 39.100 | 10.950 |
| `Chat.Update`, ms/frame | 2.225 | 2.100 |
| Peak Lua instructions | 134,000 | 104,000 |

The paired run reduced addon time by 53.3%. The busy All and tell tab captures matched their originals byte for byte. The focused gate passed four tests. It also ran the existing bundled picture comparison against a scratch image from the original code, the Wereoxxs chat-scroll regression, and the bundled Forge UI lifecycle regression.

## Release validation

`python tools/run_gates.py --release --lock-wait 120` ran with the local Frontiers disc configured.

| Suite | Passed | Failed | Ignored | Skip messages |
| --- | ---: | ---: | ---: | ---: |
| Addons | 523 | 0 | 25 | 10 |
| Full client | 2,406 | 0 | 30 | 51 |

The browser check, fork footprint, path audit and all 20 packaging tests passed. The native release build is queued behind an empty shared Cargo lock directory. The gate runner treats a tokenless lock as a manual hold.

The suites reported skips for Windows symlink privileges, optional pinned Forge fixtures and reviewed goldens, and supplementary game-data fixtures including the 2002 disc, client image, world data and savestates. The decomp comparison also skipped. The Wereoxxs benchmark and its paired picture comparisons ran on the GPU, and the bundled Wereoxxs picture test passed against the original capture in the focused run.
