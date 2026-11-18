// SPDX-License-Identifier: AGPL-3.0-or-later
//! Enhanced's opt-in game-message recorder. The transport calls `receive`
//! before decoding, while the message and any preceding sends are still intact.

use crate::session::{Session, State};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use web_time::Instant;

pub const MAGIC: [u8; 8] = *b"EQREC\x01\r\n";
const HEADER_LEN: usize = 14;
const PART_LIMIT: usize = 32 << 20;
const RETAIN_PARTS: usize = 4;

static ENABLED: AtomicBool = AtomicBool::new(false);
static RECORDER: Mutex<Option<Recorder>> = Mutex::new(None);

pub fn start(dir: &Path, run: Option<&str>) {
    if std::env::var_os("EQOA_RECORD_SESSION").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let mut slot = RECORDER.lock().unwrap_or_else(|e| e.into_inner());
    if !ENABLED.load(Ordering::Relaxed) {
        let stamp;
        let run = match run {
            Some(run) => run,
            None => {
                stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string();
                &stamp
            }
        };
        *slot = Some(Recorder::new(
            dir,
            &format!("session-{run}-{}", std::process::id()),
        ));
        ENABLED.store(true, Ordering::Relaxed);
    }
}

/// Register the credentials used by the client's separate account login.
pub fn note_credentials(account: &str, password: &str) {
    if ENABLED.load(Ordering::Relaxed)
        && let Some(recorder) = RECORDER.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
    {
        recorder.note_credentials(account, password);
    }
}

pub(crate) fn receive(session: &mut Session, message: &drdp::Message) {
    if ENABLED.load(Ordering::Relaxed) {
        with_recorder(session, |recorder, session| {
            recorder.observe(session, message)
        });
    }
}

/// Drain the existing send observation queue before the next frame and on Play drop.
pub fn drain(session: &mut Session) {
    if ENABLED.load(Ordering::Relaxed) && session.record.is_some() {
        with_recorder(session, |recorder, session| {
            recorder.drain(session)?;
            if let Some(file) = &mut recorder.file {
                file.flush()?;
            }
            Ok(())
        });
    }
}

fn with_recorder(
    session: &mut Session,
    f: impl FnOnce(&mut Recorder, &mut Session) -> io::Result<()>,
) {
    let mut slot = RECORDER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(recorder) = slot.as_mut()
        && let Err(error) = f(recorder, session)
    {
        eprintln!("session recorder stopped: {error}");
        *slot = None;
    }
    if slot.is_none() {
        session.record = None;
    }
}

struct Recorder {
    dir: PathBuf,
    stem: String,
    start: Instant,
    file: Option<BufWriter<File>>,
    parts: VecDeque<PathBuf>,
    next_part: u32,
    size: usize,
    limit: usize,
    secrets: Vec<Vec<u8>>,
    credential_drop_logged: bool,
    part_removal_logged: bool,
}

fn opcode(ty: u8, data: &[u8]) -> Option<u16> {
    matches!(ty, drdp::TYPE_GUARANTEED | drdp::TYPE_BEST_EFFORT)
        .then(|| data.first_chunk::<2>().copied().map(u16::from_le_bytes))
        .flatten()
}

impl Recorder {
    fn new(dir: &Path, stem: &str) -> Self {
        Self {
            dir: dir.to_owned(),
            stem: stem.into(),
            start: Instant::now(),
            file: None,
            parts: VecDeque::new(),
            next_part: 1,
            size: 0,
            limit: PART_LIMIT,
            secrets: Vec::new(),
            credential_drop_logged: false,
            part_removal_logged: false,
        }
    }

    fn note_credentials(&mut self, account: &str, password: &str) {
        for value in [account, password].into_iter().filter(|v| !v.is_empty()) {
            let short = value.chars().take(4).count() < 4;
            let narrow = value.as_bytes().to_vec();
            let wide: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
            for (units, mut bytes) in [(narrow.len(), narrow), (wide.len() / 2, wide)] {
                if short {
                    // Match a full protocol string field for short values.
                    let mut field = (units as u32).to_le_bytes().to_vec();
                    field.extend(bytes);
                    bytes = field;
                }
                if !self.secrets.contains(&bytes) {
                    self.secrets.push(bytes);
                }
            }
        }
        if !password.is_empty() {
            let token = crate::auth::udp_token(password).to_vec();
            if !self.secrets.contains(&token) {
                self.secrets.push(token);
            }
        }
    }

    fn observe(&mut self, session: &mut Session, message: &drdp::Message) -> io::Result<()> {
        // The first world dump arrives in Selecting, and dispatch queues its
        // replies immediately. Arm here to include both sides of that entry.
        let world_dump = matches!(session.state, State::Selecting | State::World)
            && session.conn.is_some()
            && message.connection == session.conn
            && opcode(message.ty, &message.data) == Some(0x000D);
        let promoted = session.state == State::Selecting
            && message.ty == drdp::TYPE_CONNECT
            && message.connection.is_some();
        if promoted || world_dump {
            session.record.get_or_insert_with(Vec::new);
        }
        if session.record.is_some() && matches!(session.state, State::Selecting | State::World) {
            self.drain(session)?;
            self.write(0, message.ty, &message.data)?;
        }
        Ok(())
    }

    fn drain(&mut self, session: &mut Session) -> io::Result<()> {
        if let Some(queued) = &mut session.record {
            for (ty, data) in queued.drain(..) {
                self.write(1, ty, &data)?;
            }
        }
        Ok(())
    }

    fn write(&mut self, direction: u8, ty: u8, data: &[u8]) -> io::Result<()> {
        // State channels and local connection notifications have no opcode.
        // DRDP system, fragment and connectionless payloads stay outside the file.
        let permitted = match opcode(ty, data) {
            // Add any new authentication or credential-carrying opcode to this blocklist.
            Some(op) => !matches!(
                op,
                0x0000
                    | 0x0001
                    | 0x0904
                    | 0x07B3
                    | 0x7F53
                    | 0x7F54
                    | 0x0790
                    | 0x0791
                    | 0x0992
                    | 0x0993
                    | 0x7E0F
                    | 0x7E65
            ),
            None => {
                ty < drdp::TYPE_STATE_CHANNELS
                    || matches!(ty, drdp::TYPE_CONNECT | drdp::TYPE_DISCONNECT)
            }
        };
        if !permitted || data.len() > self.limit - MAGIC.len() - HEADER_LEN {
            return Ok(());
        }
        if self
            .secrets
            .iter()
            .any(|s| data.windows(s.len()).any(|w| w == s))
        {
            if !self.credential_drop_logged {
                eprintln!("session recorder: dropped a message containing a known credential");
                self.credential_drop_logged = true;
            }
            return Ok(());
        }
        let size = HEADER_LEN + data.len();
        if self.file.is_none() || self.size + size > self.limit {
            self.rotate()?;
        }
        let micros = self.start.elapsed().as_micros() as u64;
        let mut header = [0; HEADER_LEN];
        header[..8].copy_from_slice(&micros.to_le_bytes());
        header[8] = direction;
        header[9] = ty;
        header[10..].copy_from_slice(&(data.len() as u32).to_le_bytes());
        let file = self.file.as_mut().unwrap();
        file.write_all(&header)?;
        file.write_all(data)?;
        self.size += size;
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        fs::create_dir_all(&self.dir)?;
        let path = self
            .dir
            .join(format!("{}-{:04}.eqrec", self.stem, self.next_part));
        let mut file = BufWriter::new(File::create_new(&path)?);
        file.write_all(&MAGIC)?;
        self.file = Some(file);
        self.parts.push_back(path);
        self.next_part += 1;
        self.size = MAGIC.len();
        if self.parts.len() > RETAIN_PARTS
            && let Err(error) = fs::remove_file(self.parts.pop_front().unwrap())
            && !self.part_removal_logged
        {
            eprintln!("session recorder: could not remove an old recording part: {error}");
            self.part_removal_logged = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Datagrams;

    struct NullDatagrams;
    impl Datagrams for NullDatagrams {
        fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
        fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
            None
        }
    }

    fn test_session() -> Session {
        Session::connect_with(
            Box::new(NullDatagrams),
            ([127, 0, 0, 1], 5000),
            ([127, 0, 0, 1], 10070),
            "FakeRecorderAccount",
            [42; 32],
        )
        .unwrap()
    }

    fn test_recorder(name: &str) -> Recorder {
        let dir = std::env::temp_dir().join(format!("eqoa-{}-{name}", std::process::id()));
        Recorder::new(&dir, "session-fixture")
    }

    fn payloads(bytes: &[u8]) -> Vec<(u8, u8, &[u8])> {
        assert_eq!(&bytes[..8], b"EQREC\x01\r\n");
        let mut rest = &bytes[8..];
        let mut rows = Vec::new();
        while !rest.is_empty() {
            let len = u32::from_le_bytes(rest[10..14].try_into().unwrap()) as usize;
            rows.push((rest[8], rest[9], &rest[14..14 + len]));
            rest = &rest[14 + len..];
        }
        rows
    }

    fn finish(mut recorder: Recorder) -> Vec<Vec<u8>> {
        recorder.file.take().unwrap().flush().unwrap();
        let files = fs::read_dir(&recorder.dir).unwrap();
        let mut paths: Vec<_> = files.map(|entry| entry.unwrap().path()).collect();
        paths.sort();
        let bytes = paths.iter().map(|p| fs::read(p).unwrap()).collect();
        for path in paths {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(recorder.dir).unwrap();
        bytes
    }

    #[test]
    fn session_recording_is_inert_without_opt_in() {
        if std::env::var_os("EQOA_RECORD_SESSION").is_some() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env_remove("EQOA_RECORD_SESSION")
                .args([
                    "--exact",
                    "fork_record::tests::session_recording_is_inert_without_opt_in",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let dir = test_recorder("disabled").dir;
        start(&dir, Some("disabled"));
        let mut session = test_session();
        session.state = State::Selecting;
        let world_dump = drdp::Message {
            ty: drdp::TYPE_GUARANTEED,
            connection: session.conn,
            data: vec![0x0D, 0x00],
            ..Default::default()
        };
        receive(&mut session, &world_dump);
        assert!(!ENABLED.load(Ordering::Relaxed));
        assert!(RECORDER.lock().unwrap().is_none());
        assert!(session.record.is_none());
        assert!(!dir.exists());
    }

    #[test]
    fn session_recording_omits_login_and_credential_payloads_before_writing() {
        let mut recorder = test_recorder("privacy");
        let mut session = test_session();
        let mut message = drdp::Message {
            ty: drdp::TYPE_GUARANTEED,
            connection: session.conn,
            data: vec![0x18, 0, 0x55],
            ..Default::default()
        };
        recorder.observe(&mut session, &message).unwrap();
        assert!(
            session.record.is_none() && recorder.file.is_none(),
            "a lobby message must not start a recorder"
        );

        session.state = State::Selecting;
        message.ty = drdp::TYPE_CONNECT;
        message.data.clear();
        recorder.observe(&mut session, &message).unwrap();
        message.ty = drdp::TYPE_GUARANTEED;
        message.data = vec![0x00, 0x7E, 0x55]; // offer applied internally, with no Play event
        recorder.observe(&mut session, &message).unwrap();

        recorder.note_credentials("FakeRecorderAccount", "FakeRecorderPassword");
        for op in [
            0x0000u16, 0x0001, 0x0904, 0x07B3, 0x7F53, 0x7F54, 0x0790, 0x0791, 0x0992, 0x0993,
            0x7E0F, 0x7E65,
        ] {
            let bytes = [op.to_le_bytes().as_slice(), b"opcode canary"].concat();
            session.send(&bytes);
            message.data = bytes;
            recorder.observe(&mut session, &message).unwrap();
        }
        session.state = State::World;
        let password_wide: Vec<u8> = "FakeRecorderPassword"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        for secret in [
            b"FakeRecorderAccount".as_slice(),
            b"FakeRecorderPassword",
            &password_wide,
            &crate::auth::udp_token("FakeRecorderPassword"),
        ] {
            let bytes = [b"\x0b\0", secret].concat();
            session.send(&bytes);
            message.data = bytes;
            recorder.observe(&mut session, &message).unwrap();
        }
        session.send(&[0x19, 0, 0xAA]);
        session.send_state(0x40, &[0x01, 0, 0xBB]); // same prefix as a login opcode
        message.data = vec![0x16, 0, 0xCC];
        recorder.observe(&mut session, &message).unwrap();
        session.send_opcode(0x09B0);
        recorder.drain(&mut session).unwrap();
        let parts = finish(recorder);
        assert_eq!(
            payloads(&parts[0]),
            [
                (0, 255, &[][..]),
                (0, 251, &[0x00, 0x7E, 0x55]),
                (1, 251, &[0x19, 0, 0xAA]),
                (1, 0x40, &[0x01, 0, 0xBB]),
                (0, 251, &[0x16, 0, 0xCC]),
                (1, 251, &[0xB0, 0x09]),
            ]
        );
    }

    #[test]
    fn session_recording_rotates_at_a_complete_message_and_retires_old_parts() {
        let mut recorder = test_recorder("rotation");
        recorder.limit = 25;
        for counter in 0..6 {
            recorder.write(0, 251, &[0x18, 0, counter]).unwrap();
        }
        let parts = finish(recorder);
        assert_eq!(parts.len(), 4);
        for (part, counter) in parts.iter().zip(2..6) {
            assert_eq!(part.len(), 25);
            assert_eq!(payloads(part), [(0, 251, &[0x18, 0, counter][..])]);
        }
    }

    #[test]
    fn session_recording_short_credentials_match_full_fields() {
        use crate::stream::Writer;

        let mut recorder = test_recorder("short-credentials");
        recorder.note_credentials("x", "pw");
        let mut account = Writer::op(0x000B);
        account.string(b"x");
        let mut password = Writer::op(0x000B);
        password.wide("pw");
        let mut game = Writer::op(0x000B);
        game.string(b"extra").wide("spawning");
        let mut token = Writer::op(0x000B);
        token.bytes(&crate::auth::udp_token("pw"));
        let loot = [0x18, 0, 0x55];
        for bytes in [
            account.data.as_slice(),
            &password.data,
            &game.data,
            &token.data,
            &loot,
        ] {
            recorder.write(0, 251, bytes).unwrap();
        }
        let parts = finish(recorder);
        assert_eq!(
            payloads(&parts[0]),
            [(0, 251, game.data.as_slice()), (0, 251, &loot[..])]
        );
    }

    #[test]
    fn session_recording_rotation_continues_when_an_old_part_cannot_be_removed() {
        let mut recorder = test_recorder("rotation-remove-error");
        recorder.limit = 25;
        for counter in 0..4 {
            recorder.write(0, 251, &[0x18, 0, counter]).unwrap();
        }
        let oldest = recorder.parts.front().unwrap().clone();
        fs::remove_file(&oldest).unwrap();
        fs::create_dir(&oldest).unwrap();
        let rotated = recorder.write(0, 251, &[0x18, 0, 4]);
        fs::remove_dir(oldest).unwrap();
        rotated.expect("rotation continues after old-file removal fails");
        recorder.write(0, 251, &[0x18, 0, 5]).unwrap();
        let parts = finish(recorder);
        assert_eq!(parts.len(), 4);
        assert_eq!(payloads(&parts[3]), [(0, 251, &[0x18, 0, 5][..])]);
    }
}
