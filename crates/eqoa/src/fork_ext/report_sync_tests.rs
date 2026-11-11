// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
use super::*;

#[test]
fn sync_f3_saves_redacted_zip_through_dialog_hook() {
    // Wiring guards complement the behavior check: removing the event/draw/shot
    // hooks must fail here even on a machine without a window or GPU.
    let viewer = include_str!("../viewer.rs");
    assert!(viewer.contains("fork_report::event(self, el, &event)"));
    assert_eq!(
        viewer
            .matches("fork_report::shot(g, frame.as_ref())")
            .count(),
        2
    );
    assert_eq!(
        viewer.matches("crate::report::draw(vui, &mut r)").count(),
        2
    );
    let root = std::env::temp_dir().join(format!("eqoa-sync-f3-{}", std::process::id()));
    let desktop = root.join("Desktop");
    let config = root.join("Config");
    std::fs::create_dir_all(&desktop).unwrap();
    std::fs::create_dir_all(&config).unwrap();
    let account = "SyncPrivateAccount";
    let password = "SyncPrivatePassword";
    super::super::secret(account, password);
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "/home/sync-private".into());
    std::fs::write(
        config.join("eqoa.log"),
        format!("{account} {password} {home}"),
    )
    .unwrap();
    *lock(&DIALOG) = None;
    assert!(!is_report_key(&winit::keyboard::Key::Named(
        winit::keyboard::NamedKey::F2
    )));
    assert!(!is_open(), "negative control: no F3, no dialog");
    assert!(is_report_key(&winit::keyboard::Key::Named(
        winit::keyboard::NamedKey::F3
    )));
    super::super::open();
    assert!(is_open());
    set_shot(1, 1, &[15, 25, 35, 255]);
    key(Key::Text(format!("{account} {password} {home}")));
    crate::fork_ext::report::TEST_DESTINATION
        .with(|d| *d.borrow_mut() = Some((desktop.clone(), config.clone())));
    key(Key::Enter);
    crate::fork_ext::report::TEST_DESTINATION.with(|d| *d.borrow_mut() = None);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut dialog = lock(&DIALOG);
        let d = dialog.as_mut().unwrap();
        d.poll();
        if let Status::Sent(paths) = &d.status {
            let path = PathBuf::from(&paths[0]);
            assert_eq!(path.parent(), Some(desktop.as_path()));
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("EQOA-Enhanced-report-")
            );
            let bytes = std::fs::read(path).unwrap();
            assert!(bytes.starts_with(b"PK"));
            let text = String::from_utf8_lossy(&bytes);
            for entry in ["note.txt", "screenshot.png", "eqoa.log", "manifest.txt"] {
                assert!(text.contains(entry), "{entry}");
            }
            for secret in [account, password, home.as_str()] {
                assert!(!text.contains(secret), "leaked {secret}");
            }
            *dialog = None;
            break;
        }
        assert!(!matches!(d.status, Status::Failed(..)), "save failed");
        assert!(Instant::now() < deadline, "save timed out");
        drop(dialog);
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// F3 must reach Support's form while the map is up: upstream handles it
/// before the map captures every key. This wiring guard fails if the map
/// block moves back above it (and if the block is duplicated again).
#[test]
fn sync_f3_is_handled_before_the_map_takes_every_key() {
    let viewer = include_str!("../viewer.rs");
    let f3 = viewer.find("// F3: a bug report").expect("the F3 handler");
    let map = viewer
        .find("// The map (`map.rs`), while it is up, takes every key.")
        .expect("the map's key capture");
    assert!(f3 < map, "F3 must be handled before the map captures every key");
    assert_eq!(
        viewer
            .matches("// The map (`map.rs`), while it is up, takes every key.")
            .count(),
        1,
        "one map key capture"
    );
}

#[test]
fn sync_crash_marker_is_offered_on_next_start_hook() {    assert!(include_str!("../logfile.rs").contains("crate::report::start()"));
    assert!(include_str!("../logfile.rs").contains("crate::report::crash("));
    let root = std::env::temp_dir().join(format!("eqoa-sync-crash-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let log = root.join("eqoa.log");
    let closed =
        "[eqoa] run 20261103-010101, local (local), windows x86_64, window 1\n[eqoa] closed\n";
    std::fs::write(&log, closed).unwrap();
    take_last_run(&root, &log);
    assert!(
        offer_in(&crate::fork_ext::report_crash_dir::in_config(&root)).is_none(),
        "negative control: clean exit"
    );
    std::fs::write(
        &log,
        closed.replace("[eqoa] closed", "crash marker: interrupted"),
    )
    .unwrap();
    take_last_run(&root, &log);
    let offer = offer_in(&crate::fork_ext::report_crash_dir::in_config(&root)).expect("crash offered at next start");
    assert!(matches!(offer.kind, Kind::Crash(ref files) if files.len() == 1));
    std::fs::remove_dir_all(root).unwrap();
}
