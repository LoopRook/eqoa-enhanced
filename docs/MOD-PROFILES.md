# Character mod profiles

The owner-approved contract is in backlog.md, "Mod profiles per character or role".

Implementation choices:

- JSON schema 1 bundles v2 addon keys, enabled flags, bounded plain Save values,
  and ImGui ini text. The legacy data files remain read-only migration inputs.
- Profile names are 1-48 ASCII letters, digits, spaces, hyphens or underscores.
  Reject invalid input visibly; never silently rename it. Names differing only by ASCII case cannot coexist. Default is retained as the fallback and cannot be deleted.
- Character bindings are keyed by configured server and character name only.
  Class defaults are scoped to the server. Explicit character bindings win.
- No global active-profile file. Selection belongs to each Manager/process.
  Sharing a named profile deliberately uses last-save-wins, as the owner specified.
- Profile changes run at the render boundary, before NewFrame. Old VMs are cleaned
  up and retired; replacement VMs initialize and receive Load as on a fresh load.
- Saving captures all initialized addon Save callbacks and the current layout.
  Automatic enabled/layout persistence and shutdown save target the active bundle.
- Failed reads cannot be overwritten, including external corruption after a load.
  A failed switch leaves the prior active profile intact and shows a diagnostic.
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
- fork_ext/addons/runtime.rs:657 - manager.profile_boundary(context
- fork_ext/addons/runtime.rs:387 - active.manager.shutdown()
- fork_ext/addons/manager.rs:447 - let result = if let Some(s) = &mut self.named
- fork_ext/addons/manager.rs:574 - let result = if let Some(s) = &mut self.named
- fork_ext/addons/manager_profiles.rs:227 - pub fn profile_boundary(
- fork_ext/addons/manager_profiles.rs:333 - pub fn profile_controls(

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
