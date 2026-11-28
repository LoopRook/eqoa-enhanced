//! The rooted file contract of `sandbox.rs`, on every system the client builds for: every operation stays
//! under the mod's root, and no link, `..` or swapped folder leads out of it. The link tests run where links
//! can be made without a privilege (Linux, macOS); Windows keeps its own junction and handle checks.
use super::{
    discovery::{DiscoveredAddon, EntryMode},
    sandbox::{lock_parents, lock_resource, read_bounded, Mounts},
};
use std::{fs, path::{Path, PathBuf}};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eqoa-sandbox-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn mounts(root: &Path) -> Mounts {
    let scripts = root.join("mod");
    fs::create_dir_all(&scripts).unwrap();
    let data = root.join("mod-files");
    fs::create_dir_all(data.join("files")).unwrap();
    let addon = DiscoveredAddon {
        id: "sandbox_test".into(),
        name: "sandbox_test".into(),
        entry: scripts.join("main.lua"),
        script_root: scripts.clone(),
        package_root: scripts.clone(),
        module_dirs: vec![scripts.clone()],
        resource_dirs: vec![scripts],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: Vec::new(),
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: None,
    };
    Mounts { addon, data: Some(data) }
}

#[test]
fn a_file_written_under_the_root_reads_back() {
    let root = scratch("roundtrip");
    let m = mounts(&root);
    m.write("notes/a.txt", b"hello").unwrap();
    assert_eq!(m.read("notes/a.txt").unwrap(), b"hello");
    assert!(root.join("mod-files/files/data/notes/a.txt").is_file());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn parent_traversal_is_refused_for_reads_and_writes() {
    let root = scratch("dotdot");
    let m = mounts(&root);
    fs::write(root.join("secret.txt"), b"s").unwrap();
    assert!(m.read("../secret.txt").is_err());
    assert!(m.write("../secret.txt", b"x").is_err());
    assert!(m.write("a/../../secret.txt", b"x").is_err());
    assert_eq!(fs::read(root.join("secret.txt")).unwrap(), b"s");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_plain_folder_chain_is_locked_and_a_missing_one_is_made_only_when_asked() {
    let root = scratch("chain");
    let target = root.join("a/b/c.txt");
    assert!(lock_parents(&target, false).is_ok());
    assert!(!root.join("a").exists());
    let _guard = lock_parents(&target, true).unwrap();
    assert!(root.join("a/b").is_dir());
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
mod links {
    use super::*;
    use std::os::unix::fs::symlink;

    fn with_outside(name: &str) -> (PathBuf, PathBuf) {
        let root = scratch(name);
        let outside = root.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        (root, outside)
    }

    #[test]
    fn a_folder_link_inside_the_root_that_points_outside_is_refused_for_every_operation() {
        let (root, outside) = with_outside("dirlink");
        let m = mounts(&root);
        let link = root.join("mod-files/files/data");
        let _ = fs::remove_file(&link);
        symlink(&outside, &link).unwrap();
        assert!(m.read("secret.txt").is_err(), "read followed the link");
        assert!(m.write("planted.txt", b"x").is_err(), "write followed the link");
        assert!(m.remove("secret.txt").is_err(), "remove followed the link");
        assert!(!outside.join("planted.txt").exists());
        assert_eq!(fs::read(outside.join("secret.txt")).unwrap(), b"secret");
        assert!(lock_parents(&link.join("x.txt"), true).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_link_to_a_file_outside_is_refused() {
        let (root, outside) = with_outside("filelink");
        let m = mounts(&root);
        let data = root.join("mod-files/files/data");
        fs::create_dir_all(&data).unwrap();
        symlink(outside.join("secret.txt"), data.join("leak.txt")).unwrap();
        assert!(m.read("leak.txt").is_err(), "read followed the file link");
        assert!(read_bounded(&data.join("leak.txt"), 1024).is_err());
        assert!(lock_resource(&data.join("leak.txt")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn control_the_same_file_without_a_link_reads() {
        let (root, _outside) = with_outside("control");
        let m = mounts(&root);
        m.write("leak.txt", b"mine").unwrap();
        let data = root.join("mod-files/files/data");
        assert_eq!(m.read("leak.txt").unwrap(), b"mine");
        assert_eq!(read_bounded(&data.join("leak.txt"), 1024).unwrap(), b"mine");
        assert!(lock_resource(&data.join("leak.txt")).is_ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_swapped_for_a_link_after_it_was_checked_is_refused() {
        let (root, outside) = with_outside("swap");
        let dir = root.join("held");
        fs::create_dir_all(&dir).unwrap();
        let held = super::super::sandbox::unix::open_dir(&dir).unwrap();
        let id = super::super::sandbox::unix::identity(&held).unwrap();
        // Control: untouched, the folder is still the one that was opened.
        assert!(super::super::sandbox::unix::still_there(&dir, id).is_ok());
        fs::remove_dir(&dir).unwrap();
        symlink(&outside, &dir).unwrap();
        assert!(super::super::sandbox::unix::still_there(&dir, id).is_err());
        assert!(super::super::sandbox::unix::open_dir(&dir).is_err(), "a link must not open as a folder");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_replaced_by_another_folder_is_noticed() {
        let root = scratch("replace");
        let dir = root.join("held");
        fs::create_dir_all(&dir).unwrap();
        let held = super::super::sandbox::unix::open_dir(&dir).unwrap();
        let id = super::super::sandbox::unix::identity(&held).unwrap();
        fs::rename(&dir, root.join("moved")).unwrap();
        fs::create_dir_all(&dir).unwrap();
        assert!(super::super::sandbox::unix::still_there(&dir, id).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
