// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Rooted file access on Linux and macOS: what the open folder handles of `sandbox.rs` give on Windows.
//!
//! * **One root, resolved once.** A root (a mod's folder, its data folder, the mods folder, the configuration
//!   folder) is registered when the host starts and canonicalized then: that is where macOS' `/var` and `/tmp`
//!   links are legitimately followed. From the root down NO link of any owner is followed or tolerated.
//! * **Descent by handle.** Each folder is opened with `openat(O_NOFOLLOW | O_DIRECTORY)` relative to the folder
//!   above it, the handles are held, and the operation itself (read, create, rename, remove, list, stat) is done
//!   relative to the held parent handle. Nothing is opened again by its full path after the check, so a folder
//!   swapped for a link, or moved, between the descent and the operation changes nothing: the operation still
//!   happens in the folder that was opened.
//! * Reads open the file with `O_NOFOLLOW | O_NONBLOCK` (a pipe cannot hang them), insist on a regular file with
//!   `fstat` and cap the bytes read from that very handle.
//!
//! A path under no registered root is anchored at its own parent folder, canonicalized at that call (its links
//! above the parent are followed once, the file itself must not be a link). Only paths no mod can reach (the
//! configuration folder's own files) are in that case.
use std::{
    cell::RefCell,
    ffi::{CStr, CString, OsStr, OsString},
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::{Component, Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// Registered roots: the path as the host was given it, and its canonical form (resolved once).
static ROOTS: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

/// `path` with its deepest existing part canonicalized and the part that does not exist yet appended as written.
fn canon_prefix(path: &Path) -> PathBuf {
    let mut tail: Vec<OsString> = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(canonical) = cur.canonicalize() {
            let mut out = canonical;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (cur.parent().map(Path::to_path_buf), cur.file_name().map(OsStr::to_os_string)) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Makes `root` a root: canonicalized now, once. Registering the same path again re-resolves it.
pub(crate) fn register_root(root: &Path) {
    let canonical = canon_prefix(root);
    let mut roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    match roots.iter_mut().find(|(lexical, _)| lexical == root) {
        Some(entry) => entry.1 = canonical,
        None => roots.push((root.to_path_buf(), canonical)),
    }
}

/// The canonical root a path lies under and the names below it.
fn anchor(path: &Path) -> Result<(PathBuf, Vec<OsString>), String> {
    let names = |rest: &Path| -> Result<Vec<OsString>, String> {
        let mut out = Vec::new();
        for part in rest.components() {
            match part {
                Component::Normal(name) => out.push(name.to_os_string()),
                Component::CurDir => {}
                _ => return Err("invalid addon path".into()),
            }
        }
        Ok(out)
    };
    {
        let roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
        let best = roots
            .iter()
            .filter(|(lexical, _)| path.starts_with(lexical))
            .max_by_key(|(lexical, _)| lexical.as_os_str().len());
        if let Some((lexical, canonical)) = best {
            let rest = path.strip_prefix(lexical).map_err(|_| "invalid addon path".to_string())?;
            return Ok((canonical.clone(), names(rest)?));
        }
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => Ok((canon_prefix(parent), vec![name.to_os_string()])),
        _ => Ok((canon_prefix(path), Vec::new())),
    }
}

/// Folder handles from the root down; the last is the folder an operation happens in.
pub(crate) struct Chain {
    fds: Vec<OwnedFd>,
}
impl Chain {
    fn parent(&self) -> RawFd {
        self.fds.last().expect("a chain holds its root").as_raw_fd()
    }
    /// The handles, to be kept for as long as the caller's guard lives.
    pub(crate) fn into_files(self) -> Vec<File> {
        self.fds.into_iter().map(File::from).collect()
    }
}

/// What a descent found: a folder that is not there (not an error for a read), or the held chain and the name
/// of the last component, which is the file or folder the operation is about.
pub(crate) enum Held {
    Missing,
    At(Chain, Option<OsString>),
}

fn cstring(name: &OsStr) -> Result<CString, String> {
    CString::new(name.as_bytes()).map_err(|_| "invalid addon path".to_string())
}

fn last_errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn link_refusal(errno: i32) -> bool {
    errno == libc::ELOOP || errno == libc::ENOTDIR || errno == libc::EMLINK
}

fn open_dir_following(path: &Path) -> Result<Option<OwnedFd>, String> {
    let c = cstring(path.as_os_str())?;
    // SAFETY: `c` is a valid NUL-terminated string; the result is checked.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if fd >= 0 {
        // SAFETY: `fd` is a new descriptor nothing else owns.
        return Ok(Some(unsafe { OwnedFd::from_raw_fd(fd) }));
    }
    match last_errno() {
        libc::ENOENT => Ok(None),
        e => Err(io::Error::from_raw_os_error(e).to_string()),
    }
}

fn open_dir_at(parent: RawFd, name: &OsStr, create: bool) -> Result<Option<OwnedFd>, String> {
    let c = cstring(name)?;
    for attempt in 0..2 {
        // SAFETY: `parent` is a held descriptor and `c` a valid string; the result is checked.
        let fd = unsafe {
            libc::openat(parent, c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        };
        if fd >= 0 {
            // SAFETY: `fd` is a new descriptor nothing else owns.
            return Ok(Some(unsafe { OwnedFd::from_raw_fd(fd) }));
        }
        let errno = last_errno();
        if link_refusal(errno) {
            return Err("symlink or non-folder in addon path".into());
        }
        if errno != libc::ENOENT {
            return Err(io::Error::from_raw_os_error(errno).to_string());
        }
        if !create {
            return Ok(None);
        }
        if attempt == 0 {
            // SAFETY: as above.
            let made = unsafe { libc::mkdirat(parent, c.as_ptr(), 0o777) };
            if made != 0 && last_errno() != libc::EEXIST {
                return Err(io::Error::last_os_error().to_string());
            }
        }
    }
    Err("folder vanished while it was made".into())
}

thread_local! {
    /// Test seam: runs between the descent and the operation, where a swap would have to happen.
    static AFTER_DESCENT: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) };
}
#[cfg(test)]
pub(crate) fn set_after_descent_for_test(hook: Option<Box<dyn FnMut()>>) {
    AFTER_DESCENT.with(|h| *h.borrow_mut() = hook);
}
fn after_descent() {
    #[cfg(test)]
    AFTER_DESCENT.with(|h| {
        if let Some(hook) = h.borrow_mut().as_mut() {
            hook();
        }
    });
}

/// Holds the folders above `path`'s last component, root first. `create` makes missing folders.
pub(crate) fn descend_parent(path: &Path, create: bool) -> Result<Held, String> {
    let (root, mut names) = anchor(path)?;
    let leaf = names.pop();
    let mut fds = Vec::new();
    match open_dir_following(&root)? {
        Some(fd) => fds.push(fd),
        None if create => {
            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            match open_dir_following(&root)? {
                Some(fd) => fds.push(fd),
                None => return Ok(Held::Missing),
            }
        }
        None => return Ok(Held::Missing),
    }
    for name in &names {
        let parent = fds.last().expect("root held").as_raw_fd();
        match open_dir_at(parent, name, create)? {
            Some(fd) => fds.push(fd),
            None => return Ok(Held::Missing),
        }
    }
    Ok(Held::At(Chain { fds }, leaf))
}

fn open_leaf(parent: RawFd, leaf: &OsStr, flags: libc::c_int, create_mode: libc::c_uint) -> Result<File, String> {
    let c = cstring(leaf)?;
    // SAFETY: `parent` is a held descriptor and `c` a valid string; the result is checked.
    let fd = unsafe { libc::openat(parent, c.as_ptr(), flags | libc::O_NOFOLLOW | libc::O_CLOEXEC, create_mode) };
    if fd < 0 {
        let errno = last_errno();
        return Err(match errno {
            libc::ENOENT => "file not found".to_string(),
            e if link_refusal(e) => "symlink in addon file path".to_string(),
            e => io::Error::from_raw_os_error(e).to_string(),
        });
    }
    // SAFETY: `fd` is a new descriptor nothing else owns.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// The bytes of a regular file, read from the handle opened below the held folders, at most `max`.
pub(crate) fn read_file(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    read_file_inner(path, max).map(|(_, bytes)| bytes)
}

fn read_file_inner(path: &Path, max: usize) -> Result<(File, Vec<u8>), String> {
    let (chain, leaf) = match descend_parent(path, false)? {
        Held::Missing => return Err("file not found".into()),
        Held::At(chain, Some(leaf)) => (chain, leaf),
        Held::At(..) => return Err("addon path names no file".into()),
    };
    after_descent();
    let file = open_leaf(chain.parent(), &leaf, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("addon file is not a regular file".into());
    }
    let mut bytes = Vec::new();
    (&file).take(max as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("file exceeds byte limit".into());
    }
    Ok((file, bytes))
}

/// Opens a regular file for the length of a guard (`lock_resource`): the same checks as a read, no bytes read.
pub(crate) fn open_regular(path: &Path) -> Result<Vec<File>, String> {
    let (chain, leaf) = match descend_parent(path, false)? {
        Held::Missing => return Err("file not found".into()),
        Held::At(chain, Some(leaf)) => (chain, leaf),
        Held::At(..) => return Err("addon path names no file".into()),
    };
    let file = open_leaf(chain.parent(), &leaf, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("addon file is not a regular file".into());
    }
    let mut files = chain.into_files();
    files.push(file);
    Ok(files)
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a new file beside `path` and renames it over `path`, all relative to the held folder.
/// `sync` makes the data durable before the rename; `gate` runs just before it and can refuse. Returns the file,
/// still open, for a later sync.
pub(crate) fn write_replace(
    path: &Path,
    bytes: &[u8],
    sync: bool,
    gate: Option<&dyn Fn() -> io::Result<()>>,
) -> io::Result<File> {
    let (chain, leaf) = match descend_parent(path, true).map_err(io::Error::other)? {
        Held::At(chain, Some(leaf)) => (chain, leaf),
        _ => return Err(io::Error::other("storage target has no parent")),
    };
    after_descent();
    let parent = chain.parent();
    let target = cstring(&leaf).map_err(io::Error::other)?;
    let (name, mut file) = loop {
        let name = format!(".addon-{}-u{}.tmp", std::process::id(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed));
        match open_leaf(parent, OsStr::new(&name), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o666) {
            Ok(file) => break (name, file),
            Err(e) if e.contains("File exists") => continue,
            Err(e) => return Err(io::Error::other(e)),
        }
    };
    let temp = cstring(OsStr::new(&name)).map_err(io::Error::other)?;
    let unlink_temp = || {
        // SAFETY: held descriptor and valid string.
        unsafe { libc::unlinkat(parent, temp.as_ptr(), 0) };
    };
    let mut written = file.write_all(bytes).and_then(|_| file.flush());
    if sync {
        written = written.and_then(|_| file.sync_all());
    }
    if let Err(e) = written {
        unlink_temp();
        return Err(e);
    }
    if let Some(gate) = gate {
        if let Err(e) = gate() {
            unlink_temp();
            return Err(e);
        }
    }
    // SAFETY: held descriptor and valid strings.
    let renamed = unsafe { libc::renameat(parent, temp.as_ptr(), parent, target.as_ptr()) };
    if renamed != 0 {
        let e = io::Error::last_os_error();
        unlink_temp();
        return Err(e);
    }
    Ok(file)
}

/// Removes a file (or an empty folder entry's name) relative to the held folder. A missing file is `NotFound`.
pub(crate) fn remove_file(path: &Path) -> io::Result<()> {
    let (chain, leaf) = match descend_parent(path, false).map_err(io::Error::other)? {
        Held::Missing => return Err(io::Error::from(io::ErrorKind::NotFound)),
        Held::At(chain, Some(leaf)) => (chain, leaf),
        Held::At(..) => return Err(io::Error::other("addon path names no file")),
    };
    after_descent();
    let name = cstring(&leaf).map_err(io::Error::other)?;
    // SAFETY: held descriptor and valid string.
    if unsafe { libc::unlinkat(chain.parent(), name.as_ptr(), 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// What `stat` says about a name, without following a link.
pub(crate) struct Stat {
    pub(crate) is_file: bool,
    pub(crate) is_dir: bool,
    pub(crate) len: u64,
}

fn fstatat_nofollow(parent: RawFd, name: &CStr) -> io::Result<libc::stat> {
    // SAFETY: a zeroed `stat` is a valid out-parameter; the descriptor and string are valid.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatat(parent, name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st)
}

fn stat_of(st: &libc::stat) -> Result<Stat, String> {
    let kind = st.st_mode & libc::S_IFMT;
    if kind == libc::S_IFLNK {
        return Err("symlink in addon file path".into());
    }
    Ok(Stat { is_file: kind == libc::S_IFREG, is_dir: kind == libc::S_IFDIR, len: st.st_size as u64 })
}

/// A file's or folder's kind and size, taken relative to the held folder; a link is refused.
pub(crate) fn stat(path: &Path) -> Result<Stat, String> {
    let (chain, leaf) = match descend_parent(path, false)? {
        Held::Missing => return Err("file not found".into()),
        Held::At(chain, Some(leaf)) => (chain, leaf),
        Held::At(..) => return Err("addon path names no file".into()),
    };
    after_descent();
    let name = cstring(&leaf)?;
    let st = fstatat_nofollow(chain.parent(), &name).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound { "file not found".to_string() } else { e.to_string() }
    })?;
    stat_of(&st)
}

/// The names in a folder, read from the held handle of that folder. A link among them is refused.
pub(crate) fn list_names(dir: &Path) -> Result<Vec<OsString>, String> {
    let (chain, leaf) = match descend_parent(dir, false)? {
        Held::Missing => return Err("file not found".into()),
        Held::At(chain, leaf) => (chain, leaf),
    };
    let folder = match leaf {
        Some(leaf) => match open_dir_at(chain.parent(), &leaf, false)? {
            Some(fd) => fd,
            None => return Err("file not found".into()),
        },
        None => {
            // SAFETY: duplicating a held descriptor.
            let fd = unsafe { libc::dup(chain.parent()) };
            if fd < 0 {
                return Err(io::Error::last_os_error().to_string());
            }
            // SAFETY: `fd` is a new descriptor nothing else owns.
            unsafe { OwnedFd::from_raw_fd(fd) }
        }
    };
    after_descent();
    let raw = folder.as_raw_fd();
    // SAFETY: `fdopendir` takes over the descriptor, so it is given up here (`into_raw_fd`) and closed by `closedir`.
    let handle = unsafe { libc::fdopendir(std::os::fd::IntoRawFd::into_raw_fd(folder)) };
    if handle.is_null() {
        return Err(io::Error::last_os_error().to_string());
    }
    let mut names = Vec::new();
    let mut refusal = None;
    loop {
        // SAFETY: `handle` is an open directory stream.
        let entry = unsafe { libc::readdir(handle) };
        if entry.is_null() {
            break;
        }
        // SAFETY: `readdir` returned a valid entry whose name is NUL-terminated.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_owned();
        let bytes = name.as_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        // SAFETY: as above.
        let d_type = unsafe { (*entry).d_type };
        let is_link = if d_type == libc::DT_LNK {
            true
        } else if d_type == libc::DT_UNKNOWN {
            match fstatat_nofollow(raw, &name) {
                Ok(st) => st.st_mode & libc::S_IFMT == libc::S_IFLNK,
                Err(_) => false,
            }
        } else {
            false
        };
        if is_link {
            refusal = Some("symlink in addon file path".to_string());
            break;
        }
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    // SAFETY: closes the stream and the descriptor it owns.
    unsafe { libc::closedir(handle) };
    match refusal {
        Some(e) => Err(e),
        None => Ok(names),
    }
}

/// A static check for the code that wants to refuse early: no link in `path` below its root.
pub(crate) fn links_below_root(path: &Path) -> Result<(), String> {
    let (root, names) = anchor(path)?;
    let mut current = root;
    for name in names {
        current.push(name);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => return Err("symlink in addon file path".into()),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
