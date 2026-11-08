# Round 1 design report

2026-11-08; branch `feat/eqoa-api-r1`, base `11e036f`. Design only; owner acceptance and independent review pending.

## Files written

- `docs/modding/EQOA-API-SPEC.md`: normative native API, data types/units/lifetimes, event order, capabilities/errors/versioning, resource limits, actions, sandbox, neutral seams, all 25 Forge module mappings plus UiForge/ImGui/DrawList, and round assignments.
- `docs/plans/eqoa-api/round-1-summary.md`: one-page owner summary in player/mod-author terms.
- This report. Skeleton committed separately before completing the design.

## Source verification

Read the plan, fork/handoff/ownership guidance, scout report and older four author documents at `1be52fb`. Inspected production host registration/Snapshot, runtime event/render path, manifest parser, manager frame/lifecycle/diagnostics, file/media sandbox helpers, weapon-set and chat-history extensions, and forge_snapshot. Checked native dump/entity/group/effects/ability/stat stores; target/attack/chat/cast/item paths; bank/loot/merchant transactions; native mastery receive/window; quest log; controller/control path; camera; private dialog state; map cells/store/waypoint service. Checked pinned Forge public exports across all 25 modules, relevant function bodies and UiForge binding sources. Spec anchors use this worktree, not blindly copied scout line numbers.

Resolved all three flagged part-A items:

- **Class Mastery:** x0c available, x10 **spent**, not total (`classmastery.rs:873`, reset receive :323). x08 is chosen XP allocation, not earned-point progress (:365, :762). x14 stays explicitly raw because its normalized meaning is not established; no invented percent.
- **Group coordinates:** channel40 casts s16 directly to floats (`group.rs:121`); native map/HUD use those positions directly against actors/camera in game units (`play.rs:7738,8112`), gated by world. No unproven scale factor. This is source verification of the native contract, not a live server measurement.
- **Ability display index:** explicitly reordered by 0xB6 (`play.rs:2636`) and catalogs rebuilt from entry data. It cannot serve as identity even before zoning. Use generation + ability ID + incarnation; positional keys are resolved at dispatch.

## Corrections to scout assumptions

The scout is useful but its “five complete event queues” wording conflicts with its own eight-line chat-loss and hardcoded combat dropped-counter findings. Native mutation/event sources exist; completeness is not proved. Some “REAL” rows mix valid source data with incomplete adapters; capabilities in the design distinguish support from availability. The ability-list “not id-sorted” description is self-contradictory: the BTreeMap actually gives numeric ID order, which the new API chooses explicitly. CM “total” is incorrect as above. Group.leader can be stale; derive leadership from roster. Camera compass must use camera direction, not body heading. Bank contents are present in the entry dump even closed; merchant and corpse data are session-gated. Existing file reads cover a shared script root, broader than the owner's own-folder rule; private mounts/resolution are required. New once-only entry execution must be opt-in because existing manifests already declare the same API version.

## Validation and remaining work

Static document checks PASS: all 25 Forge module mapping rows, 13 numbered sections, local Markdown links, balanced fences, consistent table columns, no conflict markers; owner summary 483 words. `git diff --check` PASS. Footprint checker PASS against reviewed base `8435f2c8a00c631d716771f8189fa0011a1fbf50`; no budgets changed. Branch confirmed `feat/eqoa-api-r1`; final changes restricted to these three documents. Pinned mouse bindings were checked directly: coordinates/drag are two-number returns, wheel returns vertical and horizontal, style vectors take two scalar arguments, and confirm kind 2 is single Cancel. These are explicit in the spec rather than copied from scout summaries.

No game code changed. No cargo build, test, wasm check, live game, GPU/audio/disc/oracle test or independent reviewer was run, as instructed; no runtime pass/skip counts are claimed. The spec explicitly requests lead review of additional neutral call sites/accessors: the plan's four-line aspiration cannot cover event fidelity, controller state and action injection honestly. There is no owner policy question; builders must measure/check their eventual footprint. Existing author drafts are mined, not retired; retirement is round 5 with owner approval. No push, merge or tag. Experience-log entry is retained here rather than writing outside the assigned worktree.
