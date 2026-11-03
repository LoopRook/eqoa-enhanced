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
    assert!(
        shipped_hashes(embedded)
            .contains("d87eab664c1114f20f6dfbd49df3b0aaf14d21efc323a6dc444a52cc832095a8"),
        "the previous chat_window.lua must stay a known shipped hash"
    );

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
