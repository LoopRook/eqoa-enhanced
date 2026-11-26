# Versioning: what `eqoa/1` promises

Plain words for mod authors. The reference is [EQOA-API-SPEC.md](EQOA-API-SPEC.md).

**Status:** from Enhanced 0.7.0 `eqoa/1` is a stable candidate. The guard test described below is on and we
do not expect breaking changes. The promise takes full effect at the final freeze, which comes when the API
is offered to the upstream client and upstream has had its say. Until then a rare correction can still
happen, and every one is listed in the release notes. At the final freeze the names fall into two groups,
proven and provisional (next section); the promise below applies in full to the proven ones.

## Proven and provisional names

At the final freeze every `eqoa/1` name is either proven or provisional. The list of provisional names is
[PROVISIONAL.md](PROVISIONAL.md); the machine-readable form is
[eqoa-1-tiers.json](eqoa-1-tiers.json). A name that is not on the list is proven.

- **Proven** means a mod shipped with the client uses it, so it has been exercised in real play. A proven
  name gets the full promise on this page: it never changes and never disappears.
- **Provisional** means the name is covered by the client's automated tests but no shipped mod uses it
  yet. It works as documented, but it can still be **corrected** after the final freeze if real use shows
  a mistake in the name, a field, a result or a limit. Every correction to a provisional name is listed in
  the release notes, with the name; a correction that is not listed there is a bug in the client.
- **Graduating:** a provisional name becomes proven once a mod shipped with the client uses it. It is then
  promised in full, and a name never goes back from proven to provisional. If you want a provisional name
  to be promised, use it in a mod and tell us.
- Provisional is a statement about how far a name has been used, not about whether it works. Nothing in
  `eqoa.capabilities` changes: a provisional name still reports `supported`.
- The FrontiersForge compatibility layer (below) is neither proven nor provisional.

Until the final freeze the list is a forecast: it is regenerated whenever a shipped mod starts or stops
using a name, and a name only moves toward proven unless the project's owner decides otherwise.

## What never changes under `eqoa/1`

For a mod that declares `"api": "eqoa/1"`, once frozen, none of these change:

- the name of a module, function, field or event;
- the type of a field or argument, its units (times in seconds, headings in radians, positions in game
  units) and its base (arrays and indexes start at 1);
- what an absent value means (a missing optional field is `nil`; zero HP is real, absent HP is not zero);
- what an error code means, and the shape of a result (`value, nil`, `nil, Error`, or `nil, nil` for "nothing
  there");
- the order events arrive in, and how long a reference (handle) stays valid;
- the default of an optional argument;
- the numbers of existing enums (the one exception: `ImGuiWindowFlags` and `ImGuiCond` freeze their names only; their integers are whatever the client's Dear ImGui uses, may change with an ImGui update, and a mod must use the names, never literal numbers).

## What may be added (and does not break you)

- new functions, modules, events and capability names;
- new optional fields on records, new optional arguments;
- new values where the reference lists the values of a string field (an entity's `kind`, a dialog's
  `prompt_kind`, an event's `cause`, a `resource` event's `kind`), and new error codes for new situations.

So your mod must **ignore** event fields it does not know, treat a value or error code it does not know as
"something else", and must not assume a record has only the fields you saw.

## What is not promised

- **Numbers marked "design intent, not part of the promise" in the reference**: some quotas, limits and
  timings (for example how many subscriptions a mod may hold, or how long a hidden window session may sit
  idle). They describe how the client is tuned today. The names, shapes and meanings around them are
  promised; the exact number is not.
- **Items listed as "under review" in the reference** (a short list in the actions section).
- Anything about performance.
- Forge compatibility (below).

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
- The `capabilities` list in `mod.json` is a different thing: it is what your mod declares to the player,
  and it is required for acting (`eqoa.actions.<name>`, `eqoa.windows.takeover`, `eqoa.map.waypoint`). It
  is not a feature test.

## Forge mods

Mods written for FrontiersForge keep loading, and the compatibility is kept up. It is **not** part of the
`eqoa/1` promise: Forge names, units and index conventions are its own and follow Forge, not this page.

## If a name is marked supported but is not

The support table is generated from the code, but a mistake is possible. If `capabilities.has` says yes and
the call is really a stub (or the reverse), that is a bug in the client, not in your mod. It is corrected in
the next release by building the function or by changing the answer to the truth (a stub becomes `not_yet`
or `unsupported`). From 0.7.0 a test stops any supported name from disappearing or losing its
status, so such a downgrade is a deliberate exception: the project's owner approves it, the frozen surface
file is changed in the open, and the release notes say so. Report a case with the path and the `Capability`
record the client gave you.
