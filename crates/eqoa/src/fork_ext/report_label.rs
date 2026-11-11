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

/// The least of a report's own text that must go after the label: a server
/// that takes the label and a few bytes would acknowledge the report, and the
/// kept file would be let go with the crash itself lost.
const BODY_MIN: usize = 256;

/// Label all pending reports, including reports kept by an older build. A
/// report goes only when the server's limit holds the whole identity, a
/// newline and at least [`BODY_MIN`] bytes of the report (or all of it, if it
/// is shorter); else it is not sent and its file stays kept.
pub(crate) fn pending(
    kept: Vec<crate::crashfile::Kept>,
    most: usize,
) -> Vec<crate::crashfile::Kept> {
    let label = crate::fork_ext::report::enhanced_label();
    let room = most.min(net::session::CRASH_REPORT_MOST);
    let total = kept.len();
    let sent: Vec<_> = kept
        .into_iter()
        .filter(|report| room >= label.len() + 1 + report.text.len().min(BODY_MIN))
        .map(|mut report| {
            report.text = format!("{label}\n{}", report.text);
            report
        })
        .collect();
    if sent.len() < total {
        eprintln!(
            "crash: {} report(s) kept, not sent: the server's limit of {room} bytes leaves no room for the EQOA Enhanced label and a useful part of the report",
            total - sent.len()
        );
    }
    sent
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
            assert!(
                super::pending(kept.clone(), label.len()).is_empty(),
                "the label alone, with none of the report, is not a report"
            );
            let sent = super::pending(kept.clone(), net::session::CRASH_REPORT_MOST);
            assert_eq!(sent[0].number, 17);
            assert!(sent[0].text.starts_with(&label));
            assert!(sent[0].text.ends_with(body));
            assert!(
                !body.starts_with("EQOA Enhanced"),
                "unlabelled negative control"
            );
            let least = label.len() + 1 + body.len().min(256);
            assert!(
                super::pending(kept.clone(), least - 1).is_empty(),
                "one byte short of the label and a useful body keeps the report"
            );
            let smallest = super::pending(kept, least);
            assert_eq!(
                &smallest[0].text[..label.len()],
                label,
                "whole identity survives wire truncation"
            );
        }
        assert!(super::pending(Vec::new(), usize::MAX).is_empty());
    }
}
