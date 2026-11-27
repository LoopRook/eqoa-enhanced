// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! The mods kept as examples for mod authors in `docs/modding/examples/` (not shipped, not installed): their files for
//! the tests that drive them as a player's own mod would run. The Stats Box left the bundle on 2026-11-27 (the game's
//! own Stat Window is the one players use); the API it uses (`eqoa.hud.hide/show/visible("stat_window")`,
//! `hud.piece_requested`, `eqoa.actions.toggle_stats_box`, `eqoa.player.stats`, ...) is still `eqoa/1`'s, and these
//! tests keep proving it with the mod itself.
use crate::fork_ext::bundled::BundledFile;

pub(crate) const FILES: &[BundledFile] = &[
    BundledFile {
        path: "eqoa_stats_box/mod.json",
        package: "eqoa_stats_box",
        data: include_bytes!("../../../../../docs/modding/examples/eqoa_stats_box/mod.json"),
    },
    BundledFile {
        path: "eqoa_stats_box/main.lua",
        package: "eqoa_stats_box",
        data: include_bytes!("../../../../../docs/modding/examples/eqoa_stats_box/main.lua"),
    },
    BundledFile {
        path: "eqoa_stats_box/README.md",
        package: "eqoa_stats_box",
        data: include_bytes!("../../../../../docs/modding/examples/eqoa_stats_box/README.md"),
    },
];

/// The shipped mods' files and the examples', as the fixtures that set up "every mod" want them.
pub(crate) fn with_bundled() -> impl Iterator<Item = &'static BundledFile> {
    crate::fork_ext::bundled::EMBEDDED_FILES.iter().chain(FILES.iter())
}

/// The folder an example mod's files are in (for the tests that read a mod's source or discover it in place).
pub(crate) fn dir(id: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding/examples").join(id)
}

/// Every example mod has its files here, and none of them is shipped.
#[test]
fn the_example_mods_are_complete_and_not_shipped() {
    for file in FILES {
        let on_disk = std::fs::read(dir(file.package).join(file.path.split_once('/').unwrap().1)).unwrap();
        assert_eq!(on_disk, file.data, "{}", file.path);
        assert!(!crate::fork_ext::bundled::PACKAGES.contains(&file.package), "{} is not shipped", file.package);
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().all(|f| f.package != file.package));
    }
    let found = super::discovery::discover(&dir("eqoa_stats_box").parent().unwrap().to_path_buf());
    assert!(found.addons.iter().any(|a| a.id == "eqoa_stats_box"), "{:?}", found.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>());
}
