# Tab after an empty Enter: fix report (2026-11-09)

Branch `fix/tab-opens-chat`, worktree `tab-opens-chat`, base `develop` at `5346797`
(round 2 merge). Scout findings in `docs/plans/eqoa-api/tab-chat-scout.md` were
verified against this tree before changing code.

## What the player sees

Tab normally picks the next target. After Enter opens the Wereoxxs chat and a
second, empty Enter closes its input, the next Tab both picks a target and puts
the chat input back in focus. Casting or opening a menu clears it until the
sequence is repeated.

## Verified mechanism

1. `bundled-mods/wereoxxs_ui/wereoxxs_ui.lua:838-855` claims Enter, suppresses
   it, and sets `chat_state.focus_requested = true` on every Enter-down.
   Text capture follows `chat_state.input_active` (line 849).
2. `bundled-mods/wereoxxs_ui/modules/chat_window.lua:212-215` turns that request
   into `ImGui.SetKeyboardFocusHere()`; the input at line 221 uses
   `EnterReturnsTrue`, so an empty submit releases the field.
3. Dear ImGui starts Tab navigation even without `NavEnableKeyboard`
   (`imgui.cpp`, `NavUpdateCreateTabbingRequest`, around lines 15032-15043), and
   its tab stops include inputable items (`NavProcessItemForTabbingRequest`,
   around line 14377). After the empty Enter, `ActiveId` is 0, so a Tab press
   starts from null focus and can land on the chat input.
4. `want_capture_keyboard` is only true while `ActiveId != 0`, a modal is open,
   or keyboard nav is enabled (`imgui.cpp`, around lines 5742-5745). The winit
   backend queues key events without changing that value synchronously.
5. `crates/eqoa/src/fork_ext/addons/runtime.rs:180-215` used to hand every event
   to the ImGui platform first, then read last frame's `want_capture_keyboard`.
   With the field closed that value was false, so the same Tab press was also
   allowed through to gameplay. `crates/eqoa/src/viewer.rs:4317-4328` then
   handled it as a target cycle.
6. The client's own chat path is not involved: an empty submit clears `mode`
   (`crates/eqoa/src/play.rs:6455-6458`).

The scout's line numbers were accurate; the only correction is that forwarding
does not change the capture value in the same event, so the ordering itself is
not the bug. The bug is that an unowned Tab was forwarded to ImGui at all.

## Fix (fork-owned only)

- `crates/eqoa/src/fork_ext/addons/input.rs:236-258` adds
  `forwards_key_to_imgui`. It gates **Tab presses only**: an addon-focused
  widget, active text capture, or a claimed/suppressed Tab in plain play still
  forwards Tab; every other key keeps the old routing. Tab releases always
  pass, so ImGui can never keep a key down that it saw pressed.
- `crates/eqoa/src/fork_ext/addons/runtime.rs:88-97` makes the decision before
  the platform call, and `runtime.rs:180-215` skips the ImGui hand-off for a Tab
  press no addon owns. Non-keyboard events and all other keys are forwarded
  exactly as before.
- No bundled Lua was changed. No upstream/shared file was edited, so there is
  no footprint budget change.

## Tests

Added in `crates/eqoa/src/fork_ext/addons/runtime.rs` (headless ImGui context,
no GPU):

- `forge_runtime_empty_chat_enter_then_tab_targets_without_refocusing` replays
  Enter down/up with the mod's focus request, an empty Enter, then Tab. It
  asserts the Tab is not forwarded, `Capture` gives the press to `Game`, and the
  chat input is still unfocused. The negative control replays the same sequence
  with the old unconditional hand-off and asserts the chat input *does* take
  focus, so the test fails on the old code.
- `forge_runtime_focused_chat_input_still_receives_tab_and_typing` opens the
  field, types a character, and verifies Tab is still forwarded while the field
  owns the keyboard and the field stays focused.

## Validation (CI-equivalent)

All run from the worktree with `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/tab-opens-chat`
and the shared `%USERPROFILE%/Paseo/cargo-build.lock` held for cargo commands.

- `python -X utf8 tools/check_fork_footprint.py --base 8435f2c8a00c631d716771f8189fa0011a1fbf50`
  - **PASS**. Shared budgets unchanged; only fork-owned files changed.
- `python -X utf8 -m unittest discover -s tools -p "test_*.py"` - **94 tests, OK**.
- `cargo test --locked --workspace --target x86_64-pc-windows-msvc -- --test-threads=1`
  with scratch `EQOA_CONFIG` (as `enhanced-dev.yml` does) - **all crates passed**;
  the `eqoa` binary reports 1668 passed, 0 failed, 26 ignored. The 26 ignored are
  the suite's existing marks; I did not reclassify them.
- `cargo build --release --locked -p eqoa --target x86_64-pc-windows-msvc` -
  **built** (existing warnings only).
- `rustfmt --edition 2024 --check` on both touched files - clean.

Oracle and disc tests cannot run on this machine (the decomp, eqoa-port's oracle
and the 2002 disc are not present), as the repository's rules already state.

## Not verified without the live game

- Exact live timing between ImGui's frame boundary and event delivery. The test
  adds the one settle frame the game loop normally has before the player's Tab;
  a Tab arriving in the same sub-frame as the closing Enter is outside the
  reported sequence and was not modelled.
- The second Enter in the test releases focus without asking for it again,
  because that is the state the player reports. The bundled mod also sets
  `focus_requested` on every Enter-down, so a live run should confirm whether a
  focus request from the closing Enter can re-open the field by itself. If it
  does, that is a separate (bundled-mod) behaviour, not this routing bug.
- No live play-test or GPU capture was possible here; the owner will play-test.
- A mod that polls ImGui key state globally for Tab (rather than using the
  `Input` API or its own text field) would now miss an unclaimed Tab press. No
  bundled mod does this; third-party mods should claim the key through `Input`
  if they need it.
