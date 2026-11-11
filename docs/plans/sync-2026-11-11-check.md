# FAIL — independent fix-round check

The requested tests and footprint gate pass, and the H1 negative control fails as expected. H1 is nevertheless not fully fixed: if migration cannot create its destination or cannot move/copy a legacy report, startup proceeds and the server sender can read the unmoved local report. The migration result is ignored (`crates/eqoa/src/fork_ext/report_core.rs:334-337`); `migrate_legacy` returns immediately on directory creation failure or leaves failed files in place (`crates/eqoa/src/fork_ext/report_crash_dir.rs:30-31,44-48`), while `play.rs:2981-2984` sends pending upstream-folder files. This can expose the local log tail/state to an offering server. Also, `read_dir` and individual entry errors are silently flattened and can still result in the completion marker (`report_crash_dir.rs:35,50-51`), preventing a retry. A same-name destination is removed before the move is known to succeed (`report_crash_dir.rs:42-44`). These are code-inspection findings; failure injection was not run.

## 1. Named tests

All seven exist and passed. Real paths: four `crashfile::fork_dirs_tests::{a_fork_crash_is_in_its_own_folder_and_never_pending_for_a_server, old_local_crash_files_move_out_of_the_senders_folder_once, pruning_or_answering_one_sides_reports_leaves_the_other, a_limit_barely_over_the_label_sends_nothing_and_keeps_the_report}`; two `fork_ext::report_label::tests::{the_support_note_names_enhanced_within_its_limit, the_label_says_the_upstream_commit_it_is_synced_to}`; and `play::fork_hooks::tests::the_note_on_the_wire_is_labelled`. The wire limit test exercises the pieces and pins the hook by source text, not an end-to-end `send_pending` call.

## 2. H1 negative control

Starting from a clean tree, temporarily changed `report_crash_dir::DIR` from `enhanced-crashes` to `crashes`, then ran `cargo test --locked -p eqoa --bin eqoa crashfile::fork_dirs_tests:: -- --test-threads=1`: exit 101, 1 passed, 3 failed. The failing tests were `a_fork_crash_is_in_its_own_folder_and_never_pending_for_a_server`, `old_local_crash_files_move_out_of_the_senders_folder_once`, and `pruning_or_answering_one_sides_reports_leaves_the_other`. The fourth test, the unrelated size-limit control, passed. Ran `git -c safe.directory=* checkout -- .` in a finally block and confirmed `git status --porcelain` empty before creating this report.

## 3. Startup order

Native `boot` calls `logfile::start` at `crates/eqoa/src/main.rs:712-720`, before `viewer::run_boot` at line 734; `logfile::start` calls `crate::report::start` at `crates/eqoa/src/logfile.rs:77-78`. Native report startup invokes `migrate_legacy` synchronously at `crates/eqoa/src/fork_ext/report_core.rs:325-335`. The server offer reaches `crashfile::send_pending` in the play loop at `crates/eqoa/src/play.rs:2964,2981-2984`. Therefore migration is attempted before login and sending on the ordinary native boot path, but successful completion is not enforced (see verdict).

## 4. Existing test changes against e0fc130

Inspected the complete diff. Six existing tests changed; no assertions removed or loosened:

- `fork_ext::report::tests::saving_produces_zip_with_all_parts_and_enhanced_header`, `crash_offer_path_saves_to_desktop`, and `log_with_enhanced_header_detected_as_ended_without_closing`: fixture paths use the new folder helper; assertions unchanged.
- `fork_ext::report_core::tests::a_crash_carries_its_own_runs_version_and_log`: fixture path uses the new folder helper; assertions unchanged.
- `fork_ext::report_sync_tests::sync_crash_marker_is_offered_on_next_start_hook`: two offer paths use the new folder helper; assertions unchanged.
- `fork_ext::report_label::tests::every_kept_report_gets_the_enhanced_identity_before_its_body`: adds short-limit assertions and adjusts the smallest accepted input to include the required body; strengthened, not loosened.

Other test changes add new tests or test-only helpers; net changes remove comments, not tests.

## 5. Reruns

Each Cargo command used a freshly created empty `EQOA_CONFIG`, `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, a separately acquired atomic `C:/Users/hocka/Paseo/cargo-build.lock`, foreground execution and a 3,600-second timeout; lock removed after each. Counts:

| Filter | Passed | Failed | Ignored | Exit |
|---|---:|---:|---:|---:|
| `fork_ext::` | 468 | 0 | 25 | 0 |
| `crashfile` | 7 | 0 | 0 | 0 |
| `the_support_note_names_enhanced_within_its_limit` | 1 | 0 | 0 | 0 |
| `the_label_says_the_upstream_commit_it_is_synced_to` | 1 | 0 | 0 | 0 |
| `the_note_on_the_wire_is_labelled` | 1 | 0 | 0 | 0 |

Rust's default test capture hides output of passing tests, so these counts do not establish how many prerequisite-based early returns occurred. The separate negative-control invocation used the same lock/target/timeout and its own fresh configuration. `python tools/check_fork_footprint.py --base a3cedec7d8bb2688cf8ed56d34a280f4a459b0ef` exited 0, PASS; all hooks met their recorded counts. No worktree `target` directory was created. Logs: `C:/Users/hocka/AppData/Local/Temp/eqoa-check-nfdi7_xt/`.
