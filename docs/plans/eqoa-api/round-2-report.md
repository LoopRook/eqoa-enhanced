# Round 2 implementation record

Status: in progress on `feat/eqoa-api-r2`, base `413d1ee`. No integration or release authorized.

The approved spec is the authority. Work sequence: descriptors/diagnostics, native
state and events, shared resource/file services, lifecycle/presentation adapters,
example and author guide, regression/wasm/footprint/release verification.

## Decisions and progress

- Execute the approved design inline; the lead owns independent review as required
  by the worker brief. No reviewer or implementation subagents are spawned.
- All additions stay in fork-owned files except the exact neutral call seams in §10.
- The §9 sandbox and §12 resource migration regressions are in this round.
- Keep existing profile identity and trusted manager profile selection unchanged.

## Verification

Pending. No passing build, runtime parity, or owner acceptance claimed.
