# Versioning: what `eqoa/1` promises

Plain words for mod authors. The exact rules live in the spec (section 2).

**Status:** the promise below takes effect when `eqoa/1` is frozen (planned for the Stable release that
carries rounds 4 to 7). Until then the API is complete in shape but a rare correction can still happen, and
every one is listed in the release notes.

## What never changes under `eqoa/1`

For a mod that declares `"api": "eqoa/1"`, once frozen, none of these change:

- the name of a module, function, field or event;
- the type of a field or argument, its units (times in seconds, headings in radians, positions in game
  units) and its base (arrays and indexes start at 1);
- what an absent value means (a missing optional field is `nil`; zero HP is real, absent HP is not zero);
- the error codes a call can return, and whether it returns `nil, nil`, `nil, Error` or `value, nil`;
- the order events arrive in, and how long a reference (handle) stays valid;
- the default of an optional argument;
- the numbers of existing enums.

## What may be added (and does not break you)

- new functions, modules, events and capability names;
- new optional fields on records, new optional arguments, new enum values.

So your mod must **ignore** event fields and enum values it does not know, and must not assume a record has
only the fields you saw.

## `eqoa/2`

A change that would break any rule above can only ship as `eqoa/2`. It is chosen per mod in `mod.json`
(`"api": "eqoa/2"`), has its own `eqoa` table, and runs beside `eqoa/1` mods. Version 1 keeps working;
nothing is migrated for you. There is no `eqoa/2` today.

## Testing for features

- `eqoa.api` is `"eqoa/1"`. `eqoa.revision` is a whole number that goes up whenever something is added
  (it is 1 today). Use it to ask "is this client new enough?": `if eqoa.revision >= 2 then ... end`.
- For one function, field or event, ask directly. This has no side effects and writes no log:

```lua
if eqoa.capabilities.has("eqoa.effects.list") then
    local effects = eqoa.effects.list()
end
local c = eqoa.capabilities.get("eqoa.actions.cast")  -- c.status: supported | partial | not_yet | unsupported | unknown
```

- `not_yet`: planned, not built. `unsupported`: left out on purpose, or no backend on this platform.
  `unknown`: the name is not part of the API (check the spelling).
- `supported` means the function exists. It does not mean there is data right now: no player yet, a closed
  window, an unloaded asset all still return their normal empty answers.
- The `capabilities` list in `mod.json` tells the manager what your mod wants and is shown to the player.
  It is not a feature test. For game actions (and window takeover) the client requires you to declare them
  (planned in round 6.5; see GETTING-STARTED for what your build does).

## Forge mods

Mods written for FrontiersForge keep loading, and the compatibility is kept up. It is **not** part of the
`eqoa/1` promise: Forge names, units and index conventions are its own and follow Forge, not this page.

## If a name is marked supported but is not

The support table is generated from the code, but a mistake is possible. If `capabilities.has` says yes and
the call is really a stub (or the reverse), that is a bug in the client, not in your mod. It is corrected in
the next release by changing the answer to the truth (a stub becomes `not_yet` or `unsupported`) or by
building the function. The correction is written in the release notes. Report it with the path and the
`Capability` record the client gave you.
