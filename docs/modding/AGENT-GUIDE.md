# Agent guide: building a mod from an owner brief

One page for an AI agent. The owner describes what they want to see in the game; you build a mod.

## 1. Before you write anything

1. Restate the brief as what the player will see, in five lines or fewer. Wait for the owner's yes if the
   mod adds a window or changes the HUD.
2. Pick the closest bundled mod from [EXAMPLES.md](EXAMPLES.md) and read its `main.lua`.
3. List the API areas you need and look each up in [EQOA-API-SPEC.md](EQOA-API-SPEC.md). Then check the
   running client, not the page: `eqoa.capabilities.get("eqoa.x.y")`. Treat `not_yet` and `unsupported`
   as "cannot be built"; tell the owner rather than faking it. Never invent function names.

## 2. Start from the template

Copy [template/](template/). Change `id` (unique, lowercase), `name`, and the `capabilities` list to the
features you use. Keep `"api": "eqoa/1"` and `"entryMode": "once"`.

## 3. Rules that prevent most bugs

- Draw only inside `eqoa.events.on("render", ...)` (once mode). Register each handler once per enable.
- Queries return `value, nil` or `nil, Error`. Handle both; `nil, nil` means "nothing there", not failure.
- Absent is not zero: no HP data is `nil`, not `0`.
- Indexes start at 1. Times are seconds, headings radians. Do not use Forge's conventions.
- References (`ref`) expire on zoning or when the thing is gone. Reacquire after `state.reset`.
- Pair every `Begin`/`End`. Release every claim (`hud.hide`, input claims) on `disabling`,
  `world.leaving` and `state.reset`.
- Settings: plain numbers, strings, booleans and tables only, through `eqoa.settings.on_save/on_load`.
- Game actions (`eqoa.actions.*`) return a request id, not a result. Listen for `action.result`. In
  round 6.5 an action needs a declared capability in `mod.json` (`eqoa.actions.*` or
  `eqoa.actions.<name>`; `eqoa.windows.takeover` for window takeover). Round 6.5 is not on every build yet:
  check `eqoa.capabilities` and the spec on the branch you target.
- Limits: 8 MiB Lua memory and 200,000 instructions per callback. No `io`, `os` file access, `load`, FFI
  or network. Do not loop over everything every frame; cache and use events.
- Do not use Forge APIs in new work.

## 4. Check your work

- Load it: copy the folder to the mods folder, rescan in the Addon Manager, enable. Errors and unsupported
  calls show there and in `mods/addons_error.log`. Zero errors and zero `unknown_symbol` diagnostics is the
  bar.
- Toggle it off and on; zone; log out and in. It must release everything and not crash.
- You cannot play the game for the owner. Report what you ran and what you could not: "loaded with no
  errors; not seen in the live game" is an honest report.

## 5. Report in a player's terms

What the owner will see, where to turn it on, what it does when something is missing (no target, no data),
and what was and was not tested. Do not paste API names at them.

## Never

Commit game data, disc images or local paths. Claim a feature works because it loads. Rely on a function the
capability query calls `not_yet`.
