// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Release routing is fork-owned; shared updater mechanics remain upstream's.
pub const DOWNLOAD_BASE: &str =
    "https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/";
pub const DEV_DOWNLOAD_BASE: &str =
    "https://github.com/LoopRook/eqoa-enhanced/releases/download/dev/";
pub const RELEASE_PAGE: &str = "github.com/LoopRook/eqoa-enhanced/releases";
pub fn base(channel: Option<&str>) -> Option<&'static str> {
    match channel {
        Some("enhanced") => Some(DOWNLOAD_BASE),
        Some("enhanced-dev") => Some(DEV_DOWNLOAD_BASE),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enhanced_channels_follow_the_forks_releases_and_nothing_else_updates() {
        assert_eq!(base(Some("enhanced")), Some(DOWNLOAD_BASE));
        assert_eq!(base(Some("enhanced-dev")), Some(DEV_DOWNLOAD_BASE));
        for channel in [None, Some("dev"), Some(""), Some("unknown")] {
            assert_eq!(
                base(channel),
                None,
                "local and unrelated channels never update"
            );
        }
        assert!(DOWNLOAD_BASE.starts_with(&format!("https://{RELEASE_PAGE}/")));
        assert!(DOWNLOAD_BASE.ends_with("/latest/download/"));
        assert!(DEV_DOWNLOAD_BASE.starts_with(&format!("https://{RELEASE_PAGE}/")));
        assert!(DEV_DOWNLOAD_BASE.ends_with("/download/dev/"));
    }
}
