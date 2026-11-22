# Developer Tools

This document describes the Python developer helper tools located under `tools/`.

## Mod API reference (`eqoa mod-api`)

The native client can print the capabilities installed for an `eqoa/1` addon:

```powershell
eqoa.exe mod-api
eqoa.exe mod-api --format json
eqoa.exe mod-api --format md --out docs/modding/SUPPORT-TABLE.md
eqoa.exe mod-api --format json --out docs/modding/eqoa-1-surface.json
```

Markdown is the default. **Use `--out <file>` for reliable exports**; it writes
UTF-8 to the named file. Without it the reference goes to stdout, which is only
visible when the program is run from a terminal (the Windows application has no
console of its own). Invalid options, install failures and output errors
exit with code 1 and an error on stderr. The command installs a scratch native
addon and its ImGui, texture, font and sound adapters. It uses a headless graphics
device (required), creates no game window, and needs no disc, login or server.
It does not run the addon's entry script or call game actions.
If no adapter or device can be acquired, the command exits 1 without writing a
reference. GPU-dependent reference tests instead print a `SKIP` line and return,
following the repository's GPU-test convention; other install errors still fail.

Generate both committed files with a Windows release build, the published
platform; sound capabilities differ elsewhere. Build provenance uses the compiled
client version and `EQOA_BUILD_COMMIT` stamp (or explicitly says it is unstamped),
the platform, and the UTC generation date. After changing the API, rebuild the
client, rerun both `--out` commands above, and commit both files.

`round7b_support_table_matches_the_running_client` regenerates the reference
through the same installers and compares every capability and the Markdown
layout. It retains the committed generation metadata during comparison: another
date or build stamp alone does not change the API. The Windows snapshot is not
compared on other platforms. These files report the current capability registry;
they do not freeze the API or replace the spec's behavior tests. Forge and ImGui
parity also rely on the bridge tests and bundled UI tests.

## 1. Branch Gates Runner (`tools/run_gates.py`)

Runs the standard development gates in serial order, managing the shared Cargo build lock, streaming step logs, and emitting a summary Markdown report.

### Command

```bash
python tools/run_gates.py [options]
```

### Steps

1. `round`: `cargo test --locked -p eqoa --bin eqoa -- <filters> --test-threads=1 --nocapture` (filters from `--filter`; step skipped when no filter is provided).
2. `addons`: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1 --nocapture`.
3. `full`: `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture --skip round4_ --skip frame_time --skip benchmark --skip scrollback_10000 --skip measure` (extended via `--extra-skip`).
4. `wasm`: `cargo check --locked -p eqoa --target wasm32-unknown-unknown`.
5. `footprint`: `python tools/check_fork_footprint.py --base <reviewed_base>`.
6. `paths`: `python -X utf8 tools/check_no_local_paths.py`.
7. `player-release`: `python -X utf8 tools/test_player_release.py`.
8. `release` (only with `--release`): `cargo build --locked --release -p eqoa` with `RUSTFLAGS=-C target-feature=+crt-static`.

### Options

- `--only <steps>`: Comma-separated list of steps to run (e.g. `--only footprint,paths,player-release`).
- `--skip-step <steps>`: Comma-separated list of step names to skip.
- `--filter <filter ...>`: Test name filters passed to the `round` test step.
- `--extra-skip <test>`: Additional test names or prefixes to skip in the `full` step (can be specified multiple times).
- `--release`: Run the final release build step with static CRT.
- `--dry-run`: Print commands and environment without executing anything.
- `--lock-wait <min>`: Lock wait timeout in minutes (default: 60).
- `--timeout <min>`: Per-step execution timeout in minutes (default: 60).
- `--root <path>`: Repository root path (default: current working directory).

### Progress Output

The build lock belongs to `run_gates.py`: never make `cargo-build.lock` by hand. An empty lock folder (no `pid-*.token` inside) that is over five minutes old while no `cargo` or `rustc` process is running is treated as left behind by a dead holder and removed, so a hand-made lock held across a quiet stretch would be taken away and two builds could collide.

While waiting for the Cargo build lock, `Waiting for cargo lock` messages include the holder PID from its token file when one is present. While any step runs, a flushed progress line is printed every 60 seconds showing the step name, elapsed minutes, and—for `cargo test` steps—the number of `test ... ok/FAILED/ignored` lines seen so far; full child output continues to be written to each step's log file.

### Shared target directory: stale-binary guard

All worktrees build into one `CARGO_TARGET_DIR`. Cargo's fingerprint for the `eqoa` unit is
the same in every worktree, so cargo only compares this worktree's source mtimes with the time
of the last build by *any* worktree. If another worktree built after your last edit, `cargo test`
prints `Finished ... in 0.87s` and runs the other worktree's binary, so a gate can pass on code
that is not yours. Never trust a "Finished in under a second" test run; use `run_gates.py`.

Inside the cargo lock, before each cargo step, `run_gates.py` therefore:

1. Touches every workspace crate's root source (`crates/*/src/main.rs`, `lib.rs`, `build.rs`) so
   cargo sees this worktree as newer than any shared artifact. It skips the touch only when
   `<target>/gates/last-build.json` shows this worktree built last and no cargo dep-info file in
   the target dir is newer than that record (so consecutive steps of one run do not rebuild).
2. After a test or release step, parses the `Running ... (path)` lines (or the release exe path)
   and requires every executable to be newer than the touch. Otherwise the step FAILS with
   `STALE BINARY CHECK FAILED` in its log and a `**STALE BINARY**` line in the summary. A test
   step that reports no executable also fails.
3. Lists each tested executable with its modification time in the summary
   (`Tested executable: ...`).

`make_owner_test_package.py --build` always touches (it also reruns `build.rs`, so the stamped
`EQOA_BUILD_COMMIT` comes from this tree) and refuses a release exe that was not rebuilt.

The cost: the first cargo step after another worktree built recompiles the workspace crates.

---

### The frozen `eqoa/1` record (`tools/eqoa1_freeze.py`)

`docs/modding/eqoa-1-frozen.json` lists every `supported` path (names only). It is separate from the regenerated `eqoa-1-surface.json`, so regenerating the reference never
moves it. Test `eqoa1_frozen_surface_only_grows` (needs a graphics adapter, like
`round7b_support_table_matches_the_running_client`; it prints a SKIP only where that one does) fails when a
recorded path is removed or drops below `supported`; additions pass (a frozen leaf may gain children). The
failure names the path with its before and after status.

**What the guard does not cover.** It guards names and their `supported` status only. Not covered: a supported
function whose body starts refusing while its registration stays; a record field no longer projected; an event
no longer emitted; a changed argument or return shape; names whose status comes from presence rather than a
registration (the Forge and UiForge layers, `ImGui.*`/`eqoa.imgui.*`, `bit.*`, and the hand check in
`annotate_host` in `native.rs`; round 7 H handles status-follows-registration). The freeze rests on three
things: this guard for names; the round 7C tests for fields and events
(`round7c_play_records_match_declared_fields`, `round7c_supported_event_inventory_has_fixture_coverage`,
`round7c_host_service_events_reach_lua_from_their_producers`, `round7c_world_events_come_from_play_changes`,
`round7c_item_records_and_window_events_use_projected_native_state`); and the spec plus reviews for shapes and
meanings.

When the surface grows, regenerate `eqoa-1-surface.json` (above), then add the new names:

```powershell
python tools/eqoa1_freeze.py update            # one command; creates the record if missing; only adds, and leaves the file alone when nothing is new; exit 1 and nothing written if it would remove or lower anything
python tools/eqoa1_freeze.py update --allow-downgrade   # only with the owner's approval and a player note
```

Options: `--surface FILE`, `--record FILE`. Tests: `tools/test_eqoa1_freeze.py`.

## 2. Bundled Mod Manifest Updater (`tools/update_bundled_manifest.py`)

Maintains exact SHA-256 checksums and file lists for bundled mods across three files:
- `bundled-mods/SOURCE.txt`
- `tools/build_player_release.py` (`BUNDLED_MOD_FILES`)
- `tools/test_player_release.py` (`BUNDLED_MOD_FILES`)

### Inclusion and Scanning Rules

Scans only files tracked by Git (`git ls-files bundled-mods`) plus untracked files under an existing mod or shared resource directory that are not git-ignored. Only runtime mod asset types matching the existing allowlist and `crates/eqoa/src/fork_ext/bundled.rs` are included:
- Extensions: `.json`, `.lua`, `.md`, `.png`, `.ttf`, `.txt`.
- Stray root files (such as `bundled-mods/SOURCE.txt` itself), dotfiles, and editor temporary/backup files (e.g. `.tmp`, `.bak`, `~`, `#*#`) are ignored.
- File bytes and line endings (`\r\n` vs `\n`) are preserved byte-identically, and any trailing text following the checksum table in `SOURCE.txt` is preserved.

### Command

```bash
python tools/update_bundled_manifest.py [options]
```

### Options

- `--check`: Verify manifests without modifying files on disk. Exits with code `1` and prints unified diffs if any manifest is out of date; exits `0` if up to date.
- `--root <path>`: Repository root path (default: current working directory).

---

## 3. Owner Test Packager (`tools/make_owner_test_package.py`)

Assembles a self-contained test deployment directory for owner playtesting. The resulting folder layout contains:
- `config/`: Copy of local player configuration (from `%APPDATA%/eqoa-rs`).
- `play/`: Extracted release build assets from `build_player_release.py`.
- `Play EQOA {name} Test.bat`: Standalone launcher targeting `config/` with updater disabled.
- `Play EQOA {name} Test (fps log).bat`: Launcher enabling `EQOA_PERF=1`.

### Command

```bash
python tools/make_owner_test_package.py --build [options]
```

Or package an already built executable:

```bash
python tools/make_owner_test_package.py --exe <path/to/release/eqoa.exe> [options]
```

### Options

- `--build`: Build `eqoa.exe` via `cargo build --locked --release -p eqoa` (using `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared`, `EQOA_BUILD_COMMIT=<HEAD>`, `EQOA_UPDATE_CHANNEL=<channel>`, `RUSTFLAGS=-C target-feature=+crt-static`, and the shared `CargoLock`) before packaging. Refuses to run when tracked files have uncommitted changes. Mutually exclusive with `--exe` (one of `--build` or `--exe` is required).
- `--exe <path>`: Path to a prebuilt release executable (mutually exclusive with `--build`).
- `--name <label>`: Package and launcher name (default: `Round5`). Used in folder name (`EQOA-Enhanced-{name}-Test`) and launcher files.
- `--channel <name>`: Release channel (default: `enhanced-dev`).
- `--output <path>`: Override package destination directory (default: `<Downloads>/EQOA-Enhanced-{name}-Test`).
- `--catalog <path>`: Optional local entity database JSON path.
- `--commit <hash>`: Optional Git commit assertion passed to `build_player_release.py`.
- `--version <ver>`: Optional version string assertion passed to `build_player_release.py`.
- `--force`: Replace `play/` and batch files in an existing output folder. Preserves existing `config/` unless `--fresh-config` is also specified.
- `--fresh-config`: Overwrite existing `config/` when `--force` is used.
- `--root <path>`: Repository root path.

---

## 4. Local Loot Scene (`tools/local_scene.py`)

Run on Windows with the local test-server installation and a Frontiers disc image.

```powershell
python -B tools/local_scene.py
```

The runner uses the main checkout's `run/eqoa.iso` by default. Set `EQOA_ISO` or pass
`--iso <path>` to use another image. `--server-dir <directory>` selects a local server
installation. It must contain `eqoa-server/gameserver.exe`, its seeded `eqoa.db` and
`config.yaml`, and `accounts.local.txt` with the `lsgm` GM entry and its `Lsgm` character
in the database. `--host <IPv4>` overrides the installation's `bind_address` if the PC's
LAN address has changed. Stop any local server already listening on auth TCP 9735 or game
UDP 10070 before running the scene.

The runner snapshots the seed database for each run and suppresses the welcome dialog
for the test character in that copy. It starts the installed server binary against the
copy, selects the default addon profile in a scratch `EQOA_CONFIG`, and launches the
shared release client offscreen. If the shared release binary is missing, it builds one
with the `run_gates.py` Cargo lock. An existing shared binary is reused, including one
built from a different branch. The password is read at run time and passed to the client
through `EQOA_PASSWORD`.

The GM spawns and kills a fire bat with `#spawn fire bat` and `#kill`. `EQOA_SAY` sends
these as ordinary chat and the server recognizes the `#` prefix. The client setting
`commands to the server false` affects forwarding of unrecognized slash commands.
The game keyboard opens the corpse and takes its first item. A passing run confirms
the world entry and GM replies, the `0x0018` corpse list, the native item request and
item receipt, the server's `0x0016` close and `0x0018` refresh, and the in-world screenshot.

Each run leaves `client.log`, `server.log`, `corpse.png`, `scene.png`, and `summary.txt`
in a timestamped folder under the server installation's `runs/` directory. Failed runs
keep the evidence produced before the failure and exit with a nonzero status. The runner
stops its own server and discards the database copy on exit. This scene is run by the
lead. The parsing tests can run independently with `python -B tools/test_local_scene.py`.

### Bag Viewer scene

Build a release client with the scripted addon input changes, then run either scene.

```powershell
python -B tools/local_scene.py --scene bag-viewer
python -B tools/local_scene.py --scene bag-viewer --r65 --exe <round-6.5-release-exe>
```

The scratch profile enables the bundled Bag Viewer, binds **B** to open it, and marks
torn fire bat wings as junk. After the GM kills the bat, the develop scene uses the
game's loot key on the scripted target. It opens the Bag Viewer's **Corpse** tab and
clicks **Take all but junk**. The runner checks the client's loot receipt, reads the
Bag Viewer's own loot log through the scene diagnostic, and compares
the disposable database before and after the run to verify that meat entered the bag
and wings were left behind. `scene.png` captures the Bag Viewer's loot log.

`--r65` uses Bag Viewer's **Open nearest corpse** and requires the `takeover:` lines
for the open request, hidden loot window, and hidden session close. The round 6.5 binary
must also include the scripted addon input changes. `--exe <path>` selects a specific
build if the executable in the shared Cargo directory is from another branch.

### Addon input replay

- `EQOA_MOUSE` accepts `click X Y@ms`, `rclick @creature name@ms`, `wheel N@ms`,
  `press l|r@ms`, `release l|r@ms`, `move DX DY@ms`, `drag DX DY@ms`,
  `ctrl down|up@ms`, and `linkclick [index]@ms`. In the
  native window these use the same mouse event dispatcher, pointer claims, and ImGui
  handlers as a physical mouse. `X Y` is measured in screenshot pixels when offscreen.
- `EQOA_MOD_KEYS` accepts `b@ms`, `enter@ms`, and `text abc@ms`. A named key is
  held for 100 ms. Text types one character every 80 ms. Keys pass through the addon
  event and capture rules and reach the game when unclaimed.
- Separate steps with semicolons. `EQOA_KEYS` remains the pregame script, and
  `EQOA_PLAY_KEYS` and `EQOA_CLICKS` remain direct game scripts. `playshot` keeps
  its original `EQOA_MOUSE` path.

In the Bag Viewer probe, the scratch profile enabled the addon and a scratch copy of its
Lua opened the Corpse tab when loot arrived. The screenshots before and after a click on
**Take all** both showed three items, and the client logged no loot request. `play` uses
`EQOA_CLICKS` to call the native game's click handler directly. `EQOA_MOUSE` is read by
the separate shot path. The addon receives pointer events through the window event handler.

---

## 5. CI Branch Gates (`tools/ci_gates.py`, `.github/workflows/branch-gates.yml`)

Runs the long test gates on GitHub's free Windows runners, so the owner's PC (one build slot,
about 40 minutes for a full run) stays free. Use it for full runs when the PC's build slot is
busy; disc tests, GPU tests and the release build stay on the PC.

```bash
python tools/ci_gates.py push <local-branch>      # pushes it as ci/<branch> to looprook, prints the run URL
python tools/ci_gates.py status <local-branch>    # waits, then prints pass/fail per gate, SKIPs, failing tests
python tools/ci_gates.py cleanup <local-branch>   # deletes the remote ci/<branch>
python tools/ci_gates.py cleanup --merged         # deletes every remote ci/* already merged into develop
```

`status` exits 0 only when every gate passed (`--no-wait` prints the current state; `--timeout`
and `--poll` set the wait). `cleanup --dry-run` shows what would be deleted.

**Safety.** The tool writes only `refs/heads/ci/*`: it builds the destination itself and refuses
every other ref (tags, `develop`, `enhanced`, `master`, refspecs, odd characters) before it runs
git. The push is forced, but only on a `ci/` ref. The workflow starts on `ci/**` pushes (and
`workflow_dispatch` with a `ref` input), has a read-only token, no secrets, no release, no tag
and no push, and a newer push to the same `ci/` branch cancels the older run. Delete each `ci/`
branch when its work merges (work branches may sit on the public repository only for CI).

**What runs in CI** (two parallel jobs on `windows-2022`, toolchain 1.99.1, no disc):

| Job | Gate | Same as |
|---|---|---|
| tests | `addons`: `fork_ext::addons::` tests | `run_gates.py` addons |
| tests | `full`: whole workspace, `--test-threads=1`, skipping the GPU-bound upstream AO test | `enhanced-dev.yml` |
| checks | `wasm`: `cargo check` for `wasm32-unknown-unknown` | `run_gates.py` wasm |
| checks | `footprint`, `paths`, `player-release` | `run_gates.py` |
| checks | `tools`: the Python tool tests | `enhanced-dev.yml` |

Each gate runs even when an earlier one failed. The job summary and the `gate-summary-*`
artifact (kept 7 days) give pass/fail/ignored counts, the number of `SKIP` lines and the
failing test names. Cargo output is cached with `Swatinem/rust-cache`; a cache belongs to its
`ci/` branch, so the first run of a new branch is cold and repeat pushes are warm.

**What still needs the owner's PC.** Tests that need the owner's Frontiers disc (about 200 skip
in CI and are counted as SKIP), GPU and window tests, anything needing the local server or a
live play session (`local_scene.py`), the optimized zero-addon allocation gate and the release
build for owner test packages (`enhanced-dev.yml` still runs both when `develop` is pushed), and
the oracle tests (not on this machine at all).

---

## 6. Upstream PR tool (`tools/upstream_pr.py`)

Cuts and re-cuts a LOCAL branch `upstream-pr/<name>` from `origin/master` in its own worktree (default:
`upstream-pr-worktrees/<name>` beside the repository, never inside it or the main checkout). It has no push
option and refuses any branch not named `upstream-pr/...`. Pushing and opening the PR stay the owner's call.

```bash
python tools/upstream_pr.py cut   <name> --spec <spec.json> [--worktree DIR] [--base origin/master]
python tools/upstream_pr.py check <name> [--spec <spec.json>] [--base origin/master]   # changes nothing
python tools/upstream_pr.py test  <name> [--print-only]
```

`cut` creates or resets the branch (a re-cut discards the branch's earlier work), applies the spec, stages
the changed files by name, runs the checks and commits. A refusal prints `file:line: refused, reason`, leaves
the branch at the base and commits nothing. `check` runs the same checks on `<base>...upstream-pr/<name>`.

**Spec** (JSON; all paths relative to the repository root, `patches` relative to the spec file):

```json
{
  "message": "fix: refused login shows its reason",
  "allowed_files": ["crates/eqoa/src/pregame/station/api.rs", "crates/eqoa/src/pregame/station/"],
  "checkout": {"ref": "develop", "paths": ["crates/eqoa/src/pregame/station/api.rs"]},
  "patches": ["station-fix.patch"],
  "commits": ["<sha>"],
  "allow_names": []
}
```

`message` and `allowed_files` are required. `allowed_files` takes exact paths, globs, or a folder ending in `/`.
The change set comes from any of `checkout` (files taken whole from a ref), `patches` (`git apply`) and
`commits` (`cherry-pick -n`), applied in that order. `allow_names` lists fork-name terms (e.g. `lua`, `mlua`)
a mod-host PR may carry; PRs 0 to 3 leave it empty.

**Refusals:** a file the spec does not name; game data (`run/`, `Testing Resources/`, disc images, keys,
decrypted modules, `credentials`); formatting-only hunks (whitespace-only, lines reordered, blank lines added);
and these names in an added line, case-insensitively: `fork_`, `forge_`, `enhanced`, `eqoa_enhanced`,
`looprook`, `frontiersforge`, `fork_ext`, `mlua`, `lua`.

**`test`** runs `cargo test --locked --workspace -- --test-threads=1` in the branch's worktree while holding
the shared cargo lock (`run_gates.CargoLock`) with `CARGO_TARGET_DIR` set to the shared folder
(`~/Paseo/targets/shared` unless the variable is set); it refuses a target folder inside the worktree.
`--print-only` shows the command without running it. Upstream's guide names no single test command, so this is
the workspace default; add the oracle and disc environment as a lead needs, and count SKIPs in the report.

Tests: `cd tools && python -m unittest test_upstream_pr` (builds its own temporary git repo; no cargo).
