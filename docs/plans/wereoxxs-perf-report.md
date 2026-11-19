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

## Follow-up with a recorded play session

`measure_wereoxxs_recorded_session` reads the recording parts in place and queues the
incoming messages in an in-memory DRDP endpoint. `Play::frame` handles them through the
real session dispatcher. The fixture also restores the target ID from each outgoing
position message. It runs Wereoxxs and the shared resources with a stand-in scene.
The optional saved profile supplies the original panel settings and ImGui layout.

Set `EQOA_WEREOXXS_RECORDINGS` to the ordered list of parts joined by the OS path
separator. Set `EQOA_WEREOXXS_SAVED_PROFILE` to the saved profile JSON and
`EQOA_WEREOXXS_SIZE` to the original window dimensions in `WIDTHxHEIGHT` form.
`EQOA_WEREOXXS_REFERENCE_ENTRY` can replace the bundled entry in the fixture's scratch
package. Set `EQOA_WEREOXXS_CAPTURE=1` for SHA-256 hashes of the GPU RGBA readbacks.
In a separate run, `EQOA_WEREOXXS_PROFILE=1` enables the `os.clock` probes around the
panels, Forge updates and font requests. The fixture fixes the chat timestamps for
both versions.

The recorded profile had only chat and the ability bars enabled in Wereoxxs. Chat used
44 px Merriweather Sans. The release profile showed that `UiForge.LoadFont` accounted for
8.509 to 11.733 ms per frame across the three samples, including the sample with an
empty scrollback. The whole `DrawChat` call took 8.807 to 12.150 ms. Ability bars took
0.632 to 0.833 ms. Combat and chat updates each stayed below 0.04 ms. Target, player,
group, casting, compass and meter panels were off in the saved profile.

The font bank already reused the loaded handle. The host checked the addon's resource
mounts and walked the Windows filesystem before it reached that cache. Wereoxxs
requested the same handle every frame, so those checks were charged to Lua in
`EQOA_PERF`.

Wereoxxs now holds one custom font handle per window and requests a new handle when
the face or size changes. Cleanup clears the handles before the addon starts again.
In the synthetic GPU budget test, the original requested the same font eight times
in eight frames and failed the one-request limit. The revised entry passed, including
the checks that each font size and face change makes one fresh request.

The paired release test used the same executable and swapped only the Lua entry.
The timing pass ran without the Lua probes. Each row is a mean across the sampled
frames in that interval. This run used the standard Windows temporary directory.

| Recording interval | Frames | Chat rows at capture | Original ms/frame | Revised ms/frame | Reduction |
| --- | ---: | ---: | ---: | ---: | ---: |
| 41–43 s | 57 | 0 | 11.150 | 2.144 | 80.8% |
| 183–185 s | 60 | 13 | 11.901 | 1.586 | 86.7% |
| 525–527 s | 60 | 34 | 12.432 | 2.093 | 83.2% |
| Weighted mean | 177 | | 11.839 | 1.938 | 83.6% |

All three nonblank GPU readbacks had matching RGBA hashes. A pair run in a separate,
more deeply nested scratch directory also matched all three readbacks and measured
18.718 to 1.270 ms per frame. In the separate profiled pass, the revised `DrawChat`
took 0.018 to 0.233 ms per frame. The replay isolates
the addon from terrain rendering and the other enabled mods. It uses the recorder's
message timing and target IDs with a fixture camera and a fixed ImGui time step.
The owner's full-client run remains the reference for actual gameplay timing.
