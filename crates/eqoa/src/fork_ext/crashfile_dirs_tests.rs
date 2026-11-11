//! The fork's local crash reports and upstream's kept crash reports share
//! no folder: a report of one side is never sent, offered, pruned,
//! acknowledged or deleted by the other. (Loaded into `crashfile`'s tests by
//! one `#[path]` line so the private `files_in`, `pending_in`, `write_in`,
//! `answered_in` and `number_of` are the real ones.)
use super::*;
use crate::fork_ext::report_crash_dir::{in_config, migrate_legacy};

const FORK_TEXT: &str = "EQOA native client crash report\nversion: dev-x (dev)\n\n[client, as last noted]\nPlayer: Ann\n\n[log] last 1 lines\nsecret log tail\n";

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("eqoa-crashdirs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The fork writes a local crash (a run that never closed) into its own
/// folder: upstream's `files_in`/`pending_in` over `crashes/` see nothing,
/// the fork's offer sees it. The control: the layout before this fix (the
/// same file in `crashes/`) is exactly what upstream would send.
#[test]
fn a_fork_crash_is_in_its_own_folder_and_never_pending_for_a_server() {
    let root = scratch("own");
    let prev = root.join("eqoa.log");
    std::fs::write(&prev, "[eqoa] run 20261111-010101, local (local), windows x86_64, window 1\nlast event\n").unwrap();
    crate::report::ended_without_closing_for_test(&root, &prev);
    let mine = in_config(&root).join("crash-20261111-010101.txt");
    assert!(mine.exists(), "the fork's report is in enhanced-crashes/");
    assert!(files_in(&root.join("crashes")).is_empty(), "upstream's folder holds nothing of the fork's");
    assert!(pending_in(&root.join("crashes")).is_empty(), "nothing of the fork's goes to a server");
    assert!(matches!(crate::fork_ext::report_core::offer_in_for_test(&in_config(&root)), Some(k) if k == vec![mine.clone()]));
    // the control: put that file where the fork used to, and upstream sends it
    std::fs::create_dir_all(root.join("crashes")).unwrap();
    std::fs::copy(&mine, root.join("crashes").join("crash-20261111-010101.txt")).unwrap();
    let old_layout = pending_in(&root.join("crashes"));
    assert_eq!(old_layout.len(), 1, "the old shared layout would have been sent");
    assert!(old_layout[0].text.contains("last event"), "with its log tail");
    let _ = std::fs::remove_dir_all(&root);
}

/// Files an older build left in `crashes/` move to the fork's folder before
/// any login, once: upstream's own reports written afterwards stay. The
/// controls: before the move upstream's sender would take them; the marker
/// stops a second move; a file that is no crash report stays.
#[test]
fn old_local_crash_files_move_out_of_the_senders_folder_once() {
    let root = scratch("migrate");
    let up = root.join("crashes");
    std::fs::create_dir_all(&up).unwrap();
    std::fs::write(up.join("crash-20261101-000001.txt"), FORK_TEXT).unwrap();
    std::fs::write(up.join("crash-20261102-000002.txt"), FORK_TEXT).unwrap();
    std::fs::write(up.join("notes.txt"), "not a crash").unwrap();
    assert_eq!(pending_in(&up).len(), 2, "control: unmoved, both would be sent at the next login");
    assert_eq!(migrate_legacy(&root), 2);
    assert!(pending_in(&up).is_empty(), "none left for a server");
    assert!(up.join("notes.txt").exists(), "what is no crash report stays");
    assert_eq!(crate::fork_ext::report_core::waiting_for_test(&in_config(&root)).len(), 2, "both are the fork's now, to offer");
    // a build with upstream's sender writes its own report; a later start leaves it
    write_in(&up, "20261111-050505-1700000000", "EQOA client crash report\npanic: boom\n").unwrap();
    assert_eq!(migrate_legacy(&root), 0, "runs once");
    assert_eq!(files_in(&up).len(), 1, "upstream's new report stays where upstream keeps it");
    assert_eq!(pending_in(&up).len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

/// Pruning and acknowledging one side never deletes the other's files. The
/// control: on one shared folder the same calls do delete across.
#[test]
fn pruning_or_answering_one_sides_reports_leaves_the_other() {
    let root = scratch("prune");
    let (up, mine) = (root.join("crashes"), in_config(&root));
    std::fs::create_dir_all(&mine).unwrap();
    let fork_names: Vec<String> = (0..3).map(|i| format!("crash-2026110{i}-000000.txt")).collect();
    for n in &fork_names {
        std::fs::write(mine.join(n), FORK_TEXT).unwrap();
    }
    // upstream writes more than it keeps, then a server answers for a report
    for i in 0..KEPT + 3 {
        write_in(&up, &format!("20261111-0600{i:02}-1700000{i:03}"), &format!("report {i}")).unwrap();
    }
    assert_eq!(files_in(&up).len(), KEPT);
    assert_eq!(files_in(&mine).len(), 3, "upstream's pruning leaves the fork's files");
    let sent = pending_in(&up);
    assert!(answered_in(&up, sent[0].number));
    assert!(!answered_in(&up, number_of(&fork_names[0])), "an answer naming a fork report's name lets none go");
    assert_eq!(files_in(&mine).len(), 3, "an acknowledgement leaves the fork's files");
    // the fork's own pruning (start's) leaves upstream's
    for i in 0..9 {
        std::fs::write(mine.join(format!("crash-2026120{i}-000000.txt")), FORK_TEXT).unwrap();
    }
    let before = files_in(&up).len();
    crate::fork_ext::report_core::prune_waiting_for_test(&mine);
    assert_eq!(files_in(&up).len(), before, "the fork's pruning leaves upstream's reports");
    assert_eq!(crate::fork_ext::report_core::waiting_for_test(&mine).len(), 5, "the fork keeps its own five");
    // the control: all of it in one folder, the old layout
    let shared = root.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    let theirs = shared.join("crash-20260101-000000.txt");
    std::fs::write(&theirs, FORK_TEXT).unwrap();
    for i in 0..KEPT {
        write_in(&shared, &format!("20261111-0700{i:02}-1700000{i:03}"), "up").unwrap();
    }
    assert!(!theirs.exists(), "control: in one folder upstream's pruning deletes the fork's report");
    let _ = std::fs::remove_dir_all(&root);
}
