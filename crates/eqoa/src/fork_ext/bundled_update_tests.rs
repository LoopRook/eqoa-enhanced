// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

struct Install {
    root: PathBuf,
    mods: PathBuf,
    cfg: PathBuf,
}
impl Install {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eqoa-bundle-update-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let this = Self {
            mods: root.join("mods"),
            cfg: root.join("config"),
            root,
        };
        install_bundled_mods(&this.mods, Some(&this.cfg)).unwrap();
        this
    }
    fn remember_old(&self, path: &str, bytes: &[u8]) {
        let file = bundled_registry_path(&self.cfg);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        let key = mods_folder_key(&self.mods);
        if value.get("shipped").is_none() {
            value["shipped"] = serde_json::json!({});
        }
        if value["shipped"].get(&key).is_none() {
            value["shipped"][&key] = serde_json::json!({});
        }
        value["shipped"][&key][path] = serde_json::json!([format!("{:x}", Sha256::digest(bytes))]);
        fs::write(file, serde_json::to_vec(&value).unwrap()).unwrap();
    }
}
impl Drop for Install {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("eqoa-bundle-update-")
        );
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bundled_update_replaces_unmodified_old_wereoxxs_file() {
    let i = Install::new();
    let path = "wereoxxs_ui/wereoxxs_ui.lua";
    let old = b"-- a previously shipped Wereoxxs script\n";
    fs::write(i.mods.join(path), old).unwrap();
    i.remember_old(path, old);
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(
        fs::read(i.mods.join(path)).unwrap(),
        EMBEDDED_FILES.iter().find(|f| f.path == path).unwrap().data
    );
}

#[test]
fn bundled_update_keeps_player_edited_file() {
    let i = Install::new();
    let path = "wereoxxs_ui/wereoxxs_ui.lua";
    fs::write(i.mods.join(path), b"-- player edits\n").unwrap();
    let result = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(result.kept_edited.get("wereoxxs_ui"), Some(&1));
    assert_eq!(fs::read(i.mods.join(path)).unwrap(), b"-- player edits\n");
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned original Wereoxxs package)"]
fn bundled_update_legacy_registry_updates_reference_wereoxxs() {
    let i = Install::new();
    let path = "wereoxxs_ui/modules/ability_bars.lua";
    let reference =
        PathBuf::from(std::env::var_os("EQOA_FORGE_REFERENCE").expect("reference required"));
    let old = fs::read(reference.join("scripts").join(path)).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&old)),
        "fbb343fac5b109f03c8ee775c5be7fcb04ae44b424fb986706a40afee741ff04"
    );
    fs::write(i.mods.join(path), old).unwrap();
    let file = bundled_registry_path(&i.cfg);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("shipped");
    fs::write(file, serde_json::to_vec(&value).unwrap()).unwrap();
    let result = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(result.files_updated, 1);
    assert!(
        fs::read(i.mods.join(path)).unwrap()
            == EMBEDDED_FILES.iter().find(|f| f.path == path).unwrap().data
    );
}

/// The previous, unedited Wereoxxs scroller stays a trusted shipped hash, so
/// updating from a build that predates this fix replaces it instead of taking
/// it for a player edit; a hand-edited scroller is still left alone.
#[test]
fn bundled_update_replaces_unmodified_previous_wereoxxs_chat_window() {
    let i = Install::new();
    let path = "wereoxxs_ui/modules/chat_window.lua";
    let embedded = EMBEDDED_FILES.iter().find(|f| f.path == path).unwrap();
    let shipped = shipped_hashes(embedded);
    for hash in [
        "d87eab664c1114f20f6dfbd49df3b0aaf14d21efc323a6dc444a52cc832095a8",
        "a0943e355d787109b44d1f19dccc0beeee61e6731b126f293ccc4bf234f127dc",
    ] {
        assert!(
            shipped.contains(hash),
            "the previous chat_window.lua must stay a known shipped hash: {hash}"
        );
    }

    let old = b"-- the scroller as it shipped before the R2 fix\n";
    fs::write(i.mods.join(path), old).unwrap();
    i.remember_old(path, old);
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(fs::read(i.mods.join(path)).unwrap(), embedded.data);

    let edited = b"-- player edits to the scroller\n";
    fs::write(i.mods.join(path), edited).unwrap();
    let result = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(result.kept_edited.get("wereoxxs_ui"), Some(&1));
    assert_eq!(fs::read(i.mods.join(path)).unwrap(), edited);
}

#[test]
fn bundled_update_adds_missing_new_file() {
    let i = Install::new();
    let path = "wereoxxs_ui/modules/casting_bar.lua";
    fs::remove_file(i.mods.join(path)).unwrap();
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(
        fs::read(i.mods.join(path)).unwrap(),
        EMBEDDED_FILES.iter().find(|f| f.path == path).unwrap().data
    );
}

#[test]
fn review_regression_bundled_update_restores_missing_shared_resources() {
    let i = Install::new();
    fs::remove_dir_all(i.mods.join("resources")).unwrap();
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert!(i.mods.join("resources/gear-icon.png").is_file());
}

#[test]
fn bundled_update_does_not_resurrect_deleted_package() {
    let i = Install::new();
    fs::remove_dir_all(i.mods.join("wereoxxs_ui")).unwrap();
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert!(!i.mods.join("wereoxxs_ui").exists());
}

#[test]
fn bundled_update_leaves_settings_and_overlays_untouched() {
    let i = Install::new();
    for name in [
        "addons/profiles/Healer.json",
        "addons/key.json",
        "addons/key-files/files/settings.lua",
        "addons/imgui.ini",
    ] {
        let path = i.cfg.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, name.as_bytes()).unwrap();
    }
    install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    for name in [
        "addons/profiles/Healer.json",
        "addons/key.json",
        "addons/key-files/files/settings.lua",
        "addons/imgui.ini",
    ] {
        assert_eq!(fs::read(i.cfg.join(name)).unwrap(), name.as_bytes());
    }
}

#[test]
fn bundled_update_rejects_corrupt_or_oversized_registry_without_writes() {
    let i = Install::new();
    let reg = bundled_registry_path(&i.cfg);
    for invalid in [b"{ damaged".to_vec(), vec![b' '; 1024 * 1024 + 1]] {
        fs::write(&reg, &invalid).unwrap();
        assert!(install_bundled_mods(&i.mods, Some(&i.cfg)).is_err());
        assert_eq!(fs::read(&reg).unwrap(), invalid);
    }
}

/// The Stats Box left the bundle on 2026-11-27 (the game's own Stat Window is the one players use). An install that an
/// older client gave the mod still has its folder, recorded in `bundled.json` with the hashes the client wrote, and the
/// player's profile may still have it on: the unchanged folder is taken out at start, so nothing loads and the game's
/// window is the player's again. A folder the player changed is kept, as is a copy the bundler never put there.
/// Control: the same folder with one changed byte is kept, and so is one not recorded as the bundler's.
#[test]
fn a_retired_mods_unchanged_folder_is_taken_out_and_a_changed_or_own_one_is_kept() {
    let old: [(&str, &[u8]); 3] = [
        ("eqoa_stats_box/mod.json", br#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"eqoa_stats_box","entry":"main.lua"}"#),
        ("eqoa_stats_box/main.lua", b"-- an old client's Stats Box\n"),
        ("eqoa_stats_box/README.md", b"# Stats Box\n"),
    ];
    let as_installed = |i: &Install, edit: bool| {
        fs::create_dir_all(i.mods.join("eqoa_stats_box")).unwrap();
        for (path, bytes) in old {
            fs::write(i.mods.join(path), bytes).unwrap();
            i.remember_old(path, bytes); // what the older client wrote in bundled.json when it installed the file
        }
        let file = bundled_registry_path(&i.cfg);
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        value["folders"][mods_folder_key(&i.mods)].as_array_mut().unwrap().push("eqoa_stats_box".into());
        fs::write(file, serde_json::to_vec(&value).unwrap()).unwrap();
        if edit {
            fs::write(i.mods.join("eqoa_stats_box/main.lua"), b"-- an old client's Stats Box, changed by the player\n").unwrap();
        }
    };

    // a fresh install never gets it
    let i = Install::new();
    assert!(!i.mods.join("eqoa_stats_box").exists(), "the Stats Box is not installed any more");
    assert!(!PACKAGES.contains(&"eqoa_stats_box") && RETIRED.contains(&"eqoa_stats_box"));

    // an old install's unchanged folder: taken out, and not put back on the next start
    as_installed(&i, false);
    let r = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert_eq!(r.retired_removed, ["eqoa_stats_box"]);
    assert!(r.retired_kept.is_empty());
    assert!(!i.mods.join("eqoa_stats_box").exists(), "the unchanged folder is gone");
    let again = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert!(again.retired_removed.is_empty() && !i.mods.join("eqoa_stats_box").exists());
    assert!(i.mods.join("eqoa_dialogs/main.lua").is_file(), "the other bundled mods are untouched");
    drop(i);

    // control: the player changed a file: kept as it is
    let i = Install::new();
    as_installed(&i, true);
    let r = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert!(r.retired_removed.is_empty());
    assert_eq!(r.retired_kept, ["eqoa_stats_box"]);
    assert_eq!(fs::read(i.mods.join("eqoa_stats_box/main.lua")).unwrap(), b"-- an old client's Stats Box, changed by the player\n");
    drop(i);

    // control: a copy the bundler never installed (a mod author's, from the examples) is not touched or reported
    let i = Install::new();
    fs::create_dir_all(i.mods.join("eqoa_stats_box")).unwrap();
    for (path, bytes) in old {
        fs::write(i.mods.join(path), bytes).unwrap();
        i.remember_old(path, bytes);
    }
    let r = install_bundled_mods(&i.mods, Some(&i.cfg)).unwrap();
    assert!(r.retired_removed.is_empty() && r.retired_kept.is_empty());
    assert!(i.mods.join("eqoa_stats_box/main.lua").is_file(), "the author's own copy stays");
}

/// The bootstrap list holds every version of the retired mods the repository ever had, so an install whose
/// `bundled.json` lost its history still has an unchanged folder recognised. Control: unknown bytes are not.
#[test]
fn every_shipped_version_of_a_retired_mod_is_in_the_bootstrap_list() {
    for path in ["eqoa_stats_box/main.lua", "eqoa_stats_box/mod.json", "eqoa_stats_box/README.md", "eqoa_turnin/main.lua"] {
        assert!(!bootstrap_hashes(path).is_empty(), "{path}");
    }
    // the last Stats Box the client shipped (bundled-mods/ at develop 4936f2c0 and after)
    assert!(bootstrap_hashes("eqoa_stats_box/main.lua").contains("2192342f2d5c8c61a824254f0a5ad05a12e6b6ad2e22345b7aad5fb25eeaa755"));
    assert!(!bootstrap_hashes("eqoa_stats_box/main.lua").contains(&hash(b"-- not a shipped file\n")));
}

// ---- review round 1 (2026-11-27): what the player is told, and the install record, when a retired mod's folder goes ----

const OLD_STATS_BOX: [(&str, &[u8]); 3] = [
    ("eqoa_stats_box/mod.json", br#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"eqoa_stats_box","entry":"main.lua"}"#),
    ("eqoa_stats_box/main.lua", b"-- an old client's Stats Box\n"),
    ("eqoa_stats_box/README.md", b"# Stats Box\n"),
];

/// An older client's install of the Stats Box in `i`: its files, the hashes that client wrote in bundled.json, and the
/// folder on the record as the bundler's.
fn install_old_stats_box(i: &Install) {
    fs::create_dir_all(i.mods.join("eqoa_stats_box")).unwrap();
    for (path, bytes) in OLD_STATS_BOX {
        fs::write(i.mods.join(path), bytes).unwrap();
        i.remember_old(path, bytes);
    }
    let file = bundled_registry_path(&i.cfg);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    value["folders"][mods_folder_key(&i.mods)].as_array_mut().unwrap().push("eqoa_stats_box".into());
    fs::write(file, serde_json::to_vec(&value).unwrap()).unwrap();
}

/// Whether bundled.json still records the Stats Box as the bundler's in `i`'s mods folder.
fn on_record(i: &Install) -> bool {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(bundled_registry_path(&i.cfg)).unwrap()).unwrap();
    value["folders"][mods_folder_key(&i.mods)].as_array().unwrap().iter().any(|v| v == "eqoa_stats_box")
}

fn install_saying(i: &Install) -> (Result<InstallResult, String>, Vec<String>) {
    let mut lines = Vec::new();
    let r = install_bundled_mods_saying(&i.mods, Some(&i.cfg), &mut |l| lines.push(l.to_string()));
    (r, lines)
}

/// MEDIUM-2: a retired folder taken out is also taken off the record. A player who later copies the example
/// (docs/modding/examples/eqoa_stats_box) into his mods folder has made his own mod: never removed and never reported,
/// on an install that once had the bundled copy; even an exact old shipped copy put back is his. Red before the fix:
/// the id stayed on the record, so the example was reported as "changes of yours" at every start and the old copy was
/// taken out again.
#[test]
fn review1_after_retirement_a_copy_the_player_makes_is_his_own_mod() {
    let i = Install::new();
    install_old_stats_box(&i);
    let (r, lines) = install_saying(&i);
    assert_eq!(r.unwrap().retired_removed, ["eqoa_stats_box"]);
    assert_eq!(lines.iter().filter(|l| l.contains("eqoa_stats_box")).count(), 1, "{lines:?}");
    assert!(!on_record(&i), "taken off the record once taken out");
    // the player copies the example in
    for file in crate::fork_ext::addons::example_mods::FILES {
        let path = i.mods.join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.data).unwrap();
    }
    for start in 0..2 {
        let (r, lines) = install_saying(&i);
        let r = r.unwrap();
        assert!(r.retired_removed.is_empty() && r.retired_kept.is_empty() && r.retired_failed.is_empty(), "start {start}: {r:?}");
        assert!(!lines.iter().any(|l| l.contains("eqoa_stats_box")), "start {start}: never reported: {lines:?}");
        assert!(i.mods.join("eqoa_stats_box/main.lua").is_file(), "start {start}: the player's copy stays");
    }
    // and an exact old shipped copy put back is his too: not taken out again
    fs::remove_dir_all(i.mods.join("eqoa_stats_box")).unwrap();
    for (path, bytes) in OLD_STATS_BOX {
        fs::create_dir_all(i.mods.join(path).parent().unwrap()).unwrap();
        fs::write(i.mods.join(path), bytes).unwrap();
    }
    let (r, _) = install_saying(&i);
    assert!(r.unwrap().retired_removed.is_empty());
    assert_eq!(fs::read(i.mods.join("eqoa_stats_box/main.lua")).unwrap(), b"-- an old client's Stats Box\n");
}

/// MEDIUM-1: an empty folder left of a retired mod (what a removal the system cut short leaves) is simply removed at the
/// next start, without a "changes of yours" line. Red before the fix: an empty folder counted as changed and was kept,
/// with that line at every start. Control: a folder with a file the player added is his: kept, said once, then never.
#[test]
fn review1_an_empty_leftover_folder_is_removed_without_a_changes_line() {
    let i = Install::new();
    install_old_stats_box(&i);
    for (path, _) in OLD_STATS_BOX {
        fs::remove_file(i.mods.join(path)).unwrap();
    }
    let (r, lines) = install_saying(&i);
    let r = r.unwrap();
    assert_eq!(r.retired_removed, ["eqoa_stats_box"], "{r:?}");
    assert!(r.retired_kept.is_empty());
    assert!(!lines.iter().any(|l| l.contains("changes of yours")), "{lines:?}");
    assert!(!i.mods.join("eqoa_stats_box").exists() && !on_record(&i));
    let j = Install::new();
    install_old_stats_box(&j);
    fs::write(j.mods.join("eqoa_stats_box/notes.txt"), b"mine").unwrap();
    let (r, lines) = install_saying(&j);
    assert_eq!(r.unwrap().retired_kept, ["eqoa_stats_box"]);
    assert_eq!(lines.iter().filter(|l| l.contains("changes of yours")).count(), 1);
    assert!(j.mods.join("eqoa_stats_box/notes.txt").is_file() && !on_record(&j));
    let (r, lines) = install_saying(&j);
    assert!(r.unwrap().retired_kept.is_empty() && !lines.iter().any(|l| l.contains("eqoa_stats_box")), "said once only");
}

/// MEDIUM-1: the system will not remove the folder (a file held open by another program without delete sharing, as an
/// editor can on Windows): the player is told that, with the system's reason, not "changes of yours"; the folder stays on
/// the record and the next start, with the file let go, takes out what is left. Red before the fix: reported as kept for
/// the player's changes.
#[cfg(windows)]
#[test]
fn review1_a_folder_the_system_will_not_remove_is_reported_truthfully_and_tried_again() {
    use std::os::windows::fs::OpenOptionsExt;
    let i = Install::new();
    install_old_stats_box(&i);
    // read sharing only: it can be read (and so matched), not deleted
    let held = fs::OpenOptions::new().read(true).share_mode(1).open(i.mods.join("eqoa_stats_box/main.lua")).unwrap();
    let (r, lines) = install_saying(&i);
    let r = r.unwrap();
    assert!(r.retired_kept.is_empty(), "not reported as the player's changes: {r:?}");
    assert!(r.retired_removed.is_empty());
    assert_eq!(r.retired_failed.len(), 1, "{r:?}");
    assert_eq!(r.retired_failed[0].0, "eqoa_stats_box");
    assert!(!r.retired_failed[0].1.is_empty(), "the system's reason is given");
    assert_eq!(lines.iter().filter(|l| l.contains("could not be taken out") && l.contains("tries again")).count(), 1, "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains("changes of yours")), "{lines:?}");
    assert!(on_record(&i), "stays on the record: tried again at the next start");
    drop(held);
    let (r, lines) = install_saying(&i);
    assert_eq!(r.unwrap().retired_removed, ["eqoa_stats_box"], "{lines:?}");
    assert!(!i.mods.join("eqoa_stats_box").exists() && !on_record(&i));
}

/// MEDIUM-1, the reviewer's last case: the folder has been taken out and a later step of the install fails: the player is
/// told what was taken out all the same. Red before the fix: the lines were written only after every later step.
#[test]
fn review1_the_line_is_written_even_if_a_later_step_of_the_install_fails() {
    let i = Install::new();
    install_old_stats_box(&i);
    // a later step fails: a folder where a bundled mod's file belongs
    let blocker = i.mods.join("eqoa_dialogs/main.lua");
    fs::remove_file(&blocker).unwrap();
    fs::create_dir_all(&blocker).unwrap();
    let (r, lines) = install_saying(&i);
    assert!(r.is_err(), "the install stops at the folder in the file's place");
    assert!(!i.mods.join("eqoa_stats_box").exists(), "the retired folder was taken out before that");
    assert_eq!(lines.iter().filter(|l| l.contains("eqoa_stats_box") && l.contains("was taken out")).count(), 1, "{lines:?}");
    assert!(!on_record(&i), "and taken off the record");
}
