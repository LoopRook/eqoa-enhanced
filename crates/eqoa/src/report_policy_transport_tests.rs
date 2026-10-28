// Fork-owned transport regressions, included in report.rs's test module.

/// The transport itself rejects upstream and missing destinations, even if
/// a caller bypasses the hidden controls. Loopback is the negative control.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn enhanced_report_transport_only_posts_to_explicit_loopback() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    assert!(
        native::post_to(None, "report", "{}".into())
            .unwrap_err()
            .contains("disabled")
    );
    assert!(crate::report_policy::upload_url(Some(SERVER), "report").is_err());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/reports", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let request = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        && start.elapsed().as_secs() < 3 =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(e) => panic!("fixture did not receive the request: {e}"),
            }
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut buf = [0; 4096];
        let n = stream.read(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).starts_with("POST /reports/report HTTP/1.1"));
        let body = r#"{"ok":true,"id":"local-fixture"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    assert_eq!(
        native::post_to(Some(&base), "report", "{}".into()).unwrap(),
        "local-fixture"
    );
    request.join().unwrap();
}

/// A local fixture cannot redirect a report to a second service.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn enhanced_report_transport_does_not_follow_redirects() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/reports", listener.local_addr().unwrap());
    let destination = format!("http://{}/unexpected", target.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let fixture = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        && start.elapsed().as_secs() < 3 =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(e) => panic!("fixture did not receive the request: {e}"),
            }
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut buf = [0; 4096];
        stream.read(&mut buf).unwrap();
        write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: {destination}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    assert!(native::post_to(Some(&base), "report", "{}".into()).is_err());
    fixture.join().unwrap();
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
