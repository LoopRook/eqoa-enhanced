// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Mod sound on Linux and macOS (`eqoa.sound.*`): WAV files decoded once into 48 kHz stereo and mixed into the game's
//! own output stream. Windows keeps its MCI path (`sound.rs`), which plays whatever Windows can decode; here only WAV
//! plays and everything else is refused with its format named.
//!
//! `SoundBank` sends MCI-style command strings to a backend; `MixBackend` is that backend here. The game's audio output
//! calls `mix_into_stream` (one hook line in `audio.rs`) on the frames it has just mixed, so the game's Master Volume,
//! Sound in Background and device choice apply to mod sound too.
//!
//! The output callback must never wait: `Mixer::mix` takes its lock with `try_lock` and a missed lock is one block
//! of silence for mod sound, never a stall; it allocates nothing (clips are decoded and shared as `Arc<[i16]>` on
//! the game's thread, long before).

#![cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

/// The rate the game mixes at (`audio.rs`): every clip is resampled to it once, at load.
pub(crate) const RATE: u32 = 48_000;
/// The longest clip kept (two minutes at 48 kHz): longer files are refused, not truncated.
const MAX_FRAMES: usize = RATE as usize * 120;
/// All decoded clips of one bank together (`MAX_AGGREGATE_BYTES` of `sound.rs`, in decoded bytes).
const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

/// What a file is, by its first bytes, for the refusal's wording.
pub(crate) fn sniff(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        "wav"
    } else if bytes.starts_with(b"ID3") || (bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0) {
        "mp3"
    } else if bytes.starts_with(b"OggS") {
        "ogg"
    } else if bytes.starts_with(b"fLaC") {
        "flac"
    } else if bytes.len() >= 12 && &bytes[..4] == b"FORM" {
        "aiff"
    } else if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        "mp4/m4a"
    } else if bytes.starts_with(&[0x30, 0x26, 0xB2, 0x75]) {
        "wma"
    } else {
        "an unknown format"
    }
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A WAV file as interleaved 48 kHz stereo `i16`. PCM 8 and 16 bit and IEEE float 32 and 64 bit, one or two channels
/// (also through `WAVE_FORMAT_EXTENSIBLE`); anything else is refused in words.
pub(crate) fn decode_wav(bytes: &[u8]) -> Result<Vec<i16>, String> {
    let kind = sniff(bytes);
    if kind != "wav" {
        return Err(format!("unsupported sound format: {kind} (only WAV plays on this system)"));
    }
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = le32(bytes, at + 4).ok_or("truncated WAV")? as usize;
        let body = at + 8;
        let end = body.saturating_add(len).min(bytes.len());
        match id {
            b"fmt " => fmt = Some(&bytes[body..end]),
            b"data" => {
                data = Some(&bytes[body..end]);
                break;
            }
            _ => {}
        }
        at = body.saturating_add(len).saturating_add(len & 1);
    }
    let fmt = fmt.ok_or("WAV has no fmt chunk")?;
    let data = data.ok_or("WAV has no data chunk")?;
    let mut tag = le16(fmt, 0).ok_or("truncated WAV fmt")?;
    let channels = le16(fmt, 2).ok_or("truncated WAV fmt")? as usize;
    let rate = le32(fmt, 4).ok_or("truncated WAV fmt")?;
    let bits = le16(fmt, 14).ok_or("truncated WAV fmt")?;
    if tag == 0xFFFE {
        tag = le16(fmt, 24).ok_or("truncated WAV extensible fmt")?;
    }
    if !(1..=2).contains(&channels) {
        return Err(format!("unsupported sound format: WAV with {channels} channels (one or two play)"));
    }
    if !(1000..=384_000).contains(&rate) {
        return Err(format!("unsupported sound format: WAV at {rate} Hz"));
    }
    let width = match (tag, bits) {
        (1, 8) | (1, 16) | (3, 32) | (3, 64) => bits as usize / 8,
        _ => {
            let what = match tag {
                1 => "PCM",
                3 => "float",
                2 | 17 => "ADPCM",
                85 => "MP3-in-WAV",
                _ => "compressed",
            };
            return Err(format!("unsupported sound format: {bits}-bit {what} WAV (8/16-bit PCM and 32/64-bit float play)"));
        }
    };
    let frame = width * channels;
    let n = data.len() / frame;
    let sample = |i: usize, c: usize| -> f32 {
        let o = i * frame + c * width;
        match (tag, width) {
            (1, 1) => (data[o] as f32 - 128.0) / 128.0,
            (1, 2) => i16::from_le_bytes([data[o], data[o + 1]]) as f32 / 32768.0,
            (3, 4) => f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]),
            _ => f64::from_le_bytes(data[o..o + 8].try_into().unwrap()) as f32,
        }
    };
    if n == 0 {
        return Err("WAV holds no samples".into());
    }
    let out_frames = ((n as u64 * RATE as u64) / rate as u64).max(1) as usize;
    if out_frames > MAX_FRAMES {
        return Err(format!("sound too long: {} s (at most {} s plays)", out_frames / RATE as usize, MAX_FRAMES / RATE as usize));
    }
    let mut out = Vec::with_capacity(out_frames * 2);
    let step = rate as f64 / RATE as f64;
    for k in 0..out_frames {
        let x = k as f64 * step;
        let i = (x as usize).min(n - 1);
        let j = (i + 1).min(n - 1);
        let t = (x - i as f64) as f32;
        for c in 0..2 {
            let c = c.min(channels - 1);
            let v = sample(i, c) * (1.0 - t) + sample(j, c) * t;
            out.push((v.clamp(-1.0, 1.0) * 32767.0).round() as i16);
        }
    }
    Ok(out)
}

struct Voice {
    clip: Arc<[i16]>,
    /// Next frame to play.
    pos: usize,
    gain: f32,
    looping: bool,
    playing: bool,
}

/// The voices playing into the game's stream. One shared by the game (`global`); tests make their own.
#[derive(Default)]
pub(crate) struct Mixer {
    voices: HashMap<String, Voice>,
}

impl Mixer {
    /// Adds the voices to interleaved stereo `pcm` (48 kHz, as `audio.rs` mixes). Allocates nothing.
    fn mix_voices(&mut self, pcm: &mut [i16]) {
        for voice in self.voices.values_mut().filter(|v| v.playing) {
            let frames = voice.clip.len() / 2;
            for out in pcm.chunks_exact_mut(2) {
                if voice.pos >= frames {
                    if voice.looping && frames > 0 {
                        voice.pos = 0;
                    } else {
                        voice.playing = false;
                        voice.pos = 0;
                        break;
                    }
                }
                let at = voice.pos * 2;
                for c in 0..2 {
                    let v = out[c] as f32 + voice.clip[at + c] as f32 * voice.gain;
                    out[c] = v.clamp(-32768.0, 32767.0) as i16;
                }
                voice.pos += 1;
            }
        }
    }
}

type Shared = Arc<Mutex<Mixer>>;

fn global() -> &'static Shared {
    static MIXER: OnceLock<Shared> = OnceLock::new();
    MIXER.get_or_init(Default::default)
}

/// The hook `audio.rs` calls on every block it has mixed: adds the mod sounds that are playing. A mixer another thread
/// holds at that moment costs this block its mod sound (silence), never a wait.
pub(crate) fn mix_into_stream(pcm: &mut [i16]) {
    mix_shared(global(), pcm);
}

fn mix_shared(shared: &Shared, pcm: &mut [i16]) {
    if let Ok(mut mixer) = shared.try_lock() {
        mixer.mix_voices(pcm);
    }
}

/// The `MciBackend` of Linux and macOS: reads the six commands `SoundBank` sends.
pub(crate) struct MixBackend {
    mixer: Shared,
    clips: HashMap<String, Arc<[i16]>>,
    decoded_bytes: usize,
}

impl MixBackend {
    pub(crate) fn new() -> Self {
        Self::with_mixer(global().clone())
    }
    pub(crate) fn with_mixer(mixer: Shared) -> Self {
        Self { mixer, clips: HashMap::new(), decoded_bytes: 0 }
    }
    fn with_voice<T>(&self, alias: &str, f: impl FnOnce(&mut Voice) -> T) -> Result<T, String> {
        let mut mixer = self.mixer.lock().map_err(|_| "mixer unavailable".to_string())?;
        mixer.voices.get_mut(alias).map(f).ok_or_else(|| format!("unknown sound alias {alias}"))
    }
    fn open(&mut self, rest: &str) -> Result<(), String> {
        // open "<path>" type mpegvideo alias NAME
        let rest = rest.strip_prefix('"').ok_or("open needs a quoted path")?;
        let (path, tail) = rest.rsplit_once("\" type mpegvideo alias ").ok_or("malformed open command")?;
        let alias = tail.trim().to_string();
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let clip: Arc<[i16]> = decode_wav(&bytes)?.into();
        let size = clip.len() * 2;
        if self.decoded_bytes + size > MAX_DECODED_BYTES {
            return Err("decoded sounds exceed the 64 MiB limit".into());
        }
        self.decoded_bytes += size;
        self.mixer
            .lock()
            .map_err(|_| "mixer unavailable".to_string())?
            .voices
            .insert(alias.clone(), Voice { clip: clip.clone(), pos: 0, gain: 1.0, looping: false, playing: false });
        self.clips.insert(alias, clip);
        Ok(())
    }
}

impl super::sound::MciBackend for MixBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn send(&mut self, command: &str) -> Result<String, String> {
        let (verb, rest) = command.split_once(' ').unwrap_or((command, ""));
        if verb == "open" {
            return self.open(rest).map(|()| String::new());
        }
        let mut words = rest.split_whitespace();
        let alias = words.next().ok_or("command names no sound")?.to_string();
        match verb {
            "play" => {
                let looping = words.next() == Some("repeat");
                self.with_voice(&alias, |v| {
                    v.looping = looping;
                    v.playing = true;
                })?;
                Ok(String::new())
            }
            "stop" => self.with_voice(&alias, |v| v.playing = false).map(|()| String::new()),
            "seek" => self.with_voice(&alias, |v| v.pos = 0).map(|()| String::new()),
            "status" => self
                .with_voice(&alias, |v| if v.playing { "playing" } else { "stopped" })
                .map(str::to_string),
            "setaudio" => {
                // setaudio NAME volume to 0..1000
                let value: f32 = words.nth(2).and_then(|n| n.parse().ok()).ok_or("setaudio needs a volume")?;
                self.with_voice(&alias, |v| v.gain = (value / 1000.0).clamp(0.0, 1.0)).map(|()| String::new())
            }
            "close" => {
                if let Some(clip) = self.clips.remove(&alias) {
                    self.decoded_bytes = self.decoded_bytes.saturating_sub(clip.len() * 2);
                }
                self.mixer.lock().map_err(|_| "mixer unavailable".to_string())?.voices.remove(&alias);
                Ok(String::new())
            }
            other => Err(format!("unsupported sound command {other}")),
        }
    }
}

impl Drop for MixBackend {
    /// A bank that goes away takes its voices with it (the aliases it opened).
    fn drop(&mut self) {
        if let Ok(mut mixer) = self.mixer.lock() {
            for alias in self.clips.keys() {
                mixer.voices.remove(alias);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::sound::MciBackend;

    /// A WAV built in the test: `samples` per channel, PCM 16-bit.
    pub(crate) fn wav16(rate: u32, channels: u16, frames: &[Vec<i16>]) -> Vec<u8> {
        let mut data = Vec::new();
        for f in frames {
            for s in f {
                data.extend_from_slice(&s.to_le_bytes());
            }
        }
        wav(1, rate, channels, 16, &data)
    }
    pub(crate) fn wav(tag: u16, rate: u32, channels: u16, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * channels as u32 * bits as u32 / 8).to_le_bytes());
        out.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    fn scratch(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("eqoa-mix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn a_48k_stereo_wav_decodes_to_itself() {
        let w = wav16(48_000, 2, &[vec![1000, -1000], vec![2000, -2000]]);
        assert_eq!(decode_wav(&w).unwrap(), vec![1000, -1000, 2000, -2000]);
    }

    #[test]
    fn mono_is_made_stereo_and_8_bit_is_centred() {
        let w = wav16(48_000, 1, &[vec![500], vec![-500]]);
        assert_eq!(decode_wav(&w).unwrap(), vec![500, 500, -500, -500]);
        let w = wav(1, 48_000, 1, 8, &[128, 255]);
        let d = decode_wav(&w).unwrap();
        assert_eq!(&d[..2], &[0, 0]);
        assert!(d[2] > 32000 && d[2] == d[3]);
    }

    #[test]
    fn float_wav_decodes_and_clips() {
        let mut data = Vec::new();
        for v in [0.5f32, -2.0] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let d = decode_wav(&wav(3, 48_000, 1, 32, &data)).unwrap();
        assert_eq!(d, vec![16384, 16384, -32767, -32767]);
    }

    #[test]
    fn a_24_khz_wav_is_resampled_to_twice_the_frames() {
        let w = wav16(24_000, 1, &[vec![0], vec![1000], vec![1000], vec![1000]]);
        let d = decode_wav(&w).unwrap();
        assert_eq!(d.len(), 4 * 2 * 2);
        assert_eq!(d[0], 0);
        assert_eq!(d[2], 500, "halfway between the first two source samples");
    }

    #[test]
    fn other_formats_are_refused_with_their_names() {
        let err = |b: &[u8]| decode_wav(b).unwrap_err();
        assert!(err(b"ID3\x03\0\0\0\0\0\0xxxxxxxx").contains("mp3"));
        assert!(err(b"OggSxxxxxxxxxxxx").contains("ogg"));
        assert!(err(b"fLaCxxxxxxxxxxxx").contains("flac"));
        assert!(err(&wav(1, 48_000, 1, 24, &[0; 6])).contains("24-bit PCM"));
        assert!(err(&wav(2, 48_000, 1, 4, &[0; 6])).contains("ADPCM"));
        assert!(err(&wav16(48_000, 2, &[vec![0, 0]]).iter().map(|b| *b).take(20).collect::<Vec<_>>()).contains("WAV"));
    }

    #[test]
    fn the_mix_adds_clips_scales_by_gain_clips_the_sum_and_ends_non_looping_voices() {
        let mut mixer = Mixer::default();
        mixer.voices.insert(
            "a".into(),
            Voice { clip: vec![1000i16, 1000, 2000, 2000].into(), pos: 0, gain: 0.5, looping: false, playing: true },
        );
        mixer.voices.insert(
            "loud".into(),
            Voice { clip: vec![32000i16, 32000].into(), pos: 0, gain: 1.0, looping: false, playing: true },
        );
        let mut pcm = [100i16, 100, 100, 100, 100, 100];
        mixer.mix_voices(&mut pcm);
        // frame 0: 100 + 500 + 32000 (clipped at 32767 only if over); frame 1: 100 + 1000; frame 2: untouched
        assert_eq!(pcm[0], 100 + 500 + 32000);
        assert_eq!(pcm[2], 100 + 1000);
        assert_eq!(pcm[4], 100);
        assert!(!mixer.voices["a"].playing, "the clip ended");
        let mut loud = [20000i16, 20000];
        mixer.voices.get_mut("loud").unwrap().playing = true;
        mixer.mix_voices(&mut loud);
        assert_eq!(loud[0], 32767, "the sum is clipped, not wrapped");
    }

    #[test]
    fn a_looping_voice_wraps_and_a_stopped_one_is_silent() {
        let mut mixer = Mixer::default();
        mixer.voices.insert(
            "l".into(),
            Voice { clip: vec![10i16, 10].into(), pos: 0, gain: 1.0, looping: true, playing: true },
        );
        let mut pcm = [0i16; 6];
        mixer.mix_voices(&mut pcm);
        assert_eq!(pcm, [10; 6]);
        mixer.voices.get_mut("l").unwrap().playing = false;
        let mut quiet = [0i16; 4];
        mixer.mix_voices(&mut quiet);
        assert_eq!(quiet, [0; 4]);
    }

    #[test]
    fn a_held_mixer_costs_the_block_its_mod_sound_and_no_wait() {
        let shared: Shared = Default::default();
        shared.lock().unwrap().voices.insert(
            "a".into(),
            Voice { clip: vec![500i16, 500].into(), pos: 0, gain: 1.0, looping: true, playing: true },
        );
        let mut pcm = [0i16; 2];
        {
            let _held = shared.lock().unwrap();
            let start = std::time::Instant::now();
            mix_shared(&shared, &mut pcm);
            assert!(start.elapsed() < std::time::Duration::from_millis(50), "the callback must not wait");
        }
        assert_eq!(pcm, [0, 0], "silence for that block");
        mix_shared(&shared, &mut pcm);
        assert_eq!(pcm, [500, 500], "control: free, the sound is mixed");
    }

    #[test]
    fn the_backend_opens_plays_queries_sets_volume_stops_and_closes() {
        let shared: Shared = Default::default();
        let mut b = MixBackend::with_mixer(shared.clone());
        let p = scratch("beep.wav", &wav16(48_000, 1, &[vec![1000]; 4]));
        let open = format!("open \"{}\" type mpegvideo alias s1", p.display());
        b.send(&open).unwrap();
        assert_eq!(b.send("status s1 mode").unwrap(), "stopped");
        b.send("setaudio s1 volume to 500").unwrap();
        b.send("seek s1 to start").unwrap();
        b.send("play s1").unwrap();
        assert_eq!(b.send("status s1 mode").unwrap(), "playing");
        let mut pcm = [0i16; 4];
        mix_shared(&shared, &mut pcm);
        assert_eq!(pcm, [500; 4], "half volume");
        let mut rest = [0i16; 8];
        mix_shared(&shared, &mut rest);
        assert_eq!(b.send("status s1 mode").unwrap(), "stopped", "the clip ended by itself");
        b.send("play s1 repeat").unwrap();
        b.send("stop s1").unwrap();
        let mut silent = [0i16; 2];
        mix_shared(&shared, &mut silent);
        assert_eq!(silent, [0, 0]);
        b.send("close s1").unwrap();
        assert!(b.send("status s1 mode").is_err(), "a closed sound is gone");
        assert!(shared.lock().unwrap().voices.is_empty());
    }

    #[test]
    fn opening_an_mp3_is_refused_and_leaves_nothing_behind() {
        let shared: Shared = Default::default();
        let mut b = MixBackend::with_mixer(shared.clone());
        let p = scratch("song.mp3", b"ID3\x03\0\0\0\0\0\0\xFF\xFB\x90\0");
        let err = b.send(&format!("open \"{}\" type mpegvideo alias s2", p.display())).unwrap_err();
        assert!(err.contains("mp3"), "{err}");
        assert!(shared.lock().unwrap().voices.is_empty());
        // control: the same call with a WAV opens
        let w = scratch("ok.wav", &wav16(48_000, 1, &[vec![1]]));
        assert!(b.send(&format!("open \"{}\" type mpegvideo alias s3", w.display())).is_ok());
    }

    #[test]
    fn a_bank_that_goes_away_takes_its_voices() {
        let shared: Shared = Default::default();
        {
            let mut b = MixBackend::with_mixer(shared.clone());
            let w = scratch("gone.wav", &wav16(48_000, 1, &[vec![1]]));
            b.send(&format!("open \"{}\" type mpegvideo alias s4", w.display())).unwrap();
            assert_eq!(shared.lock().unwrap().voices.len(), 1);
        }
        assert!(shared.lock().unwrap().voices.is_empty());
    }
}
