# Forge compatibility handoff

The one current handoff for the native addon runtime (FrontiersForge compatibility). It merges the
Forge handoff of 2026-10-29 and the Wereoxxs overnight handoffs of 2026-10-30, and adds the owner's
live test of 2026-10-30. Earlier versions: `git show 039bbff:docs/FORGE-COMPATIBILITY-HANDOFF.md`
(stages up to sound) and `docs/WEREOXXS-{FINAL-HANDOFF,MORNING-HANDOFF,OVERNIGHT-STATUS}.md`
(history of the overnight run; their stage-by-stage detail stays there).

## Where the work is

- Branch `forge-compatibility`, worktree `C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility`
  (Paseo workspace `wks_12cecc2d0898f767`). It now contains the Wereoxxs overnight work (fast-forwarded
  from `forge-wereoxxs-overnight-20261030`, which is retired) and is merged into `develop`.
- `develop` holds the same work. `enhanced` (stable, released 0.1.1) has none of it. Nothing is pushed.
- Footprint base: `7912c7393df7d85655d5816980fae1217cc48a32` until a reviewed upstream sync.
- Unchanged reference packages (never edit them): main checkout `run/forge-compat-reference/`, pinned
  FrontiersForge `4026d5c` (v0.3.2), UiForge `6f2d125`, sol2 bindings `ec6f9ef`. Use the real
  `sol_ImGui.h`/C++ implementations as the reference, not the IDE Lua stubs. ImGui pinned at
  `37b7a7a` (1.92.7 WIP), header in `run/forge-preview-prep/imgui-37b7a7a9.h`.

## The goal and its gate

The first public dev build ships when **`wereoxxs_ui` works in full in a live game**: independently
placed hotbars with icons and visible cooldowns, casting bar, player/target/group frames, effects,
compass, chat and damage meter, its settings/profile, enable/disable/reload, and each "hide the game's
X" option. Loading the package, or showing only its hotbars, does not meet it. The owner's live test
is the acceptance; tests passing are not.

Later: `avoids_modern_ui` and `arpg_ui` (do not block the dev build), then all seven required packages
(`ff_example`, `screen_grid`, `uiforge_example`, `retro_health_hearts`, `arpg_ui`, `avoids_modern_ui`,
`wereoxxs_ui`) for the release gate. Advertise only what is verified.

## Owner decisions that stand

- Native maps stay the defaults; Forge `world_map`/`mini_map` are optional proofs, never blockers.
- Run unchanged addon scripts through native high-level adapters. No emulator memory, FFI pointers,
  EEmem or fake MIPS hooks. An unsupported call is visible and actionable (diagnostic); nil/false only
  where the source's semantics call for it.
- The HD texture pack (`Testing Resources/eqoa-hd-pack-v2.7.zip`, ~2.2 GB) is a separate deferred
  feature: do not extract or stage it.
- Updater notices and minimap route guidance are deferred (`docs/backlog.md`). No updater changes here.
- Runtime code goes in `crates/eqoa/src/fork_ext/addons/`; shared files only get hooks within budget.
  Player notes go in `FORK-UPDATE-NOTES.md`.

## What exists

Native-only mlua 0.12.2 (vendored Lua 5.1) and dear-imgui 0.18 on the game's WGPU backend. One VM per
addon with limits (8 MiB heap, 200k instructions a top-level run, 4,096 bridge calls). Discovery,
packages, PNG textures, lifecycle manager, health/hearts, common ImGui widgets, DrawList (window and
background, AddText/AddImage/clip rects, geometry budget scaled by radius), stdlib subset (bit, os.clock/
date, pcall; no coroutine/io/debug), Windows MCI sound, queued fonts, and the overnight adapters:
`frontiers_forge.player`, `.ui` (hide claims), `.group`, `.entity`, `.entity_list`, `.ability_bar`,
`.ability_list`, `.ability`, `.icon`, `.chat`, `.combat`, `.input`. Outgoing addon chat is sent through
`play.chat_line`. Errors are appended to `mods/addons_error.log`. Claimed: 185 addon tests pass; the
reviews were Gemini Flash reviewing Gemini Flash.

## Owner's live test, 2026-10-30 (after c769a71)

| Item | Result |
|---|---|
| Typing in the Wereoxxs chat sends | **Works** |
| Hide the game's health | Works |
| Hide the game's ability bars | **Fails** |
| Hide active effects | **Fails** |
| Hide the game's compass | **Fails** |
| Hide the game's chat window | **Fails**: hides the target nameplate instead; chat stays |
| Hide group / pet panels | Not tested |
| Icons | **None** (see below) |

### Why the hides fail (scout, DeepSeek, file:line verified 2026-10-30)

Health works because it is gated at its draw site (`fork_hud::health_visible`, `fork_ext/hud.rs:5`).
Everything else goes through `should_suppress_quad` (`fork_ext/hud.rs:89-129`), which drops HUD quads
whose bounding box falls inside hard-coded 640x448 boxes. That design is the bug:

- Ability bars (`hud_menus.draw`, `play.rs:7683`) and active effects (`influence::draw`,
  `play.rs:7552`) have no box at all, and both are drawn after the filter runs.
- Compass (`draw_compass`, `hud.rs:475-495`) is a rotated quad, so its box exceeds x 10-70 at most
  headings; on a widescreen window `Anchor::Left` shifts it (`render/src/ui.rs:398-401`) and it never
  matches.
- Chat's box (y 325-460, x <= 220) matches the target block (`TARGET_AT=[20,348]`, `hud.rs:177`); the
  real chat window (`wnd_chat.draw`, `play.rs:7556-7561`, anchored right) is drawn later, elsewhere.
- Group bars start at y 93 (`GROUP_AT`, `hud.rs:176`), above the box's 100: expect group hide to fail.
- Claim names with no consumer: `hotbar_draw_compact`/`_expanded`, `active_effects`, `chat_window`,
  `group_compass_marks` (`addons/visibility.rs`).

**Fix direction:** delete the bounding-box filter; gate each element at its draw site with a
`fork_hud::*_visible` hook as health does (compass, target, group, pet, chat window, ability bars,
effects, group compass marks), each a one-line hook in the shared file within budget. Test each with
a negative control (the element draws when unclaimed, not when claimed), and at a widescreen size.

### Icons

- Ability and effect icons: `Icon.GetTexture` is a stub returning nil. Bridge the game's own icon art
  (`vui.blit_ability_icon` / the HUD's texture sheets, `crates/vi/src/ui.rs`, `crates/eqoa/src/ui.rs`)
  into addon texture handles.
- `gear-icon.png`: exists in the reference (`scripts/resources/`), but the test deployment copied only
  `mods/wereoxxs_ui/`, not the shared `mods/resources/` beside it. A packaging fix, plus a test that
  the deployed layout resolves it.
- `class_icons/<class>.png`: optional overrides that the package probes for and falls back from (a
  drawn badge). The miss is correct, but it is shown to the player as an error; make an optional
  probe's miss quiet (log only) if Forge treats it so.

## Next, in order

1. Hide claims at the draw sites (above). Owner retests all hide options, including group and pet.
2. Ability/effect icons through `Icon.GetTexture`, and the resources packaging fix.
3. Remaining live checks: cooldowns, casting bar, D-pad bar rotation on a real controller, damage meter,
   settings/profile save and load, enable/disable/reload.
4. Build a preview package from `develop` for the owner (the last delivered preview, in
   `run/forge-compatibility-preview/21984f39e488/`, predates all of this; never overwrite it).

## Verifying

From the Forge worktree, one cargo build at a time on the machine:

```powershell
$env:CARGO_TARGET_DIR='C:/Users/hocka/.paseo/worktrees/1usktejc/forge-compatibility/target'
cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1
cargo check --locked -p eqoa --target wasm32-unknown-unknown
python tools/check_fork_footprint.py --base 7912c7393df7d85655d5816980fae1217cc48a32
& "$env:CARGO_TARGET_DIR\release\eqoa.exe" "C:\Users\hocka\Documents\PS2 Games\EverQuest - Online Adventures - Frontiers (USA).iso"
```

Four Windows symlink-privilege tests skip (error 1314); count them. GPU, disc and source fixtures can
log SKIP and still return Ok: check the output. The full workspace suite includes a slow exhaustive EE
test; run it only with cause. Some older reports were saved as CP1252: keep files UTF-8.
