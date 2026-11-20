# Round 7 Part C

Branch `feat/eqoa-api-r7c`, based on the November 18 upstream sync and then
updated from `develop` for 0.6.0.

## Capability and window contracts

- A refusal installs its Lua member and descriptor together. Native binds
  describe implemented methods and module status is derived when queried.
  Compatibility tables are inspected for members their adapters have yet
  to describe. Continuous controls name the upstream PR in their reasons.
- The round labels follow the mod API plan, including round 6 for takeover
  and the additional 6.5 actions and entity field. After the freeze, authors
  should use capability `revision` for feature detection.
- The audit classifies the round 6.5 additions in
  [the audit delta](round-7-audit.md#5-round-65-delta-part-c). Chat sender,
  channel code and direction, and the native hotbar's command text and icon
  fields have no projection in this client. Their descriptors give the reason.
- `replaces` is optional in `mod.json`. A full takeover also requires the
  existing takeover capability. A mod that uses `own` may omit `replaces`.
  The native manager displays the two approved disclosure lines from the
  manifest.
- **Use the game's windows** is a session-wide native switch. Turning it on
  releases every claim immediately and blocks new claims. Hidden sessions
  use the existing close path. Turning the switch off allows mods to claim
  windows again.

## Validation

The first CI attempt at `1539a24b` passed the checks job. Compilation of the
test binary failed to resolve a test-only field helper from the item fixture.
The next commit moved the helper into the shared addon test module. The
`develop` merge keeps the Part C UI disclosure as an unreleased change.

The first focused run passed all five registration and window-contract
tests. The record tests originally enumerated a Lua proxy with `pairs`,
which does not enumerate its backing table. The revised fixture checks
the underlying keys and reads their values through the public record.
The host event fixture uses a multi-step batch so it exercises progress
and completion.
