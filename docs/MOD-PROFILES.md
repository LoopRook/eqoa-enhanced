# Character mod profiles

The owner-approved contract is in backlog.md, "Mod profiles per character or role".

Implementation choices:


Current behavior (owner-approved role setup; supersedes historical template and
silent character-copy behavior below):

- Default is an ordinary selectable, editable and shareable profile. Existing
  assignments, including Default, are unchanged. Default cannot be deleted.
- First run seeds missing Tank, Healer, Melee and Caster profiles from the saved
  Default. profiles.lock protects creation and an atomic .roles-v1 marker makes
  this a one-time operation. Existing roles are never overwritten. Deleting a
  role later does not recreate it at startup.
- New unassigned characters preview the saved class default, otherwise their
  role, and see Set up mods for Name even with the Addon Manager closed/all mods
  disabled. Use Profile assigns and shares it; Make a separate copy clones the
  chosen saved bundle into a character-named profile and assigns that copy.
  Use is the default keyboard action. Closing uses the ORIGINAL preselection;
  logout/exit before choosing also uses it. Preview changes are not flushed
  until a choice is made, so merely browsing does not alter a shared profile.
- Tank: Warrior/Paladin/Shadowknight. Healer: Druid/Shaman/Cleric.
  Melee: Ranger/Monk/Bard/Rogue. Caster: Magician/Necromancer/Enchanter/Wizard/
  Alchemist. Native Alchemist is ID 14 (live.rs:22; auctionwnd picker tests),
  already present in addons/player.rs. Enchanter ID 12 uses Caster per the lead's
  clarification. A server's explicit class default overrides its role.
- The manager lists every profile with active, character and class-default
  markers. Select a row, then Use for this character, Copy into this character's
  profile, Use as Class default or Delete. Save current as creates a new named
  profile; existing names are kept and must be loaded before editing/saving.
- Copy names use the normalized character name, then server/numeric suffixes on
  collisions. A deliberate repeated copy keeps earlier bundles rather than
  replacing them. Names remain within the existing bounded ASCII policy.
- Each world entry starts fresh VMs and loads the destination's enabled set,
  Save state and layout before its first frame. Logout pauses addon frames.
  Saves on switch/logout/exit keep the active bundle; deliberate shared profiles
  retain last-save-wins semantics. Account data is never an input.
- Atomic writes, corrupt-file protection, OS locks, transactional Save capture,
  bounded data and two-second failed-load retry remain in force. Busy first-run
  role creation retries instead of picking a misleading fallback profile.

Historical implementation/verification notes follow:

- JSON schema 1 bundles v2 addon keys, enabled flags, bounded plain Save values,
  and ImGui ini text. The legacy data files remain read-only migration inputs.
- Profile names are 1-48 ASCII letters, digits, spaces, hyphens or underscores.
  Reject invalid input visibly; never silently rename it. Names differing only by ASCII case cannot coexist. Default is retained as the fallback and cannot be deleted.
- Character bindings are keyed by configured server and character name only.
  Class defaults are scoped to the server. Explicit character bindings win.
- No global active-profile file. Selection belongs to each Manager/process.
  Sharing a named profile deliberately uses last-save-wins, as the owner specified.
- Manually selecting the active profile preserves unsaved state and layout.
  Explicit Revert reloads it from disk. Every character world entry instead starts
  fresh VMs from the assigned profile or shared Default, even for the same name.
- Failed world-entry loads retain the previous identity, retry every two seconds,
  and emit one diagnostic for that pending character.
- Profile changes run at the render boundary, before NewFrame. Old VMs are cleaned
  up and retired; replacement VMs initialize and receive Load as on a fresh load.
- Saving captures all initialized addon Save callbacks and the current layout.
  Shutdown also stages the complete bundle; one callback failure blocks publication.
  Automatic enabled/layout persistence and shutdown save target the active bundle.
- Failed reads cannot be overwritten, including external corruption after a load.
  A failed switch leaves the prior active profile intact and shows a diagnostic.
- A separate nonblocking OS lock serializes Default migration. The winner checks
  again under the lock and atomically publishes once; contenders read the result
  or report a busy migration without writing.
- The legacy resource-file overlay remains keyed by the existing v2 addon key;
  Lua Save values and layouts are profile-specific. Resource files are not part of
  the approved bundle format.
- Newly discovered addons absent from a selected bundle start disabled. Default
  migration retains the old enabled choices and manifest defaults.
- A short nonblocking operating-system lock protects character-binding read/modify/write operations;
  another client gets a visible retry message if it tries to assign simultaneously.
  The lock is released automatically on close or process exit.

Native settings safety: the new empty-layout path initially crashed in ClearIniSettings
without a bound context and then in LoadIniSettingsFromMemory with an empty Rust str.
dear-imgui-rs context/settings.rs:213 passes its byte pointer and length directly;
dear-imgui-sys 0.18 imgui.cpp:16507 calls strlen when the length is zero. The new code
binds the context through ContextBinding and passes a newline for an empty layout.
This was introduced by the profile implementation, not released client behavior.

Implementation sequence: bounded bundle/migration storage and tests; character
bindings; manager clean switch and Save routing; runtime boundary/layout integration;
Addon Manager controls; focused negative controls and full native/reference/WASM/
footprint verification; commit; release build on the final commit.

Verification coverage includes real Lua Save/Load, fresh VM frame counts and both
DisableScript/OnEject callbacks, empty and disabled-window layouts, partial Save
failure, damaged Default and named profiles, failed legacy migration, and closing
before the first frame. Native settings calls use the library's scoped binding.
Old live windows are excluded from ini serialization until the destination opens
them. A failed migration blocks writes until a healthy load or explicit Save As.

A read-only audit agent made two review turns (inherited model; no edits or cargo).
Its failed-migration and retained-window findings received failing regression
controls before fixes. Further controls caught partial Save publication and
pre-first-frame shutdown layout loss. Build logs are in target/local-checks.

## Hooks and adapters

- fork_ext/play.rs:617 - character: crate::fork_ext::addons::named_profiles
- viewer.rs:5512 - snapshot, self.booted.as_ref()
- viewer.rs:3263 - Snapshot::default(), None
- fork_ext/addons/runtime.rs:660 - manager.profile_boundary(context
- fork_ext/addons/runtime.rs:387 - active.manager.shutdown()
- fork_ext/addons/manager.rs:447 - let result = if let Some(s) = &mut self.named
- fork_ext/addons/manager.rs:574 - let result = if let Some(s) = &mut self.named
- fork_ext/addons/manager_profiles.rs:330 - pub fn profile_boundary(
- fork_ext/addons/manager_profiles.rs:493 - pub fn profile_controls(

The only shared-file changes extend the two existing viewer draw hooks with the
configured server. Normal boot uses booted Shared.server; direct play uses the
server argument, with server.opt as fallback. No account/password parameter enters
the profile API. CharacterSnapshot comes directly from Play.player.name and
Play.dump.player.class while connected, independently of HUD/loading/HP readiness.

## Tests

- mod_profiles_two_characters_restore_save_state_and_layout_on_fresh_vms
- mod_profiles_manager_save_as_bind_class_switch_and_delete_actions
- mod_profiles_two_clients_save_only_their_own_active_bundle
- mod_profiles_failed_load_keeps_active_and_can_escape_damaged_default
- mod_profiles_failed_migration_cannot_create_empty_default_on_shutdown
- mod_profiles_missing_window_layout_does_not_inherit_previous_geometry
- mod_profiles_disabled_windows_do_not_contaminate_saved_layout
- mod_profiles_failed_save_keeps_previous_bundle_on_later_autosave
- mod_profiles_names_reject_paths_devices_and_reserved_metadata
- mod_profiles_corrupt_file_is_never_overwritten_or_deleted
- mod_profiles_character_and_class_bindings_use_server_and_never_account
- mod_profiles_delete_protects_active_default_and_case_collisions
- mod_profiles_binding_lock_releases_and_deleted_assignments_fall_back
- mod_profiles_migrate_default_once_without_removing_legacy_files
- mod_profiles_world_entry_uses_character_not_account_and_survives_loading
- forge_runtime_imgui_ini_window_settings_persistence (expanded to bundles,
  legacy-file preservation, corrupt migration and closing before NewFrame).

## Final verification before commit

Commands used the worktree target directory and waited for other cargo/rustc
processes. Native tests ran one thread at a time. The reference run set
EQOA_FORGE_REFERENCE to the pinned forge-compat-reference directory.

Plain: cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture

    test result: ok. 1410 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out; finished in 63.86s

Reference: the same command with --include-ignored

    test result: ok. 1431 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 73.03s

WASM: cargo check --locked -p eqoa --target wasm32-unknown-unknown

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.46s

Footprint: python tools/check_fork_footprint.py --base ad941f5ca18a9238c5c5e51bdb77b9abb6afd37d

    PASS: fork footprint is within reviewed policy

The plain run intentionally ignores 19 pinned-package tests and two FX benchmarks.
Both runs also print prerequisite skips for unavailable disc/assets/save state,
symlink privilege and the optional Forge GPU spike. No profile regression skipped.
The reference run executes all pinned-package tests.

Negative-control logs show failed assertions for migration, profile switching,
failed-migration write protection, inherited/disabled-window layouts, transactional
Save capture, and pre-first-frame shutdown. Each passed after its corresponding fix.
The release build is required after the final commit; its result is reported in
the handoff rather than modifying the committed tree after that build.

## Terra review corrections

All four findings at f7ae3b7 reproduced. Negative control:

    test result: FAILED. 16 passed; 4 failed; 0 ignored; 0 measured; 1416 filtered out; finished in 10.54s

After fixes:

    test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 1416 filtered out; finished in 11.53s

New tests:

- mod_profiles_concurrent_migration_keeps_the_winners_saved_default
- mod_profiles_shutdown_save_failure_preserves_the_entire_previous_file
- mod_profiles_active_load_preserves_unsaved_session_until_explicit_revert
- mod_profiles_failed_world_entry_retries_after_backoff_with_one_diagnostic
- mod_profiles_concurrent_character_bindings_keep_both_clients_entries

The binding test passed before the fixes: characters.json already uses a fresh
read under the nonblocking OS lock, followed by atomic replacement. Two simultaneous
clients retrying lock contention retain both assignments.

Review-fix full verification (same commands and prerequisites as above):

Plain:

    test result: ok. 1415 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out; finished in 68.25s

Reference:

    test result: ok. 1436 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 76.71s

Wasm:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.68s

Footprint:

    PASS: fork footprint is within reviewed policy

Release is built after the new review-fix commit.

## Owner live-test follow-up (release/0.4.0)

Read-only evidence at b29d982, 2026-11-01:

- The real addons/profiles directory contained only Default.json as a named
  profile: no Test.json. characters.json had one explicit character assignment
  to Default and no class defaults. This means those characters share Default.
- Default enabled Wereoxxs, disabled ARPG/Modern, and held Wereoxxs Save data plus
  window positions. Its timestamp was 12:15, while legacy files were last changed
  at 03:57. Default's hide_ability_bars, hide_game_chat and real_item_icons flags
  differed from the legacy values. Later saves had updated Default.
  Default explicitly saved chat=true and bars=true, with player/target/group/
  casting/compass/meter=false: the partial Wereoxxs UI was already in Default.
- No addon resource-overlay directories existed. The Wereoxxs package's Save/Load
  callbacks use the bundled settings table. Overlay sharing cannot explain this
  instance. Neither eqoa.log nor eqoa.prev.log had profile-switch records, so an
  exact attempted-switch timeline cannot be recovered from these files.
- The per-addon Save profile/Load profile buttons in runtime.rs:629 called one
  addon's callbacks and updated the selected named bundle's cached addon state;
  they did not create or assign a named profile. These obsolete production buttons
  made two different meanings of profile visible at once.
- World entry resolved unassigned characters to Default, but load_named treated
  an already-active Default as a no-op. Logout saved the active bundle yet kept
  running the retained VMs. Distinct Test-to-Default bundle selection already
  worked; same-Default runtime state and logout-frame isolation did not.

New contract: one profile section owns all enabled mods, Save state and layout.
The active name remains selected at character select, while old addon frame
callbacks pause and release their active HUD claims. World entry loads the
assignment (or shared Default) with fresh VMs and the saved layout. Settings saved
into Default are deliberately shared by characters using Default. Save as a new
profile, then Use for this character, creates a separate setup. Manual selection
of the currently active profile stays a no-op; explicit Revert still discards
unsaved changes. This replaces the earlier review's world-entry no-op behavior.

Per-package Save/Load buttons are hidden when named profiles are enabled; the
legacy standalone-manager test path remains supported. Two short help lines
explain the single model. World entry, logout, selection, saves and assignments
now emit Mod profiles log records with validated character/server, active profile
and enabled counts. No account/password or raw session input enters this logger.
Real user configuration and packages were read only, never modified or copied.

Regression tests:

- mod_profiles_test_character_logout_unassigned_character_loads_default
- mod_profiles_same_default_after_logout_starts_a_fresh_character_vm
- mod_profiles_active_load_preserves_unsaved_session_until_explicit_revert

Before production fixes, the two new tests failed (20 passed, 2 failed). They
assert logout frame isolation and Default VM freshness, plus the complete
Test-to-logout-to-unassigned-Default enabled/state/layout sequence.

Live-test follow-up verification (worktree release-0-4-0, shared report-to-desktop target):

Plain:

    test result: ok. 1450 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out; finished in 74.13s

Reference:

    test result: ok. 1471 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 79.36s

Wasm:

        Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s

Footprint:

    PASS: fork footprint is within reviewed policy

The first plain run had one unrelated failure: the unchanged map community
test reused a PID-based cache directory, loaded version 9 with one marker, and
expected its three-marker fixture. It passed alone with a fresh TMP/TEMP root.
Both final full runs used fresh temporary roots; no map source was changed.
Existing disc/assets/privilege/optional-GPU prerequisites still printed skips.
No profile regression or reference-package fixture skipped. Release build follows
the final commit; its result is reported in the handoff.

## Automatic per-character follow-up

Requested negative control: 22 passed / 5 failed before implementation. These
five tests now cover the owner's A/off -> B/template -> A/off sequence, the old
A -> Default assignment, class seeding by copy, active-profile delete refusal,
and a rendered visible scrollable list containing all stored profiles:

- mod_profiles_automatic_character_owner_sequence_isolates_enabled_state
- mod_profiles_automatic_legacy_default_assignment_copies_template
- mod_profiles_automatic_class_default_seeds_an_independent_character
- mod_profiles_automatic_active_character_profile_cannot_be_deleted
- mod_profiles_visible_list_contains_all_profiles_in_a_scrollable_region

Further negative controls caught invalid automatic names/reselect assignment
(27 passed / 2 failed) and template Revert/pending-load recovery (0 passed / 2
failed). Additional tests cover simultaneous auto creation and damaged assigned
files, explicit sharing counts, active/assigned/class catalog markers, template
editing without changing an existing character, and recovery to a healthy profile.
The old runtime ini test now explicitly enters template-edit mode before changing
Default; its cold-close/migration/corrupt-file/restore checks remain intact.

Automatic-character final verification (fresh TMP/TEMP roots, requested shared target):

Plain:

    test result: ok. 1460 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out; finished in 79.26s

Reference:

    test result: ok. 1481 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 87.08s

Wasm:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.43s

Footprint:

    PASS: fork footprint is within reviewed policy

All 32 profile regressions passed. Existing prerequisite SKIP lines: 106 plain /
108 reference; no profile or pinned-package fixture skipped. The release build
is run after the final commit and reported in the handoff. Live owner retest is pending.


## Role setup follow-up (supersedes automatic template copying)

The six requested workflow regressions failed against 26ed0de before production
changes: 0 passed / 6 failed. They cover Druid -> Healer preselection and Copy,
Share propagation, a selectable/shareable/writable Default, closing -> Use the
preselection, one-time role seeding, and fresh Save/enabled/layout/VM state.
Additional negative controls reproduced busy first-run seeding (6 passed / 1
failed) and a queued old-world close incorrectly answering the next character's
prompt (0 passed / 1 failed). Both now pass. Class mapping covers all 15 native
IDs, including Alchemist 14 and the clarified Enchanter 12 -> Caster.

Final verification (fresh TMP/TEMP roots; requested report-to-desktop target):

Plain:

    test result: ok. 1468 passed; 0 failed; 21 ignored; 0 measured; 0 filtered out; finished in 85.42s

Pinned reference plus --include-ignored:

    test result: ok. 1489 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 97.62s

Wasm:

    cargo :     Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.93s

Footprint:

    PASS: fork footprint is within reviewed policy

All 40 profile regressions ran; no profile/package fixture skipped. Other
prerequisite SKIP lines: 106 plain / 108 reference (disc/assets, privileges and
optional GPU prerequisites). Release build runs after the final commit and is
reported in the handoff. Owner live QA is pending.
