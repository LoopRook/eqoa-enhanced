# Tab opens chat after an empty Enter: scout findings (2026-11-09)

Read-only trace by DeepSeek Go; the lead has not yet verified the line references. The bug exists on
develop before the round 2 merge (413d1ee): `bundled-mods/wereoxxs_ui` is unchanged since.

1. **Likely cause: Wereoxxs UI plus one-frame-late key routing, not upstream's chat code.**
   `wereoxxs_ui.lua:838-855` claims Enter and sets `focus_requested` on every Enter-down; that makes
   `chat_window.lua:212-215` call `ImGui.SetKeyboardFocusHere()`, which sets ImGui's `NavWindow`.
2. ImGui's Tab navigation is active even without `NavEnableKeyboard` and stops on inputable items, so
   after Enter, Enter the next Tab activates the chat `InputText` (`chat_window.lua:221`).
3. The same Tab also reaches the game: `runtime.rs:167-176` feeds the event to ImGui, then reads
   `want_capture_keyboard()` (last frame's value; ActiveId was 0 after the empty close), so
   `wants_key_capture` (`runtime.rs:209-210`) is false and the game targets (`viewer.rs:4319-4327`)
   while ImGui focuses chat in the same press.
4. The client's own chat path is clean: Enter opens (`viewer.rs:4108-4110`), an empty submit clears
   `mode` (`play.rs:6455-6458`), fork text capture clears with a 1 s lapse (`input.rs:173-198`).
5. **Smallest fix:** in `Runtime::event` (`runtime.rs:196-216`) decide `wants_key_capture` for
   `KeyboardInput` before `handle_window_event`, and do not forward the key to ImGui when the mod does
   not capture it. Alternative: expose `ImGui.PushItemFlag`/`NoTabStop` and wrap the chat `InputText`.
6. Risk to check: other mods that rely on ImGui seeing Tab or other keys when nothing is captured.
