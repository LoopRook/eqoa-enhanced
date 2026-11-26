# Agent guide: building a mod from an owner brief

One page for an AI agent. The owner describes what they want to see in the game; you build a mod.

## 1. Before you write anything

1. Restate the brief as what the player will see, in five lines or fewer. Wait for the owner's yes if the
   mod adds a window or changes the HUD.
2. Pick the closest bundled mod from [EXAMPLES.md](EXAMPLES.md) and read its `main.lua`.
3. List the API areas you need and look each up in [EQOA-API-SPEC.md](EQOA-API-SPEC.md). Then check the
   generated [SUPPORT-TABLE.md](SUPPORT-TABLE.md) (or ask the running client with
   `eqoa.capabilities.get(path)`). Treat `not_yet` and `unsupported` as "cannot be built"; tell the owner
   rather than faking it. Never invent function names. Prefer proven names: those on the provisional list
   ([PROVISIONAL.md](PROVISIONAL.md)) work but no shipped mod uses them yet, so they may still be corrected
   after the final freeze.

## 2. Start from the template

Copy [template/](template/). Change `id` (unique, lowercase), `name`, and the `capabilities` list to the
features you use. Keep `"api": "eqoa/1"` and `"entryMode": "once"`. Every manifest field is in the
reference, section 2.

## 3. Rules that prevent most bugs

- Draw only inside `eqoa.events.on("render", ...)` (once mode). Register each handler once per enable.
- Queries return `value, nil` or `nil, Error`. Handle both; `nil, nil` means "nothing there", not failure.
- Absent is not zero: no HP data is `nil`, not `0`.
- Indexes start at 1. Times are seconds, headings radians. Do not use Forge's conventions.
- References (`ref`) expire on zoning or when the thing is gone. Reacquire after `state.reset`.
- Pair every `Begin`/`End`. Release every claim (`hud.hide`, input claims) on `disabling`,
  `world.leaving` and `state.reset`.
- Settings: plain numbers, strings, booleans and tables only, through `eqoa.settings.on_save/on_load`.
- Game actions (`eqoa.actions.*`) return a request, not a result. Listen for `action.result` and read
  its `state`. Every action needs its capability in `mod.json` (`eqoa.actions.<name>`, or `eqoa.actions.*`
  for all); a chat line starting with `/` needs `eqoa.actions.command`, any other `eqoa.actions.chat`;
  window takeover needs `eqoa.windows.takeover`, the map waypoint `eqoa.map.waypoint`. Declare the fewest
  you need: the player sees the list before enabling the mod, and widening it later disables the mod until
  the player accepts again.
- Keys: a key is its full name (`ctrl+u` is not `u`). Give defaults the game does not use; a default that
  is taken leaves the row empty. F3, F11, F12, Escape and the clipboard keys can never be mod keys.
- Limits: 8 MiB of Lua memory and 4 million instructions per frame for all of the mod's handlers. No
  network, no processes, no `load`, no FFI; files only through `eqoa.files` (or `io.open`) in the mod's
  private area. Do not loop over everything every frame; cache and use events.
- Do not use Forge APIs in new work.

## 4. Check your work

- Load it: copy the folder to the mods folder, open Mods & Addons (Shift+F10 in the world), Rescan folder,
  Enable. Errors and unsupported calls show there (`[Warnings: N]`) and in `mods/addons_error.log`. Zero
  warnings is the bar; an `unknown_symbol` diagnostic is a misspelt name.
- Toggle it off and on; zone; log out and in. It must release everything and not crash.
- You cannot play the game for the owner. Report what you ran and what you could not: "loaded with no
  errors; not seen in the live game" is an honest report.

## 5. Report in a player's terms

What the owner will see, where to turn it on, what it does when something is missing (no target, no data),
and what was and was not tested. Do not paste API names at them.

## Never

Commit game data, disc images or local paths. Claim a feature works because it loads. Rely on a function the
capability query calls `not_yet`.
