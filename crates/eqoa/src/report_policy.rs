//! Enhanced keeps diagnostics locally. Report uploads are disabled except for
//! an explicitly configured loopback test server; a future fork service belongs here.

pub fn enabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    return false;
    #[cfg(not(target_arch = "wasm32"))]
    upload_url(std::env::var("EQOA_REPORTS").ok().as_deref(), "report").is_ok()
}

pub fn upload_url(endpoint: Option<&str>, route: &str) -> Result<String, &'static str> {
    const DISABLED: &str =
        "Report uploads are disabled in EQOA Enhanced (local diagnostics are kept)";
    if !matches!(route, "report" | "crash") {
        return Err(DISABLED);
    }
    let base = endpoint.ok_or(DISABLED)?.trim_end_matches('/');
    // Numeric sockets avoid DNS, credentials and ambiguous URL authorities.
    // Reject URL syntax that could change how the HTTP client reads this base.
    if base.bytes().any(|b| {
        b.is_ascii_whitespace()
            || b.is_ascii_control()
            || matches!(b, b'\\' | b'?' | b'#' | b'%' | b'@')
    }) {
        return Err(DISABLED);
    }
    let rest = base.strip_prefix("http://").ok_or(DISABLED)?;
    let authority = rest.split('/').next().ok_or(DISABLED)?;
    let socket = authority
        .parse::<std::net::SocketAddr>()
        .map_err(|_| DISABLED)?;
    if !socket.ip().is_loopback() || socket.port() == 0 {
        return Err(DISABLED);
    }
    Ok(format!("{base}/{route}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_have_no_default_or_remote_destination() {
        for endpoint in [
            None,
            Some("https://eqoaclassic.com/eqoa/reports"),
            Some("http://example.com:80/reports"),
            Some("http://localhost:8080/reports"),
            Some("http://127.0.0.1.evil:80/reports"),
            Some("http://127.0.0.1:80@evil/reports"),
            Some("http://127.0.0.1:80/reports?next=evil"),
            Some("http://127.0.0.1:80\\evil/reports"),
        ] {
            assert!(upload_url(endpoint, "report").is_err(), "{endpoint:?}");
        }
    }

    #[test]
    fn explicit_loopback_fixtures_are_the_control() {
        assert_eq!(
            upload_url(Some("http://127.0.0.1:8080/reports/"), "report"),
            Ok("http://127.0.0.1:8080/reports/report".into())
        );
        assert_eq!(
            upload_url(Some("http://[::1]:8080/reports"), "crash"),
            Ok("http://[::1]:8080/reports/crash".into())
        );
        assert!(upload_url(Some("http://127.0.0.1:8080/reports"), "../elsewhere").is_err());
        assert!(upload_url(Some("http://127.0.0.1:0/reports"), "report").is_err());
    }
}
