/// Returns the Enhanced header label identifying this client fork.
pub fn enhanced_label() -> String {
    let version = crate::fork_ext::version::current();
    let commit =
        crate::update::COMMIT.map_or("local build", |c| if c.len() > 7 { &c[..7] } else { c });
    let build = if crate::update::CHANNEL == Some("enhanced-dev") {
        version
    } else {
        format!("{version} ({commit})")
    };
    format!(
        "EQOA Enhanced {build}, a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"
    )
}

/// Prefix at the sending boundary, including reports kept by an older build.
pub(crate) fn crash_text(text: &str) -> String {
    format!("{}\n{text}", crate::fork_ext::report::enhanced_label())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kept_report_gets_the_enhanced_identity_before_its_body() {
        for body in [
            "EQOA client crash report\npanic: boom",
            "an older page's crash",
            "",
        ] {
            let sent = super::crash_text(body);
            assert!(sent.starts_with(&super::enhanced_label()));
            assert!(sent.ends_with(body));
            assert!(
                !body.starts_with("EQOA Enhanced"),
                "unlabelled negative control"
            );
            assert!(
                sent.chars()
                    .take(64)
                    .collect::<String>()
                    .contains("EQOA Enhanced")
            );
        }
    }
}
