/// The footprint manifest the build was made with: its `reviewed_base` is the
/// upstream commit this tree is synced to.
const FOOTPRINT: &str = include_str!("../../../../docs/fork-footprint.json");

/// The first seven characters of the `reviewed_base` in `manifest`, if it has
/// one that looks like a commit.
fn synced_commit_in(manifest: &str) -> Option<&str> {
    let rest = manifest.split_once("\"reviewed_base\"")?.1;
    let sha = rest.split_once('"')?.1.split_once('"')?.0;
    (sha.len() >= 7 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then(|| &sha[..7])
}

/// The upstream commit (7 characters) this build is synced to.
pub fn synced_commit() -> &'static str {
    synced_commit_in(FOOTPRINT).unwrap_or("unknown")
}

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
        "EQOA Enhanced {build}, synced to upstream {}, a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client",
        synced_commit()
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

/// F3's server Support petition: the note sent just before it (play.rs
/// `send_bug_note`) names this client as EQOA Enhanced, on its own `client:`
/// line after upstream's `build:` line, so a GM or the vanilla client's
/// maintainers read a fork's report as the fork's. Upstream's lines stay; the
/// note stays within the wire's `BUG_NOTE_MAX` (the petition's own text, up to
/// 2,047 characters, is not touched), the lines that no longer fit going from
/// the end.
pub(crate) fn bug_note(note: String) -> String {
    let label = crate::fork_ext::report::enhanced_label();
    if note.contains(&label) {
        return note;
    }
    let (first, rest) = note.split_once('\n').unwrap_or((&note, ""));
    let mut lines: Vec<&str> = rest.lines().collect();
    let head = format!("{first}\nclient: {label}");
    let join = |lines: &[&str]| lines.iter().fold(head.clone(), |all, l| format!("{all}\n{l}"));
    let mut out = join(&lines);
    while out.len() > net::session::BUG_NOTE_MAX && lines.pop().is_some() {
        out = join(&lines);
    }
    out
}

#[cfg(test)]
mod tests {
    /// The label says which upstream commit the build is synced to: the first
    /// seven characters of the footprint's `reviewed_base`. The controls: a
    /// manifest with another base gives another commit, and one with none (or
    /// a base that is no commit) gives none, so the label's commit comes from
    /// the manifest and not from a constant.
    #[test]
    fn the_label_says_the_upstream_commit_it_is_synced_to() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../docs/fork-footprint.json")).unwrap();
        let base = manifest["reviewed_base"].as_str().unwrap();
        let label = super::enhanced_label();
        assert!(label.contains(&format!(", synced to upstream {}, a fork of ", &base[..7])), "{label}");
        assert_eq!(super::synced_commit(), &base[..7]);
        assert_eq!(super::synced_commit_in(r#"{"reviewed_base": "0123456789abcdef"}"#), Some("0123456"));
        assert_ne!(super::synced_commit_in(r#"{"reviewed_base": "fedcba9876"}"#), Some(&base[..7]));
        assert_eq!(super::synced_commit_in(r#"{"reviewed_base": "not a commit"}"#), None);
        assert_eq!(super::synced_commit_in("{}"), None);
    }

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

    /// F3's note names EQOA Enhanced as the client, behind upstream's own
    /// first line, and never past the wire's limit; upstream's lines stay.
    /// The control: the note as `bugnote::compose` makes it says no such
    /// thing.
    #[test]
    fn the_support_note_names_enhanced_within_its_limit() {
        let label = super::enhanced_label();
        let plain = crate::bugnote::compose(&[("place", "world 0 (1.0, 2.0, 3.0)".into())]);
        assert!(!plain.contains("Enhanced"), "control: upstream's note is unlabelled");
        let note = super::bug_note(plain.clone());
        let lines: Vec<&str> = note.lines().collect();
        assert!(lines[0].starts_with("build: "), "{note}");
        assert_eq!(lines[1], format!("client: {label}"));
        assert!(lines[2..].iter().copied().eq(plain.lines().skip(1)), "upstream's other lines stay: {note}");
        assert_eq!(super::bug_note(note.clone()), note, "never twice");
        // a long note: whole lines go from the end, the identity and build stay
        let long = format!("{plain}\n{}", (0..40).map(|i| format!("line {i}: {}", "y".repeat(40))).collect::<Vec<_>>().join("\n"));
        let cut = super::bug_note(long.clone());
        assert!(cut.len() <= net::session::BUG_NOTE_MAX, "{}", cut.len());
        assert!(cut.starts_with("build: ") && cut.contains(&label) && cut.contains("line 0:"));
        assert!(!cut.contains("line 39:"), "the tail went, not the identity");
    }
}
