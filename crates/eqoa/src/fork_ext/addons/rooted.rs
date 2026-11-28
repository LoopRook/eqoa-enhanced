// Code in this file is LoopRook's addition for EQOA Enhanced and is under the GNU Affero General Public License v3 or later (see LICENSE).
//! Rooted file access on Linux and macOS: what the open folder handles of `sandbox.rs` give on Windows.
//!
//! * **Roots.** A root (the mods folder, a mod's folder, the configuration folder's `addons`) is registered when
//!   the host starts using it. A link AT or ABOVE a root is the player's own arrangement (macOS' `/var` and `/tmp`
//!   are links, a player may link a whole mods folder to another disk): it is followed exactly once, at
//!   registration, and the root's folder handle is then held for the life of the registration. Every descent
//!   starts from that held handle, never from the root's path, so a root replaced by a link later has no effect.
//!   A folder registered below another root is not a root: it is descended from the outer one, so a link there
//!   (a `data` or package folder made a link) is a link below a root and is refused.
//! * **Both spellings.** A path is matched against a root's lexical spelling and its canonical one, so a path a
//!   caller canonicalized lands under the same root as the path it was given. A path under no registered root is
//!   refused (`Scope::Mod`); only the host's own configuration files (`Scope::Internal`) are anchored at their
//!   canonicalized parent folder, and no mod can name those.
//! * **Descent by handle.** Each folder below the root is opened with `openat(O_NOFOLLOW | O_DIRECTORY)` relative
//!   to the folder above it, the handles are held, and the operation (read, create, rename, remove, list, stat) is
//!   done relative to the held parent handle: nothing is opened again by its full path after the check.
//! * Reads open the file with `O_NOFOLLOW | O_NONBLOCK` (a pipe cannot hang them), insist on a regular file with
//!   `fstat` and cap the bytes read from that very handle.
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
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// Who is asking: a mod (only registered roots) or the host about its own configuration files.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Mod,
    Internal,
}

/// A registered root: its two spellings and the handle of its folder, held once it exists.
struct Root {
    lexical: PathBuf,
    canonical: PathBuf,
    fd: Mutex<Option<Arc<OwnedFd>>>,
}

static ROOTS: Mutex<Vec<Arc<Root>>> = Mutex::new(Vec::new());

fn lock_roots() -> std::sync::MutexGuard<'static, Vec<Arc<Root>>> {
    ROOTS.lock().unwrap_or_else(|e| e.into_inner())
}

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

fn under(path: &Path, root: &Root) -> bool {
    path.starts_with(&root.lexical) || path.starts_with(&root.canonical)
}

/// Makes `root` a root: its links are followed now, once, and its folder handle is held. A path that lies under a
/// root already registered is not a root of its own (it is descended from that one). Registering a path again
/// (a mod enabled again) resolves it afresh; registering a folder that holds registered roots takes them over.
pub(crate) fn register_root(root: &Path) {
    let canonical = canon_prefix(root);
    let mut roots = lock_roots();
    let probe = Root { lexical: root.to_path_buf(), canonical: canonical.clone(), fd: Mutex::new(None) };
    if roots.iter().any(|r| r.lexical != probe.lexical && under(&probe.lexical, r) && under(&probe.canonical, r)) {
        return;
    }
    roots.retain(|r| !(under(&r.lexical, &probe) || under(&r.canonical, &probe)));
    let held = open_dir_following(&canonical).ok().flatten().map(Arc::new);
    roots.push(Arc::new(Root { lexical: probe.lexical, canonical, fd: Mutex::new(held) }));
}

/// Test seam: other tests register whatever parent folder their data lives in (often the system temp folder), which
/// would make everything below it a folder of that root. A test about roots takes the roots that contain its own
/// folder out first, so what it checks is its own registrations.
#[cfg(test)]
pub(crate) fn isolate_for_test(path: &Path) {
    lock_roots().retain(|r| !under(path, r));
}

/// A mod's folders: the mods folder it came from, its script folder and its package folder.
pub(crate) fn register_addon(addon: &super::discovery::DiscoveredAddon) {
    if let Some(mods) = &addon.mods_root {
        register_root(mods);
    }
    register_root(&addon.script_root);
    register_root(&addon.package_root);
}

fn names_of(rest: &Path) -> Result<Vec<OsString>, String> {
    let mut out = Vec::new();
    for part in rest.components() {
        match part {
            Component::Normal(name) => out.push(name.to_os_string()),
            Component::CurDir => {}
            _ => return Err("invalid addon path".into()),
        }
    }
    Ok(out)
}

/// The root a path lies under (either spelling, the longest) and the names below it.
fn anchor(path: &Path, scope: Scope) -> Result<(Arc<Root>, Vec<OsString>), String> {
    {
        let roots = lock_roots();
        let best = roots
            .iter()
            .filter(|r| under(path, r))
            .max_by_key(|r| {
                let a = if path.starts_with(&r.lexical) { r.lexical.as_os_str().len() } else { 0 };
                let b = if path.starts_with(&r.canonical) { r.canonical.as_os_str().len() } else { 0 };
                a.max(b)
            });
        if let Some(root) = best {
            let rest = if path.starts_with(&root.lexical) {
                path.strip_prefix(&root.lexical)
            } else {
                path.strip_prefix(&root.canonical)
            }
            .map_err(|_| "invalid addon path".to_string())?;
            return Ok((root.clone(), names_of(rest)?));
        }
    }
    if scope == Scope::Mod {
        return Err("addon path is outside every registered root".into());
    }
    // The host's own configuration files: anchored at their canonicalized parent folder.
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => {
            let canonical = canon_prefix(parent);
            let root = Root { lexical: parent.to_path_buf(), canonical, fd: Mutex::new(None) };
            Ok((Arc::new(root), vec![name.to_os_string()]))
        }
        _ => Err("invalid addon path".into()),
    }
}

/// The folder handles an operation goes through: the root's held handle, then each folder below it.
pub(crate) struct Chain {
    root: Arc<OwnedFd>,
    fds: Vec<OwnedFd>,
}
impl Chain {
    fn parent(&self) -> RawFd {
        self.fds.last().map(AsRawFd::as_raw_fd).unwrap_or_else(|| self.root.as_raw_fd())
    }
    /// The handles below the root, to be kept for as long as the caller's guard lives.
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

/// Runs a system call again while a signal interrupts it.
fn retry<T: PartialEq + Copy>(failed: T, mut call: impl FnMut() -> T) -> T {
    loop {
        let result = call();
        if result == failed && last_errno() == libc::EINTR {
            continue;
        }
        return result;
    }
}

fn link_refusal(errno: i32) -> bool {
    errno == libc::ELOOP || errno == libc::ENOTDIR || errno == libc::EMLINK
}

fn open_dir_following(path: &Path) -> Result<Option<OwnedFd>, String> {
    let c = cstring(path.as_os_str())?;
    // SAFETY: `c` is a valid NUL-terminated string; the result is checked.
    let fd = retry(-1, || unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) });
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
        let fd = retry(-1, || unsafe {
            libc::openat(parent, c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        });
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
            let made = retry(-1, || unsafe { libc::mkdirat(parent, c.as_ptr(), 0o777) });
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

/// The root's handle: the one held since registration, or (a root that did not exist then) opened now and held.
fn root_fd(root: &Root, create: bool) -> Result<Option<Arc<OwnedFd>>, String> {
    let mut held = root.fd.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(fd) = held.as_ref() {
        return Ok(Some(fd.clone()));
    }
    let mut opened = open_dir_following(&root.canonical)?;
    if opened.is_none() && create {
        std::fs::create_dir_all(&root.canonical).map_err(|e| e.to_string())?;
        opened = open_dir_following(&root.canonical)?;
    }
    let fd = opened.map(Arc::new);
    *held = fd.clone();
    Ok(fd)
}

/// Holds the folders above `path`'s last component, from the root's held handle down. `create` makes missing folders.
pub(crate) fn descend_parent(path: &Path, create: bool) -> Result<Held, String> {
    descend_parent_in(path, create, Scope::Mod)
}

fn descend_parent_in(path: &Path, create: bool, scope: Scope) -> Result<Held, String> {
    let (root, mut names) = anchor(path, scope)?;
    let leaf = names.pop();
    let Some(root_fd) = root_fd(&root, create)? else {
        return Ok(Held::Missing);
    };
    let mut fds: Vec<OwnedFd> = Vec::new();
    for name in &names {
        let parent = fds.last().map(AsRawFd::as_raw_fd).unwrap_or_else(|| root_fd.as_raw_fd());
        match open_dir_at(parent, name, create)? {
            Some(fd) => fds.push(fd),
            None => return Ok(Held::Missing),
        }
    }
    Ok(Held::At(Chain { root: root_fd, fds }, leaf))
}

/// Why a leaf could not be opened: the system's error number, and the words a mod may be shown.
struct OpenFailure {
    errno: i32,
    text: String,
}

fn open_leaf_raw(parent: RawFd, leaf: &OsStr, flags: libc::c_int, create_mode: libc::c_uint) -> Result<File, OpenFailure> {
    let c = cstring(leaf).map_err(|text| OpenFailure { errno: 0, text })?;
    // SAFETY: `parent` is a held descriptor and `c` a valid string; the result is checked.
    let fd = retry(-1, || unsafe {
        libc::openat(parent, c.as_ptr(), flags | libc::O_NOFOLLOW | libc::O_CLOEXEC, create_mode)
    });
    if fd < 0 {
        let errno = last_errno();
        let text = match errno {
            libc::ENOENT => "file not found".to_string(),
            e if link_refusal(e) => "symlink in addon file path".to_string(),
            e => io::Error::from_raw_os_error(e).to_string(),
        };
        return Err(OpenFailure { errno, text });
    }
    // SAFETY: `fd` is a new descriptor nothing else owns.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn open_leaf(parent: RawFd, leaf: &OsStr, flags: libc::c_int, create_mode: libc::c_uint) -> Result<File, String> {
    open_leaf_raw(parent, leaf, flags, create_mode).map_err(|f| f.text)
}

/// The bytes of a regular file, read from the handle opened below the held folders, at most `max`.
pub(crate) fn read_file(path: &Path, max: usize) -> Result<Vec<u8>, String> {
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
    Ok(bytes)
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

/// What runs the rename: `commit` is given the rename itself and decides when to do it, so a caller that must keep
/// a lock across the check and the rename (the JSON saves' generation order) can.
pub(crate) type Commit<'a> = &'a dyn Fn(&dyn Fn() -> io::Result<()>) -> io::Result<()>;

/// Writes `bytes` to a new file beside `path` and renames it over `path`, all relative to the held folder.
/// `sync` makes the data durable before the rename. Returns the file, still open, for a later sync.
pub(crate) fn write_replace(path: &Path, bytes: &[u8], sync: bool, commit: Option<Commit<'_>>) -> io::Result<File> {
    write_replace_in(path, bytes, sync, commit, Scope::Mod)
}

/// The same for a file of the host's own configuration folder.
pub(crate) fn write_replace_internal(path: &Path, bytes: &[u8], sync: bool) -> io::Result<File> {
    write_replace_in(path, bytes, sync, None, Scope::Internal)
}

fn write_replace_in(path: &Path, bytes: &[u8], sync: bool, commit: Option<Commit<'_>>, scope: Scope) -> io::Result<File> {
    let (chain, leaf) = match descend_parent_in(path, true, scope).map_err(io::Error::other)? {
        Held::At(chain, Some(leaf)) => (chain, leaf),
        _ => return Err(io::Error::other("storage target has no parent")),
    };
    after_descent();
    let parent = chain.parent();
    let target = cstring(&leaf).map_err(io::Error::other)?;
    let (name, mut file) = loop {
        let name = format!(".addon-{}-u{}.tmp", std::process::id(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed));
        match open_leaf_raw(parent, OsStr::new(&name), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o666) {
            Ok(file) => break (name, file),
            Err(f) if f.errno == libc::EEXIST => continue,
            Err(f) => return Err(io::Error::other(f.text)),
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
    let rename = || -> io::Result<()> {
        // SAFETY: held descriptor and valid strings.
        let renamed = retry(-1, || unsafe { libc::renameat(parent, temp.as_ptr(), parent, target.as_ptr()) });
        if renamed != 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    };
    let result = match commit {
        Some(commit) => commit(&rename),
        None => rename(),
    };
    if let Err(e) = result {
        unlink_temp();
        return Err(e);
    }
    Ok(file)
}

/// Removes a file relative to the held folder. A missing file is `NotFound`.
pub(crate) fn remove_file(path: &Path) -> io::Result<()> {
    let (chain, leaf) = match descend_parent(path, false).map_err(io::Error::other)? {
        Held::Missing => return Err(io::Error::from(io::ErrorKind::NotFound)),
        Held::At(chain, Some(leaf)) => (chain, leaf),
        Held::At(..) => return Err(io::Error::other("addon path names no file")),
    };
    after_descent();
    let name = cstring(&leaf).map_err(io::Error::other)?;
    // SAFETY: held descriptor and valid string.
    if retry(-1, || unsafe { libc::unlinkat(chain.parent(), name.as_ptr(), 0) }) != 0 {
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
    if retry(-1, || unsafe { libc::fstatat(parent, name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) }) != 0 {
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

/// Reads a folder from its held handle, one entry at a time: `visit` is told each name and whether it is a link, and
/// may stop the listing by returning an error. A listing that cannot be read to its end is an error, never short.
pub(crate) fn list_each<E>(
    dir: &Path,
    mut visit: impl FnMut(OsString, bool) -> Result<(), E>,
    io_error: impl Fn(String) -> E,
) -> Result<(), E> {
    let (chain, leaf) = match descend_parent(dir, false).map_err(&io_error)? {
        Held::Missing => return Err(io_error("file not found".into())),
        Held::At(chain, leaf) => (chain, leaf),
    };
    let folder = match leaf {
        Some(leaf) => match open_dir_at(chain.parent(), &leaf, false).map_err(&io_error)? {
            Some(fd) => fd,
            None => return Err(io_error("file not found".into())),
        },
        None => {
            // SAFETY: duplicating a held descriptor, close-on-exec.
            let fd = unsafe { libc::fcntl(chain.parent(), libc::F_DUPFD_CLOEXEC, 0) };
            if fd < 0 {
                return Err(io_error(io::Error::last_os_error().to_string()));
            }
            // SAFETY: `fd` is a new descriptor nothing else owns.
            unsafe { OwnedFd::from_raw_fd(fd) }
        }
    };
    after_descent();
    let raw = folder.as_raw_fd();
    let raw_for_dir = std::os::fd::IntoRawFd::into_raw_fd(folder);
    // SAFETY: `fdopendir` takes over the descriptor on success; on failure it stays ours and is closed here.
    let handle = unsafe { libc::fdopendir(raw_for_dir) };
    if handle.is_null() {
        let e = io::Error::last_os_error().to_string();
        // SAFETY: the descriptor was not taken over.
        unsafe { libc::close(raw_for_dir) };
        return Err(io_error(e));
    }
    let outcome = (|| -> Result<(), E> {
        loop {
            clear_errno();
            // SAFETY: `handle` is an open directory stream.
            let entry = unsafe { libc::readdir(handle) };
            if entry.is_null() {
                return match last_errno() {
                    0 => Ok(()),
                    e => Err(io_error(io::Error::from_raw_os_error(e).to_string())),
                };
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
                // The file system does not say: ask, and when that cannot be answered treat it as a link (closed).
                match fstatat_nofollow(raw, &name) {
                    Ok(st) => st.st_mode & libc::S_IFMT == libc::S_IFLNK,
                    Err(_) => true,
                }
            } else {
                false
            };
            visit(OsString::from_vec(bytes.to_vec()), is_link)?;
        }
    })();
    // SAFETY: closes the stream and the descriptor it owns.
    unsafe { libc::closedir(handle) };
    outcome
}

fn clear_errno() {
    // SAFETY: the thread's own errno location.
    unsafe {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            *libc::__errno_location() = 0;
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
        {
            *libc::__error() = 0;
        }
    }
}

/// The names in a folder (a link among them refuses the listing).
pub(crate) fn list_names(dir: &Path) -> Result<Vec<OsString>, String> {
    let mut names = Vec::new();
    list_each(
        dir,
        |name, is_link| {
            if is_link {
                return Err("symlink in addon file path".to_string());
            }
            names.push(name);
            Ok(())
        },
        |e| e,
    )?;
    Ok(names)
}

/// A static check for the code that wants to refuse early: no link in `path` below its root. A path under no
/// registered root is refused.
pub(crate) fn links_below_root(path: &Path) -> Result<(), String> {
    let (root, names) = anchor(path, Scope::Mod)?;
    let mut current = root.canonical.clone();
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
