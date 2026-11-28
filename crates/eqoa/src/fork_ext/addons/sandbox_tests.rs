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
    #[cfg(unix)]
    {
        super::rooted::register_root(&scripts);
        super::rooted::register_root(&data);
    }
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
    use crate::fork_ext::addons::rooted;
    use std::os::unix::fs::symlink;

    /// A mod root with `files/data/a/b/c.txt` ("inside") and a folder `outside` beside it with the same tree ("outside").
    fn tree(name: &str) -> (PathBuf, Mounts, PathBuf) {
        let root = scratch(name);
        let m = mounts(&root);
        let data = root.join("mod-files/files/data");
        fs::create_dir_all(data.join("a/b")).unwrap();
        fs::write(data.join("a/b/c.txt"), b"inside").unwrap();
        let outside = root.join("outside");
        fs::create_dir_all(outside.join("a/b")).unwrap();
        fs::write(outside.join("a/b/c.txt"), b"outside").unwrap();
        fs::write(outside.join("c.txt"), b"outside").unwrap();
        (root, m, outside)
    }

    /// Replaces `what` (a folder or a file) with a link to `to`, owned by the current user.
    fn link_in_place(what: &Path, to: &Path) {
        if what.is_dir() {
            fs::rename(what, what.with_extension("moved")).unwrap();
        } else {
            fs::remove_file(what).unwrap();
        }
        symlink(to, what).unwrap();
    }

    #[test]
    fn control_the_unlinked_tree_reads_writes_lists_and_removes() {
        let (root, m, _outside) = tree("control");
        assert_eq!(m.read("a/b/c.txt").unwrap(), b"inside");
        m.write("a/b/d.txt", b"new").unwrap();
        assert_eq!(m.read("a/b/d.txt").unwrap(), b"new");
        let data = root.join("mod-files/files/data");
        let mut names = rooted::list_names(&data.join("a/b")).unwrap();
        names.sort();
        assert_eq!(names.len(), 2);
        assert!(m.remove("a/b/d.txt").unwrap());
        assert!(rooted::stat(&data.join("a/b/c.txt")).unwrap().is_file);
        let _ = fs::remove_dir_all(&root);
    }

    /// A link owned by the current user at the first, a middle and the last component is refused by every operation.
    #[test]
    fn a_link_at_the_first_middle_or_last_component_is_refused_by_read_write_remove_list_and_stat() {
        let cases: [(&str, &str, fn(&Path) -> PathBuf); 3] = [
            ("first", "mod-files/files/data", |o: &Path| o.to_path_buf()),
            ("middle", "mod-files/files/data/a", |o: &Path| o.join("a")),
            ("last", "mod-files/files/data/a/b/c.txt", |o: &Path| o.join("c.txt")),
        ];
        for (case, rel_link, target_of) in cases {
            let (root, m, outside) = tree(&format!("link-{case}"));
            let site = root.join(rel_link);
            link_in_place(&site, &target_of(&outside));
            let key = "a/b/c.txt";
            assert!(m.read(key).is_err(), "{case}: read followed the link");
            assert!(m.write("a/b/new.txt", b"x").is_err() || case == "last", "{case}: write followed the link");
            assert!(!outside.join("a/b/new.txt").exists(), "{case}: a file was planted outside");
            assert!(m.remove(key).is_err(), "{case}: remove followed the link");
            assert!(outside.join("a/b/c.txt").exists(), "{case}: the outside file was removed");
            let data = root.join("mod-files/files/data");
            assert!(rooted::stat(&data.join("a/b/c.txt")).is_err(), "{case}: stat followed the link");
            if case != "last" {
                assert!(rooted::list_names(&data.join("a/b")).is_err(), "{case}: list followed the link");
            }
            assert!(lock_parents(&data.join("a/b/c.txt"), true).is_err() || case == "last", "{case}: the chain followed the link");
            let _ = fs::remove_dir_all(&root);
        }
    }

    /// The rule is "any link, whoever owns it": a root-owned link cannot be made on a runner, so the helper's
    /// refusal is shown to look at nothing but the link (no owner is read anywhere in `rooted.rs`).
    #[test]
    fn the_rule_refuses_any_link_whatever_its_owner() {
        let source = include_str!("rooted.rs");
        for owner_check in ["uid()", "st_uid", "MetadataExt", "geteuid"] {
            assert!(!source.contains(owner_check), "rooted.rs must not look at who owns a link: {owner_check}");
        }
        let (root, _m, outside) = tree("anyowner");
        symlink(outside.join("c.txt"), root.join("mod-files/files/data/l.txt")).unwrap();
        assert!(rooted::stat(&root.join("mod-files/files/data/l.txt")).is_err());
        assert!(rooted::links_below_root(&root.join("mod-files/files/data/l.txt")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_root_reached_through_a_link_is_resolved_once_and_the_link_below_it_is_still_refused() {
        let base = scratch("rootlink");
        let real = base.join("real");
        fs::create_dir_all(real.join("data")).unwrap();
        fs::write(real.join("data/f.txt"), b"fine").unwrap();
        let alias = base.join("alias");
        symlink(&real, &alias).unwrap();
        // the root as the host is given it runs through a system-style link: followed once, at registration
        let root = alias.join("data");
        rooted::register_root(&root);
        assert_eq!(rooted::read_file(&root.join("f.txt"), 100).unwrap(), b"fine");
        // a link below the root is not
        symlink(real.join("data/f.txt"), root.join("l.txt")).unwrap();
        assert!(rooted::read_file(&root.join("l.txt"), 100).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    /// The swap goes between the descent and the operation (a test seam): the operation still happens in the folder
    /// that was opened. Control: a read by full path made after the same swap does read the outside file.
    #[test]
    fn a_folder_swapped_for_a_link_after_the_descent_changes_nothing_for_read_write_remove_and_list() {
        for op in ["read", "write", "remove", "list"] {
            let (root, _m, outside) = tree(&format!("swap-{op}"));
            let data = root.join("mod-files/files/data");
            let folder = data.join("a/b");
            let target = folder.join("c.txt");
            let (swap_from, swap_to, outside_b) = (folder.clone(), folder.with_extension("moved"), outside.join("a/b"));
            rooted::set_after_descent_for_test(Some(Box::new(move || {
                fs::rename(&swap_from, &swap_to).unwrap();
                symlink(&outside_b, &swap_from).unwrap();
            })));
            match op {
                "read" => assert_eq!(rooted::read_file(&target, 100).unwrap(), b"inside", "read the replacement"),
                "write" => {
                    rooted::write_replace(&target, b"changed", false, None).unwrap();
                    assert_eq!(fs::read(outside.join("a/b/c.txt")).unwrap(), b"outside", "wrote through the link");
                    assert_eq!(fs::read(data.join("a/b.moved/c.txt")).unwrap(), b"changed");
                }
                "remove" => {
                    rooted::remove_file(&target).unwrap();
                    assert!(outside.join("a/b/c.txt").exists(), "removed through the link");
                    assert!(!data.join("a/b.moved/c.txt").exists());
                }
                _ => {
                    let names = rooted::list_names(&folder).unwrap();
                    assert_eq!(names, vec![std::ffi::OsString::from("c.txt")]);
                }
            }
            rooted::set_after_descent_for_test(None);
            if op == "read" {
                // control: by full path, after the swap, the outside file is what a path-based read gets
                assert_eq!(fs::read(&target).unwrap(), b"outside");
            }
            let _ = fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn a_pipe_and_an_oversized_file_are_refused_without_hanging() {
        let (root, _m, _outside) = tree("special");
        let data = root.join("mod-files/files/data");
        let fifo = data.join("pipe");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: a valid path string.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        fs::write(data.join("big.bin"), vec![b'x'; 2048]).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let (fifo2, big, small) = (fifo.clone(), data.join("big.bin"), data.join("a/b/c.txt"));
        std::thread::spawn(move || {
            let pipe = read_bounded(&fifo2, 1024);
            let over = read_bounded(&big, 1024);
            let fits = read_bounded(&big, 4096);
            let control = read_bounded(&small, 1024);
            let _ = tx.send((pipe, over, fits, control, lock_resource(&fifo2).is_err()));
        });
        let (pipe, over, fits, control, lock_refused) =
            rx.recv_timeout(std::time::Duration::from_secs(10)).expect("a read of a pipe must not hang");
        assert!(pipe.unwrap_err().contains("not a regular file"));
        assert!(over.unwrap_err().contains("exceeds"));
        assert_eq!(fits.unwrap().len(), 2048, "control: the same file within the limit reads");
        assert_eq!(control.unwrap(), b"inside");
        assert!(lock_refused);
        let _ = fs::remove_dir_all(&root);
    }
}
