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

- Initial negative control: both new native contract tests failed on the base
  (missing api/client). After registration they passed (2/2).
- Native source checkpoint: 6 passed, 0 failed, 0 ignored, no SKIP lines. Includes
  production Play snapshot/effects/roster assembly, logical add_chat burst (25
  messages vs the old eight-line ring), generation expiry and live Report bytes.
- The first full addon run exposed legacy callback and resource regressions.
  Those were fixed; the next run reached 299 passed / 1 failed / 23 ignored. The
  remaining assertion concerns distinct invalid-path diagnostic messages, fixed
  after that run. A fresh full regression run remains required.
- Owner approved the measured play.rs budget increase +755 to +759 for the
  iteration, begin/end event capture and logical-text calls. Viewer stays within
  its existing budget. No other shared seams were added.

Work remains in progress. No live-game parity or owner acceptance claimed.
