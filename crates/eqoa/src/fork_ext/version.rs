// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The build label players see. Stable and local builds show VERSION as it is;
//! a Nightly says so and names the commit it was built from.

/// `version` for Stable and local builds; for Nightly, `version+nightly` and
/// the short build commit when there is one.
pub fn display_version(channel: Option<&str>, commit: Option<&str>, version: &str) -> String {
    if channel != Some("enhanced-dev") {
        return version.to_string();
    }
    match commit.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{version}+nightly ({})", short(c)),
        None => format!("{version}+nightly"),
    }
}

/// This build's label: the compiled VERSION and the channel/commit CI stamped.
pub fn current() -> String {
    display_version(
        crate::update::CHANNEL,
        crate::update::COMMIT,
        include_str!("../../../../VERSION").trim(),
    )
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}

#[cfg(test)]
mod tests {
    use super::display_version;

    #[test]
    fn stable_and_local_builds_show_the_plain_version() {
        for channel in [Some("enhanced"), None, Some("unknown")] {
            assert_eq!(
                display_version(channel, Some("0123456789abcdef"), "0.4.14"),
                "0.4.14",
                "{channel:?}"
            );
        }
    }

    #[test]
    fn a_nightly_says_so_and_names_its_build() {
        assert_eq!(
            display_version(Some("enhanced-dev"), Some("0123456789abcdef"), "0.4.14"),
            "0.4.14+nightly (0123456)"
        );
        assert_eq!(
            display_version(Some("enhanced-dev"), None, "0.4.14"),
            "0.4.14+nightly",
            "a Nightly with no stamped commit still names its channel"
        );
        assert_eq!(
            display_version(Some("enhanced-dev"), Some("  "), "0.4.14"),
            "0.4.14+nightly",
            "a blank commit is no commit"
        );
    }
}
