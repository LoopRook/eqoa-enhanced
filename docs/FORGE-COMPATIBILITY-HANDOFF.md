# Forge compatibility continuation handoff

## Owner-approved first public dev-build gate

The owner approved fully working `wereoxxs_ui` as the first public development
build milestone. This includes the whole package: independently positioned
hotbars with visible cooldowns, casting bar, player/target/group frames, effects,
compass, chat and damage meter, with its settings/profile and enable/disable/
reload behavior tested against actual gameplay. Merely loading the package or
showing only its hotbars is insufficient.

Modern UI (`avoids_modern_ui`) and ARPG UI (`arpg_ui`) remain eventual
compatibility targets but do not block this first dev build. The original
seven-package full-compatibility goal remains the later release gate, not the
first development-build gate. UIForge example acceptance is an internal
infrastructure checkpoint. Native maps remain defaults; optional Forge maps do
not block either gate. Describe the dev build's actually verified support
explicitly; do not advertise unrestricted Forge compatibility. This records a
future release criterion, not authorization to publish an untested build or to
change normal automatic updates.

## Latest checkpoint: accepted sound stage (2026-10-29)

The owner authorized resuming the next bounded stage after the earlier budget
stop. That stage is now complete and stopped at local code commit
`4e4dc737ca1c31e9d85a152da2cb48500038db5f`. Windows MCI sound APIs and instance
lifecycle are implemented and independently accepted by MiniMax rev.5; Kimi was
the implementation worker. Claude was down, Codex was at96% weekly and Argon
had an individual quota limit; other Antigravity models remain available.

Read `FORGE-SOUND-REPORT.md` and `forge-handoff/2026-10-29/sound-stage/` for exact
review dispositions, evidence and limitations. 133 addon tests/native MCI/
native build/WASM passed at substantive fix6cab6c1; the final Unicode-only helper
follow-up passed23 sound tests. Four existing Windows1314 symlink skips; no
native MCI/GPU/source/PNG skips in those accepted runs. No Linux execution or
user listening is claimed. No new preview, public release, enhanced merge or
push. Next stage is queued fonts; do not redo sound implementation from the
older uncompiled draft below. Stop here until owner continuation.

The remainder records the earlier checkpoint and historical decisions; this
latest section supersedes statements that sound is still unimplemented.

## Stop instruction and task

The owner requested a clean stopping point because weekly usage is nearly spent.
The already-active drawing slice is now finished and committed. Its exact tested
state and preparation artifacts are preserved. STOP here. Do not automatically start another phase or launch a successor agent.
Resume implementation only when the owner authorizes continuation. This document
is meant for a fresh agent with zero conversation history.

When resumed, continue native FrontiersForge compatibility until the required
non-map addons and a usable native author API can be tested and released. Do not
replace the scope with a window demo, fake module tables, or compilation alone.
The owner permits live testing intervention when native/server behavior needs it.
Provide short updates when a phase starts or a material blocker appears.

## Workspace and branch identities

- Main checkout: `C:/Users/hocka/Github Projects/EQOA Native Rust`.
- Active Forge worktree: `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`.
- Branch: `forge-compatibility`; Paseo workspace `wks_12cecc2d0898f767`.
- Other worktree used for preview tooling: `C:/Users/hocka/.paseo/worktrees/1usktejc/minimap-scale-fix-preview`.
- Shared Git repository: main checkout `.git`; command-local safe.directory is
  needed for the managed worktrees. Do not change global Git configuration.
- `origin` is upstream; `looprook` is this fork. Never push fork changes to
  master/origin, force push, move released tags, or discard unrelated worktrees.
- Main enhanced is at released 0.1.1 `fcd3923eb76ba037e49f1cb7380c829f66c5980b`.
  Forge contains that release via merge `ed731f4`; both native map fixes are included.
- Reviewed upstream policy base: `7912c7393df7d85655d5816980fae1217cc48a32`.
  Origin/master advanced later; use the explicit reviewed base for footprint
  checks until a separately reviewed upstream sync. Do not loosen budgets to
  hide differences against the newer upstream ref.

Read FORK.md first, then AGENTS.md/CLAUDE.md and docs/FORK-OWNERSHIP.md.
The earlier session freeze does not revoke the owner's explicit approval of this
compatibility initiative. Keep runtime implementation in fork_ext/. Player notes
belong in FORK-UPDATE-NOTES.md; UPDATE-NOTES.md remains upstream-identical.

## Owner decisions that must survive

- Both native maps remain defaults. Forge world_map and mini_map are OPTIONAL
  compatibility proof cases, not required replacements or release blockers.
- Required legacy packages: ff_example, screen_grid, uiforge_example,
  retro_health_hearts, arpg_ui, avoids_modern_ui, wereoxxs_ui.
- Support unchanged entry/private scripts using native high-level game adapters.
  Do not execute original emulator-memory shared modules, expose FFI pointers,
  emulate EEmem, or pretend MIPS hooks exist.
- Unsupported operations must be visible and actionable. Nil or false is allowed
  where source semantics require it, with diagnostics for unavailable capability.
- The PCSX2 HD texture pack is a separate deferred feature. Do not extract or
  stage `Testing Resources/eqoa-hd-pack-v2.7.zip` (about 2.2 GB). No JSON image
  conversion is required; native texture-ID replacement mapping is unimplemented.
- Updater notification polish is deferred in docs/backlog.md. The user observed
  a new exe and old renamed copy, but no update message. Do not change updater
  mechanics as part of Forge work.
- No public Forge release/merge has been requested yet. Local commits and the
  test preview are authorized. The earlier native map release WAS approved and
  published; do not republish it or confuse it with Forge acceptance.

## Local commits and current checkpoint

Already committed in Forge:

| Commit | Content |
|---|---|
| 3dc2a70 | Native Lua runtime, discovery/package/texture/manager/app foundation |
| ed731f4 | Merge published enhanced/map fixes into Forge |
| 10992c9 | Legacy hearts UI bridge and retained-texture lifecycle |
| 345fc8f | Native health receipt/snapshot/HUD claims and updater backlog |
| 7bf02d3 | Lua/ImGui dependency notices |
| 4e002c5 | Early preview and live acceptance instructions |
| 4e88236 | Common widgets, exact Begin tuple order, typed scope recovery |
| 9afa4c2 | Bit operations, clean os, protected pcall and coroutine removal |
| e4620d8 | Detailed handoff and durable, inert future preparation |
| b32deda | Scoped window/background drawing primitives and geometry limits |

The drawing slice is committed at b32deda0353e51db9adbc073977cfc0041abb0ee.
Implementation is clean; this handoff closeout changes documentation only. No
Forge push occurred.
Do not assume ignored preparation files are compiled or accepted implementation.

## What is implemented

Native-only mlua 0.12.2 / vendored Lua 5.1 and dear-imgui 0.18 / WGPU30 / winit0.30.
Dedicated VM/global environment/package.loaded per addon. 8 MiB Lua heap,
200,000 instructions per top-level execution, 4,096 bridge calls, depth32,
1 MiB script/profile limits. No native pointers are exposed to Lua.

Discovery handles loose scripts; package named entry then main.lua then init.lua;
one child scripts tree; native mod.json schema1/runtime lua/api eqoa/1. Companion
manifests are skipped. Private modules/resources precede shared directories.
Duplicate IDs are excluded, legacy names remain opaque, persistence uses safe
hashed identities. Module text reads are bounded; bytecode/FFI/dynamic loaders
are unavailable; host modules take precedence over disk.

PNG textures have owner-scoped opaque handles, limits, relative/legacy absolute
resource lookup, deferred collection after rendering, and actual post-upload Lua
allocation rollback tests. JPEG is not supported in the current addon loader.

Manager runs addons sequentially on one app thread/context/backend. Ordinary
DISABLE retains VM/globals/module cache/live textures; terminal failure, successful
replacement, removal and shutdown retire resources. Failed compile reload keeps
the old instance. Disabled Save/Load/Settings extensions retain textures but may
not publish HUD claims. Sixteen instantiated VMs includes disabled VMs: disabling
cannot free a slot; remove/rescan before reload at capacity.

Runtime creates graphics lazily, renders after render-scale resolve in pregame
and live game paths, uses physical pixels/Locked(1.0) DPI policy, routes capture
with press ownership/release/focus/raw motion bookkeeping, and shuts down while
Context is alive. Pre-render error cleanup ends an open frame before reuse;
actual same-Context GPU recovery tests exist. Standalone CLI shot_with does NOT
execute addons; app offscreen/report capture follows the overlay pass.

Current game APIs: util.IsInGame/IsStartMenuOpen (numeric 0/1), player.GetCurrentHp/
GetMaxHp, ui.DisableHealthBar/EnableHealthBar. Health receipt is reset on WorldEntry
and disconnect, only complete 0x42 stats count, HP0 is valid death, maxHP<=0 is
unavailable. Loading/HUD readiness suppresses managed in-game readiness. Pending
active pre-login hide intent promotes when ready; inactive callbacks cannot
create it. Cleanup, pause/failure/reload/shutdown revoke effective claims. Native
HUD cache invalidation and the health gauge/percentage label gates are wired.

Hearts ordinary re-enable behavior is SOURCE behavior: OnDisable releases the
hide claim; resumed initialized script does not reassert it. Hearts render again
with the native health bar visible. Explicit saved profile Load or the script's
settings toggle reasserts the claim. Do not invent an Initialize/Load callback
on ordinary enable to hide this distinction.

The common UI bridge adds real widgets, text/layout, menu/tree/tab/child scopes,
correct tuple returns, window flags and typed reverse unwind. Begin(name) returns
visibility; bool overload returns (open, shouldDraw), always owing End. Strict
scope ordering is a documented bounded contract and can reject unusual legal
native interleavings. Image supports 3 and 6 arguments with actual UV/tint/bg GPU
oracles. Some overloads remain deliberately unsupported; inspect the report.

Stdlib now provides the required eight-function bit subset, private native
os.clock/date closures, and pcall preserving normal errors/multireturns while
fatal instruction-limit flags escape nested protection. Only top-level run and
callback reset that flag/budget. Coroutine is removed from BOTH globals and
package.loaded and blocked by require (Lua5.1 base otherwise opens it). No xpcall,
io, arbitrary OS/file operations, debug, or dynamic chunk loading.

## Evidence and important limits

Root independently ran the pinned serial addon suite after UI+stdlib:
99 passed, 0 failed, four explicit Windows symlink-operation privilege skips
(error1314), no GPU/source/PNG prerequisite skips. Log is ignored
`target/root-ui-stdlib-verification.log`. Earlier health native build and WASM
check passed. The early release build passed. Full workspace last ran in public
0.1.1 CI, NOT for the new Forge runtime. Do not present that older CI as current
Forge coverage. Drawing evidence belongs in the final section below.

Real unchanged hearts hash:
`ac5c5b7ab8a245dfedfa7414d8938ef0608546c7618db17b44425c2467fafe37`.
Four original PNGs rendered in WGPU tests; full/half/death red fill counts were
1395/690/0, retained four handles reused on enable. The 256x192 fixture crops the
fifth heart at default position: it proves fill/lifecycle, not full layout/DPI.

Still manual/unproven: actual server HP delivery after WorldEntry, changing live
HP, login/zoning/reconnect, native window focus/input/IME/DPI, packaged cwd behavior,
visual comparison against Forge. If stats arrive only BEFORE WorldEntry and are
never sent after, conservative receipt reset could leave hearts waiting forever.
Keep the readiness explanation; investigate real order before changing freshness.
No account login, chat transmission or live-server acceptance was automated.

## Early preview already delivered to the owner

Main checkout:
`run/forge-compatibility-preview/21984f39e488/Play Forge Early Preview.cmd`
and `eqoa-forge-preview-21984f39e488.zip` in the same directory.

This package is source7bf02d3: EARLIER than the UI/stdlib/drawing work. It includes
only unchanged screen_grid and hearts plus four PNGs and released native maps.
Both addons default disabled; Options > Mods & Addons > Addon Manager enables
and configures them. Launcher uses its own profile/ and pinned catalog. Updater
channel/build commit were absent during compilation. No profile directory was
created by root. The owner has not yet reported this preview's live results.

Executable SHA256:
`21984f39e488bcef5aa15a710234e6d956214cf1ee73a544863d3bf6c2c72e2f`.
Final ZIP SHA256:
`f44ccaddb647d8090174411ea0e53577c0585c35747f12e0e401080ceeb82910`.
Sixteen unique ZIP entries independently compared byte-for-byte to disk, including
pinned addons/catalog and Native-Addon-Runtime-NOTICES.txt. No disc, logs, profile,
credentials or texture pack. Root corrected a PowerShell backtick-r typo in the
README and reverified/repacked; an earlier ZIP hash in worker output is obsolete.
Do not overwrite this directory or its future user profile.

## Resume order after owner authorization

1. Read final checkpoint, git status/log and current source; do not redo completed
   research/tests just because old prep notes describe it as missing.
2. Sound is accepted (see latest checkpoint). Implement fonts, then load unchanged packaged
   uiforge_example with original private click_tally and gear PNG. Sound prep is
   NOT compiled. Fonts are design only. Test every expanded UI section, settings,
   clicks, save/load, audio state, font sizes/fallback, failure and reload.
3. Expand native player/data/HUD adapters and startup/event/action paths needed by
   ff_example and the remaining HUD suites. Never solve missing imports with fake
   success or empty modules. Define the native eqoa author API/capability contract.
4. Required full acceptance includes real game data, action dispatch, independent
   chat/combat events, cooldowns/icons/inventory/quest/group/window lifecycle,
   profiles/resources/input and honest unavailable/unsupported controls. Loading
   a collapsed example is only a partial milestone, not full compatibility.
5. Build/package a concrete next preview after integration checks, then obtain
   player live/visual acceptance before claiming full compatibility/publication.

Sound choice: Windows MCI via winmm, directly following pinned UiForge; no new
shared audio.rs hook/dependency/second cpal stream. Missing files return nil,
invalid/nil IDs false, actual playback/query/stop/volume/release are required.
Inactive callbacks cannot start audio. Ordinary disable stops playback but retains
handles; terminal cleanup closes them. Paths/IDs/quotas/close-failure accounting
need review and tests. Non-Windows capability is explicitly unsupported. Draft
is preserved as inert .rs.txt; mock tests are not real playback evidence.

Font choice: safe StbTrueTypeFontData validator + FontSource::stb_truetype_with_size,
NOT raw unsafe TTF or ab_glyph header validation as a complete proof. Atlas updates
must happen before the frame. Stable owner/context handles initially resolve to
real default font and update to actual requested font after a pre-frame flush
(one-frame adaptation). Invalid/missing fonts cache real fallback. PushFont must
use reference LegacySize: passing0 keeps current size and breaks 20/36px requests.
Guard stale/foreign handles, typed Font stack unwind, bounded cache across reload,
and Context teardown. No font implementation exists yet.

## Sources and durable files

Pinned reference in main checkout ignored `run/forge-compat-reference/`:
FrontiersForge `4026d5cebcede24943ece8dc34833e584f46cd90` (v0.3.2),
UiForge `6f2d125cc686cbc882f8098a939cc03cc23a59a1`,
sol2 bindings `ec6f9ef7bab73e60f7ed32ef1e95dbee3b7b348e`.
Use actual sol_ImGui.h/C++ implementations, NOT the IDE Lua stubs.

Pinned ImGui gitlink is ocornut/imgui
`37b7a7a9dfc4a775c371bbd0ab8ceaf132c78fce` (1.92.7WIP).
Fetched header main `run/forge-preview-prep/imgui-37b7a7a9.h`, SHA256
`b642eaa18a010ba6b9e5c12fa4829ddba328eb1e7cb94571b0f6cf3d07912905`.
Source URL: https://github.com/ocornut/imgui/blob/37b7a7a9dfc4a775c371bbd0ab8ceaf132c78fce/imgui.h
Pinned AddRect is rounding,flags,thickness; native backend swaps last two. Keep
legacy Lua order and adapt only the FFI call. Defaults omitted through C++ member
pointers are a host convenience, not proof sol2 supplies C++ defaults automatically.

Durable reports: FORGE-{COMPATIBILITY-STATUS,COMPATIBILITY-MATRIX,RUNTIME-SPIKE,
DISCOVERY,PACKAGE-LOADER,TEXTURE-RESOURCES,LIFECYCLE-MANAGER,APP-INTEGRATION,
HEALTH-ADAPTER,HEARTS-BRIDGE,HEALTH-PLUMBING,UI-BRIDGE-NEXT,STDLIB,DRAWLIST}-REPORT.md
(the status/matrix names have no -REPORT suffix). The machine-readable matrix is
`docs/forge-compatibility-matrix.json`; its original unimplemented baseline is
historical, not automatically maintained coverage. Scanner is lexical regex,
not AST parsing. Plan is `docs/superpowers/plans/2026-10-28-forge-compatibility.md`;
its initial-state/proposed-render-crate paragraphs are historical. Current runtime
reuses the backend, with no new render-crate addon shader/dependency.

Prepared future work is preserved in `docs/forge-handoff/2026-10-29/` with hashes.
Read that directory's README caveats before using drafts. Do not apply stale
patches blindly. Implemented stdlib scratch patches in target/ are obsolete.

## Tooling, worker state and efficient verification

- Single compiler/build slot; at most three workers plus lead. User prefers
  Antigravity capacity and staggered builds. Claude was unavailable earlier;
  choose a receiving profile only if owner authorizes a new agent.
- Paseo server `srv_0YaZnxosBtr4`. Existing workers: Sol
  `c969d86a-e9c2-4a37-9382-7f588f25eac9` (drawing), Antigravity
  `469c8aba-3fc8-47c2-81e7-958f1df462bc` (stdlib/player audit), Antigravity
  `9ee7c0d9-3719-4927-95e7-d50647458a71` (media/packager). All were told to
  stop/idle at this checkpoint. An individual worker saying "no processes"
  describes its lane; verify global compiler processes before assigning a build.
- Use Paseo skills for operations; reuse workers, notifyOnFinish and wait for
  notifications rather than polling. Do not launch a successor on budget stop.
- Rust tools `C:/Users/hocka/.cargo/bin`, Python `C:/Python314/python.exe`.
  rg was unavailable; use Select-String/Python. Use command-local safe.directory.
- Use one consistent target cache `Forge-worktree/target`. Avoid alternating
  original checkout target and worktree target or changing toolchain/PATH env
  unnecessarily; C++ build scripts can rebuild expensively. No repeated broad
  workspace suite after bounded green checks without new cause.
- Windows PowerShell: single-quoted here-string to a temporary Python script
  avoids embedded quote/backtick/$ interpolation bugs. Do not use JSON.stringify
  as shell escaping. Use literal paths for file operations.
- Some reports were written with CP1252 bytes/trailing spaces; normalize UTF-8
  and check STAGED diffs. Untracked files are not checked by plain git diff.
- Root sandbox only writes original checkout by default; managed worktree writes
  used require_escalated auto-review with the owner's existing authorization.

Typical checks, from Forge worktree (one compiler at a time):

```powershell
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
$env:CARGO_TARGET_DIR='C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility/target'
cargo test --locked -p eqoa fork_ext::addons -- --nocapture --test-threads=1
cargo build --locked -p eqoa
cargo check --locked -p eqoa --target wasm32-unknown-unknown
python tools/check_fork_footprint.py --base 7912c7393df7d85655d5816980fae1217cc48a32
git diff --check
git diff --cached --check
```

Report explicit SKIP output: GPU/disc/source tests can return Ok after logging
SKIP, so harness "ok" is not proof the fixture executed. Full workspace EE tests
include an exhaustive bit-pattern test and can take a long time; do not run that
just to repeat already-completed focused verification.

## Final stop checkpoint

Stopped on 2026-10-29 after code commit
`b32deda0353e51db9adbc073977cfc0041abb0ee`. The following closeout commit contains
only this handoff and the status ledger. No next phase, successor, merge, push or
Forge release has started. Resume only on fresh owner authorization.

Final rebuilt pinned serial addon suite: **108 passed, 0 failed**, with four
explicit Windows symlink-operation privilege skips (1314). No GPU/source/PNG
prerequisite skips. Root checked the actual Compiling line and final result in
`target/drawlist-prep/addons-restored.log`; this is the authoritative final log.
Focused drawing tests: nine passed. Actual wrong-window-target and omitted-fill
negative controls failed their intended pixel assertions, then clean production
source was restored. See FORGE-DRAWLIST-REPORT.md for exact scope and caveats.

One intermediate post-restore run reused a negative binary because Copy-Item
preserved an old source timestamp: 105 passed/three expected fill failures with
no Compiling line. Source hashes were correct; updating its timestamp forced the
actual rebuild and the authoritative 108/108 GREEN run. Do not cite that stale
run as final evidence. Initial RED also exposed a host test-fixture MissingEnd
abort; fixture cleanup was corrected before GREEN, as the report records.

Drawing includes real window/background target separation, rectangle/circle/
triangle/ngon fill and outline, private window/callback lifetime tokens and a
shared per-frame geometry quota. Rounded rectangles reserve 520 units; automatic
circle estimates account conservatively for native tessellation. One hundred
radius20 balls fit; repeated excessive radius4096 geometry is rejected. Omitted
argument defaults are a host convenience; window handle lifetime is stricter than
upstream raw pointers. Context teardown assumptions remain documented limits.

At closeout all three workers were confirmed idle, the global cargo/rustc/link/
eqoa process count was zero, and implementation git status was clean. All seven
preserved preparation attachments matched manifest.savedSha256. Formatting and
diff checks passed. The final documentation commit is followed by a reviewed-base
footprint check and clean-status verification. No native/WASM/full-workspace or
release build was repeated for this bounded native-private drawing slice.

Full compatibility is still incomplete. Fonts, real sound integration, remaining
game adapters and required unchanged-addon acceptance remain, along with live
server/window/DPI/input gates. The delivered early preview remains the older
7bf02d3 build, not a package of this final checkpoint.
