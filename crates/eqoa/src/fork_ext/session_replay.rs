// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passive playback into a test Session backed by in-memory Datagrams.

use net::session::{Event, Session};

pub(crate) struct Row<'a> {
    pub sent: bool,
    pub ty: u8,
    pub bytes: &'a [u8],
}

/// Decode a recording part. Tests may select a slice of the returned rows.
pub(crate) fn rows(mut data: &[u8]) -> Result<Vec<Row<'_>>, &'static str> {
    data = data
        .strip_prefix(&net::fork_record::MAGIC)
        .ok_or("invalid recording header")?;
    let mut rows = Vec::new();
    while !data.is_empty() {
        let (header, rest) = data.split_at_checked(14).ok_or("short recording row")?;
        let sent = match header[8] {
            0 => false,
            1 => true,
            _ => return Err("invalid recording direction"),
        };
        let len = u32::from_le_bytes(header[10..14].try_into().unwrap()) as usize;
        let (bytes, next) = rest
            .split_at_checked(len)
            .ok_or("short recording payload")?;
        rows.push(Row {
            sent,
            ty: header[9],
            bytes,
        });
        data = next;
    }
    Ok(rows)
}

impl Row<'_> {
    /// Deliver server data through DRDP's local endpoint and the real Session
    /// dispatcher. The test drives client actions and consumes the events.
    pub(crate) fn deliver(&self, session: &mut Session) -> Vec<Event> {
        if self.sent || matches!(self.ty, drdp::TYPE_CONNECT | drdp::TYPE_DISCONNECT) {
            return Vec::new();
        }
        let ep = session.drdp.system_endpoint;
        let dst = session.drdp.eps[ep].id;
        session.drdp.endpoint_send(ep, dst, self.bytes, self.ty);
        session.pump()
    }
}
