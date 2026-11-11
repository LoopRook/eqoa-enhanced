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

/// Label all pending reports, including reports kept by an older build. A
/// server that cannot accept the full identity gets none; the files stay kept.
pub(crate) fn pending(
    kept: Vec<crate::crashfile::Kept>,
    most: usize,
) -> Vec<crate::crashfile::Kept> {
    let label = crate::fork_ext::report::enhanced_label();
    if most.min(net::session::CRASH_REPORT_MOST) < label.len() {
        return Vec::new();
    }
    kept.into_iter()
        .map(|mut report| {
            report.text = format!("{label}\n{}", report.text);
            report
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kept_report_gets_the_enhanced_identity_before_its_body() {
        use crate::crashfile::Kept;
        let label = super::enhanced_label();
        for body in [
            "EQOA client crash report\npanic: boom",
            "an older page's crash",
            "",
        ] {
            let kept = vec![Kept {
                number: 17,
                text: body.into(),
            }];
            assert!(
                super::pending(kept.clone(), label.len() - 1).is_empty(),
                "short offer must keep the report unsent"
            );
            let sent = super::pending(kept.clone(), net::session::CRASH_REPORT_MOST);
            assert_eq!(sent[0].number, 17);
            assert!(sent[0].text.starts_with(&label));
            assert!(sent[0].text.ends_with(body));
            assert!(
                !body.starts_with("EQOA Enhanced"),
                "unlabelled negative control"
            );
            let smallest = super::pending(kept, label.len());
            assert_eq!(
                &smallest[0].text[..label.len()],
                label,
                "whole identity survives wire truncation"
            );
        }
        assert!(super::pending(Vec::new(), usize::MAX).is_empty());
    }
}
