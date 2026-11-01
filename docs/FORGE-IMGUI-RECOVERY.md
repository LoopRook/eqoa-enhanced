# Recoverable ImGui scopes and max-level Modern UI

Pinned read-only reference: FrontiersForge 4026d5c / UiForge 6f2d125 under
EQOA_FORGE_REFERENCE. Packages are unchanged. Integration tests copy the package
bytes to temporary storage so diagnostics cannot write into the reference tree.

Reference evidence (paths relative to that reference):

- UiForge/src/core/forgescript_manager.cpp:632-649 snapshots the ImGui stacks,
  executes each script, disables it only when an exception escapes, then recovers
  the ImGui state. ForgeScript::Run at :187-199 throws only if lua_pcall failed.
- UiForge/src/core/ui_manager.cpp:85-90 explicitly enables error recovery, disables
  recovery assertions and enables debug log/tooltip reporting.
- UiForge/externals/sol2_ImGui_Bindings/sol_ImGui.h:49-62 directly calls ImGui Begin/End
  (:2610-2615 registers those wrappers); it does not convert a leaked scope into a script-disable decision.
- The pinned UiForge/externals/imgui directory is empty, so ImGui's own recovery
  implementation is unavailable in this checkout. The host policy is explicit.
- scripts/modules/frontiers_forge/util.lua:355-360 returns the XP table entry,
  including nil at level 60. The native adapter keeps this behavior.
- scripts/avoids_modern_ui/modules/avoids_exp_bars.lua:258-269 opens its window,
  gets XP required, then compares nil with zero at level 60, before End at :305.
- scripts/avoids_modern_ui/avoids_modern_ui.lua:269-275 catches each module Update
  error and logs it. Its overall Lua call therefore succeeds.

Root cause: our bridge fabricated a fatal error after successfully recovering
an otherwise successful Lua invocation. The same fatal branch exists in the
released v0.3.0 bridge. This is a compatibility regression, independent of profiles.
The settings-initialization hint is normal for a disabled, uninitialized VM.

Profile hypothesis: the manager initializes/registers callbacks on the first run
and applies the saved Load callback afterward. Modern UI supplies defaults before
its first Update; partial Load only changes supplied fields. The partial fixture also disables
ExpBars at pack level, verifying Load applies even when the initial max-level
Update caught an error. Tests verify initialized
state, non-nil defaults, correct level and the saved bar width after enabling.
Both paths passed every level-59 payload before the fix; both failed at level 60
with absent Save data, reproducing the owner's exact caught nil-comparison error.

The bridge still unwinds only its own native operations. Recovered invocations
return success; uncaught Lua/API/budget errors still return their original error.
Each VM records one warning per ordered leaked-scope signature, independent of
frame and diagnostic drains. History and queue are capped at 64 signatures plus
one suppression notice. Manager diagnostics display these under the imgui phase.

Test coverage:

- forge_recovery_caught_error_keeps_addon_running_and_warns_once: five frames,
  native window/color stack equality, one warning, then an uncaught fatal error.
- forge_recovery_distinct_leaks_have_distinct_bounded_diagnostics: two signatures.
- forge_recovery_warning_history_and_queue_are_bounded: 65 distinct signatures,
  bounded history/queue, persistent deduplication across drains.
- forge_recovery_modern_{profiles,legacy}_level{59,60}: enable from disabled with
  absent, empty, partial and authentic complete Save payloads (16 combinations).
  Every combination checks five actual GPU drawing frames, initialized state,
  loaded settings and balanced native stacks. Profile cases enter a real assigned
  profile before enabling a fresh VM. Fixtures require EQOA_FORGE_REFERENCE and
  are explicitly ignored in plain CI, like the existing package tests.
- Existing incorrect End/Pop, unsafe arguments and real quota errors stay fatal;
  the old quota controls now exceed the actual current limits. A missing End alone
  is verified as recoverable.

Before production changes: 2 passed / 4 failed (the two level-59 matrices passed;
synthetic caught errors and both level-60 paths failed). After the change the six
behavior scenarios passed. A test generator's extra empty Lua statements caused
a syntax error in the new bound test; its Lua source was corrected.

Known unchanged-package limit: the XP panel still catches its own nil comparison
at level 60 and cannot finish drawing that panel. Other panels continue and the
addon stays enabled, matching UiForge. No addon files or XP return values were
changed. The package's caught error remains visible as a deduplicated Lua log.
Owner live QA is pending; final verification is recorded below.


Final verification (fresh TMP/TEMP roots; shared report-to-desktop target):

Plain:

    test result: ok. 1471 passed; 0 failed; 25 ignored; 0 measured; 0 filtered out; finished in 87.47s

Pinned reference + --include-ignored:

    test result: ok. 1496 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 102.13s

Wasm:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.74s

Footprint:

    PASS: fork footprint is within reviewed policy

All 16 Modern UI matrix combinations and all synthetic recovery/bound/error tests
ran successfully. Other prerequisite SKIP lines: 106 plain / 108 reference;
no new GPU/package fixture skipped. Release build follows the final commit and
is reported in the handoff. No user config or reference package was edited.
