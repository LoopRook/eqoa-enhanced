//! Local telemetry broadcaster for external tools (e.g. EQOA Combat Parser).
//! Broadcasts clean JSON combat events over UDP to 127.0.0.1:28015 so external
//! tools can run without needing Administrator privileges, packet sniffers, or PINE.

use std::net::UdpSocket;
use std::sync::OnceLock;

static SOCKET: OnceLock<Option<UdpSocket>> = OnceLock::new();

fn socket() -> Option<&'static UdpSocket> {
    SOCKET
        .get_or_init(|| {
            let sock = UdpSocket::bind("127.0.0.1:0").ok()?;
            sock.set_nonblocking(true).ok()?;
            Some(sock)
        })
        .as_ref()
}

/// Broadcast a telemetry event to local consumers.
pub fn broadcast(json: &str) {
    if let Some(sock) = socket() {
        let _ = sock.send_to(json.as_bytes(), "127.0.0.1:28015");
    }
}
