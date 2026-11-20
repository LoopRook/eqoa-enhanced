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

The generated preflight reference from the Windows client has 1,742
descriptors, including 1,643 supported, nine partial, 50 planned and 40
unsupported. All planned rows name their plan. The checks name all 46
supported events and cover the 72 existing record schemas plus icon
surface info.

### Negative controls

One focused run applied these three temporary mutations. Each failed the
intended test.

| Mutation | Observed failure |
|---|---|
| Mark the refusing `eqoa.actions.move` member supported | Its capability status disagreed with the call's error code. |
| Remove `class_id` from the Play player projection | `eqoa.player.Player.class_id is missing`. |
| Suppress the combat hit event's production name | `Play did not emit: combat.hit`. |

Removing `class_id` also removed the default class argument for the
item check. That fixture now passes the class, race and level explicitly
to the real `can_use` binding. All three mutations were restored
byte-for-byte before this change.

The restored focused gate passed **11 tests, 0 failed, 0 ignored**.
It used the shared Cargo lock and reported the current worktree's
test executable.
