//! Directories held open, and everything done relative to them.
//!
//! A transition used to resolve a root-relative path, check what it found,
//! and then act on the path again — and the kernel looked every name up a
//! second time. A directory replaced by a symbolic link between the two
//! lookups redirected the act out of the root: under `--allow-root`, onto
//! anything the daemon can reach. Here a path is walked once, one real
//! directory at a time, and what follows is done relative to the directory
//! the walk ended in, through its descriptor, so no later lookup passes
//! through a name above it again.
//!
//! Nothing here follows a symbolic link. Every open is `O_NOFOLLOW`, every
//! stat and chown `AT_SYMLINK_NOFOLLOW`, and a mode is changed through a
//! descriptor rather than by name, since `chmod` by name follows a link.
//!
//! What this cannot close is the window between checking an entry and
//! acting on it: a save landing there is still overwritten. That is the
//! remaining half of accepted risks §2.

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

/// How a directory is held. On Linux `O_PATH`: the descriptor serves only
/// as a base for `*at` calls, so it needs no read permission, as looking a
/// name up never did. Elsewhere there is no such flag, and a directory is
/// opened for reading — which every directory synchronization descends
/// into already allows, since it had to be listed to be scanned.
#[cfg(target_os = "linux")]
const DIRECTORY: OFlags = OFlags::PATH
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
#[cfg(not(target_os = "linux"))]
const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// A mode as this platform's `mode_t`: 32 bits on Linux, 16 on macOS,
/// where no permission bit lies above them.
#[allow(clippy::unnecessary_cast)]
fn raw_mode(mode: u32) -> Mode {
    Mode::from_raw_mode(mode as rustix::fs::RawMode)
}

/// A directory held open by descriptor. The path it was reached by is kept
/// only to describe it: nothing is ever looked up through it.
#[derive(Debug)]
pub(crate) struct Dir {
    fd: OwnedFd,
    path: PathBuf,
}

impl AsFd for Dir {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Dir {
    /// Opens `path` as a directory, refusing a symbolic link as its final
    /// component. Used for a root; everything beneath one is reached with
    /// [`Dir::open_dir`].
    pub(crate) fn open(path: &Path) -> io::Result<Dir> {
        let fd = rustix::fs::open(path, DIRECTORY, Mode::empty())?;
        Ok(Dir {
            fd,
            path: path.to_path_buf(),
        })
    }

    /// Holds an already-open directory, reached by `path`. The caller has
    /// checked that the handle is the directory it means.
    pub(crate) fn from_handle(handle: impl Into<OwnedFd>, path: PathBuf) -> Dir {
        Dir {
            fd: handle.into(),
            path,
        }
    }

    /// The path this directory was reached by, for messages.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The path of an entry in this directory, for messages.
    pub(crate) fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.path.join(name)
    }

    /// Opens the directory `name` within this one, refusing a symbolic
    /// link: `ENOTDIR` or `ELOOP` rather than a redirection.
    pub(crate) fn open_dir(&self, name: impl AsRef<Path>) -> io::Result<Dir> {
        let name = name.as_ref();
        let fd = rustix::fs::openat(&self.fd, name, DIRECTORY, Mode::empty())?;
        Ok(Dir {
            fd,
            path: self.path.join(name),
        })
    }

    /// Describes the entry `name` itself: a symbolic link is described,
    /// not followed.
    pub(crate) fn stat(&self, name: impl AsRef<Path>) -> io::Result<Stat> {
        let raw = rustix::fs::statat(&self.fd, name.as_ref(), AtFlags::SYMLINK_NOFOLLOW)?;
        Ok(Stat::from_raw(&raw))
    }

    /// Describes this directory itself.
    pub(crate) fn stat_self(&self) -> io::Result<Stat> {
        Ok(Stat::from_raw(&rustix::fs::fstat(&self.fd)?))
    }

    /// Opens the file `name` with `flags`, never through a symbolic link.
    /// `mode` applies only when `flags` creates it.
    pub(crate) fn open_file(
        &self,
        name: impl AsRef<Path>,
        flags: OFlags,
        mode: u32,
    ) -> io::Result<File> {
        let fd = rustix::fs::openat(
            &self.fd,
            name.as_ref(),
            flags | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            raw_mode(mode),
        )?;
        Ok(File::from(fd))
    }

    /// Creates a new private file `name` for reading and writing: `0600`,
    /// and never through anything already at that name, a planted symbolic
    /// link included. The descriptor-relative [`crate::fsutil::private_file`].
    pub(crate) fn create_private_file(&self, name: impl AsRef<Path>) -> io::Result<File> {
        self.open_file(name, OFlags::RDWR | OFlags::CREATE | OFlags::EXCL, 0o600)
    }

    /// Creates the directory `name`, its mode narrowed by the umask as
    /// `mkdir` narrows it; [`Dir::set_mode`] gives it the exact one.
    pub(crate) fn create_dir(&self, name: impl AsRef<Path>, mode: u32) -> io::Result<()> {
        rustix::fs::mkdirat(&self.fd, name.as_ref(), raw_mode(mode))?;
        Ok(())
    }

    /// Creates a symbolic link `name` pointing at `target`.
    pub(crate) fn symlink(&self, target: &Path, name: impl AsRef<Path>) -> io::Result<()> {
        rustix::fs::symlinkat(target, &self.fd, name.as_ref())?;
        Ok(())
    }

    /// Reads the target of the symbolic link `name`.
    pub(crate) fn read_link(&self, name: impl AsRef<Path>) -> io::Result<PathBuf> {
        let target = rustix::fs::readlinkat(&self.fd, name.as_ref(), Vec::new())?;
        Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
    }

    /// Removes the non-directory `name`. A symbolic link is removed itself.
    pub(crate) fn remove_file(&self, name: impl AsRef<Path>) -> io::Result<()> {
        rustix::fs::unlinkat(&self.fd, name.as_ref(), AtFlags::empty())?;
        Ok(())
    }

    /// Removes the empty directory `name`.
    pub(crate) fn remove_dir(&self, name: impl AsRef<Path>) -> io::Result<()> {
        rustix::fs::unlinkat(&self.fd, name.as_ref(), AtFlags::REMOVEDIR)?;
        Ok(())
    }

    /// Removes `name` and, if it is a directory, everything beneath it —
    /// descending only real directories, so a symbolic link anywhere inside
    /// is removed as a link and what it points at is left alone.
    pub(crate) fn remove_tree(&self, name: impl AsRef<Path>) -> io::Result<()> {
        let name = name.as_ref();
        if !self.stat(name)?.file_type().is_dir() {
            return self.remove_file(name);
        }
        let inner = self.open_dir(name)?;
        for entry in inner.entries()? {
            inner.remove_tree(entry?.name)?;
        }
        drop(inner);
        self.remove_dir(name)
    }

    /// Changes the mode of the entry `name` without following a symbolic
    /// link at that name: the change is made through a descriptor for the
    /// entry itself, which a link swapped in afterwards cannot redirect.
    /// A symbolic link is refused with `ELOOP`.
    pub(crate) fn set_mode(&self, name: impl AsRef<Path>, mode: u32) -> io::Result<()> {
        let name = name.as_ref();
        #[cfg(target_os = "linux")]
        {
            // `fchmod` refuses an `O_PATH` descriptor and `fchmodat` cannot
            // decline to follow a link, so the change goes through the
            // descriptor's `/proc` entry: a magic link to this very inode,
            // whatever has happened to the name since. glibc does the same.
            let entry = rustix::fs::openat(
                &self.fd,
                name,
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?;
            if FileType::from_raw_mode(rustix::fs::fstat(&entry)?.st_mode as _) == FileType::Symlink
            {
                return Err(io::Error::from_raw_os_error(libc::ELOOP));
            }
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                format!("/proc/self/fd/{}", entry.as_raw_fd()),
                std::fs::Permissions::from_mode(mode),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            rustix::fs::chmodat(&self.fd, name, raw_mode(mode), AtFlags::SYMLINK_NOFOLLOW)?;
            Ok(())
        }
    }

    /// Changes the owner and group of the entry `name`, a symbolic link
    /// itself rather than what it points at. `None` leaves either alone.
    pub(crate) fn chown(
        &self,
        name: impl AsRef<Path>,
        owner: Option<u32>,
        group: Option<u32>,
    ) -> io::Result<()> {
        rustix::fs::chownat(
            &self.fd,
            name.as_ref(),
            owner.map(rustix::fs::Uid::from_raw),
            group.map(rustix::fs::Gid::from_raw),
            AtFlags::SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }

    /// Lists this directory, `.` and `..` aside. An entry that could not be
    /// read is an error in its place, as `read_dir` yields one.
    pub(crate) fn entries(&self) -> io::Result<Vec<io::Result<Entry>>> {
        // Listed through a descriptor of its own, opened for reading: the
        // held one may be `O_PATH`, which cannot be read.
        let listing = rustix::fs::openat(
            &self.fd,
            ".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let mut reader = rustix::fs::Dir::read_from(&listing)?;
        let mut entries = Vec::new();
        while let Some(entry) = reader.read() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    entries.push(Err(error.into()));
                    break;
                }
            };
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let name = OsString::from_vec(bytes.to_vec());
            // A filesystem that does not say an entry's type in the
            // listing is asked, as `DirEntry::file_type` asks.
            let kind = match entry.file_type() {
                FileType::Unknown => self.stat(&name).map(|stat| stat.file_type()),
                kind => Ok(Kind(kind)),
            };
            entries.push(Ok(Entry { name, kind }));
        }
        Ok(entries)
    }
}

/// An entry of a listed directory.
pub(crate) struct Entry {
    /// Its name, as the directory holds it.
    pub(crate) name: OsString,
    /// What it is, without following a symbolic link.
    pub(crate) kind: io::Result<Kind>,
}

impl Entry {
    /// The name, if it is valid UTF-8.
    pub(crate) fn name_str(&self) -> Option<&str> {
        self.name.to_str()
    }
}

/// Renames `from`, resolved relative to `from_dir`, to `name` in `to`. A
/// replacement uses the plain rename. A *creation* refuses to replace
/// anything: it carries no expectation about existing content, so whatever
/// appeared at the name since the caller looked belongs to someone else.
/// Linux enforces that atomically with `RENAME_NOREPLACE`, macOS with
/// `RENAME_EXCL`; a filesystem without the flag reports it unsupported, and
/// there the plain rename keeps the check-then-rename window rather than
/// failing every creation — the residual accepted risks §2 describes.
pub(crate) fn rename(
    from_dir: impl AsFd,
    from: &Path,
    to: &Dir,
    name: impl AsRef<Path>,
    replace: bool,
) -> io::Result<()> {
    let name = name.as_ref();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if !replace {
        use rustix::io::Errno;
        match rustix::fs::renameat_with(
            from_dir.as_fd(),
            from,
            to,
            name,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => return Ok(()),
            // Linux: a filesystem without the flag says EINVAL, a kernel
            // without the call ENOSYS, some stacks EOPNOTSUPP. macOS:
            // ENOTSUP or EINVAL.
            Err(error)
                if error == Errno::INVAL
                    || error == Errno::NOSYS
                    || error == Errno::OPNOTSUPP
                    || error == Errno::NOTSUP => {}
            Err(error) => return Err(error.into()),
        }
    }
    let _ = replace;
    rustix::fs::renameat(from_dir, from, to, name)?;
    Ok(())
}

/// Exchanges `from` in `from_dir` with `name` in `to`, atomically: each
/// name ends up holding the other's file, and neither is ever missing.
///
/// `Ok(false)` where there is no exchange to make — a kernel before 3.15,
/// a filesystem that refuses the flag, a platform other than Linux and
/// macOS — so that the caller can fall back to a plain rename.
pub(crate) fn exchange(from_dir: &Dir, from: &str, to: &Dir, name: &str) -> io::Result<bool> {
    #[cfg(test)]
    if EXCHANGE_WITHHELD.get() {
        return Ok(false);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use rustix::io::Errno;
        match rustix::fs::renameat_with(from_dir, from, to, name, rustix::fs::RenameFlags::EXCHANGE)
        {
            Ok(()) => Ok(true),
            Err(error)
                if error == Errno::INVAL
                    || error == Errno::NOSYS
                    || error == Errno::OPNOTSUPP
                    || error == Errno::NOTSUP =>
            {
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (from_dir, from, to, name);
        Ok(false)
    }
}

#[cfg(test)]
thread_local! {
    /// A test seam withholding [`exchange`], as a filesystem without it
    /// does. Per thread, so parallel tests cannot see each other's.
    pub(crate) static EXCHANGE_WITHHELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether something has `file` open for writing, where Linux can say: a
/// read lease is refused while the file is open for writing anywhere, so
/// taking one and handing it straight back answers without waiting on
/// anyone. `file` must be open read-only.
///
/// Readers do not count, and must not. A reader loses nothing when the
/// file is replaced, and readers are everywhere: a scan or a delta base in
/// this process, a viewer, an index. So is a copy no one is using: a child
/// process holds every descriptor its parent had from fork until exec, so
/// a file this process closed a moment ago can still be open in one. A
/// write lease, refused for any open at all, deferred replacements over
/// both. A copy of a write descriptor lingers the same way: this process's
/// own, on the temporary that becomes the target, or a program's on a save
/// it has just closed. Each is closed before the file is next probed, a
/// cycle later rather than an instant, so at worst a replacement waits one
/// cycle.
///
/// `None` where the question cannot be asked: not Linux, a filesystem
/// without leases, or a file this process neither owns nor may lease.
pub(crate) fn written_elsewhere(file: &File) -> Option<bool> {
    /// `F_SETSIG`, which the libc crate does not export: 10 in Linux's
    /// generic `fcntl.h`, which x86-64 and AArch64 use.
    #[cfg(target_os = "linux")]
    const F_SETSIG: libc::c_int = 10;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        // rustix has no lease calls, so these go through libc.
        //
        // An open for writing while the lease is held breaks it, and the
        // kernel tells the holder with a signal: SIGIO unless told
        // otherwise, whose default is to terminate the process. It is
        // pointed at SIGURG instead, whose default is to be ignored, so a
        // break in the instant the lease is held costs nothing.
        //
        // SAFETY: each call is fcntl on a descriptor `file` keeps open for
        // the duration, with integer arguments.
        unsafe {
            if libc::fcntl(fd, F_SETSIG, libc::SIGURG) != 0 {
                return None;
            }
            if libc::fcntl(fd, libc::F_SETLEASE, libc::F_RDLCK) == 0 {
                libc::fcntl(fd, libc::F_SETLEASE, libc::F_UNLCK);
                return Some(false);
            }
        }
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::EAGAIN) => Some(true),
            _ => None,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
        None
    }
}

/// What a directory entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kind(FileType);

impl Kind {
    pub(crate) fn is_file(self) -> bool {
        self.0 == FileType::RegularFile
    }
    pub(crate) fn is_dir(self) -> bool {
        self.0 == FileType::Directory
    }
    pub(crate) fn is_symlink(self) -> bool {
        self.0 == FileType::Symlink
    }
}

/// What a stat reports about one entry: the facts a transition checks
/// before acting, named as `std::os::unix::fs::MetadataExt` names them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stat {
    kind: Kind,
    mode: u32,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ino: u64,
    dev: u64,
    nlink: u64,
}

impl Stat {
    // The field types differ by platform and backend; each is widened to
    // what `MetadataExt` reports, so the two describe a file identically.
    #[allow(clippy::unnecessary_cast)]
    fn from_raw(raw: &rustix::fs::Stat) -> Stat {
        Stat {
            kind: Kind(FileType::from_raw_mode(raw.st_mode as _)),
            mode: raw.st_mode as u32,
            size: raw.st_size as u64,
            mtime: raw.st_mtime as i64,
            mtime_nsec: raw.st_mtime_nsec as i64,
            ino: raw.st_ino as u64,
            dev: raw.st_dev as u64,
            nlink: raw.st_nlink as u64,
        }
    }

    /// Describes an open file.
    pub(crate) fn of(file: &impl AsFd) -> io::Result<Stat> {
        Ok(Stat::from_raw(&rustix::fs::fstat(file)?))
    }

    pub(crate) fn file_type(&self) -> Kind {
        self.kind
    }
    pub(crate) fn mode(&self) -> u32 {
        self.mode
    }
    pub(crate) fn size(&self) -> u64 {
        self.size
    }
    pub(crate) fn mtime(&self) -> i64 {
        self.mtime
    }
    pub(crate) fn mtime_nsec(&self) -> i64 {
        self.mtime_nsec
    }
    pub(crate) fn ino(&self) -> u64 {
        self.ino
    }
    pub(crate) fn dev(&self) -> u64 {
        self.dev
    }
    pub(crate) fn nlink(&self) -> u64 {
        self.nlink
    }

    /// How long ago the entry was last modified; zero for a time in the
    /// future, as a clock step can produce.
    pub(crate) fn age(&self) -> std::time::Duration {
        let modified = std::time::UNIX_EPOCH
            + std::time::Duration::new(self.mtime.max(0) as u64, self.mtime_nsec.max(0) as u32);
        modified.elapsed().unwrap_or_default()
    }
}

impl From<&std::fs::Metadata> for Stat {
    fn from(metadata: &std::fs::Metadata) -> Stat {
        use std::os::unix::fs::MetadataExt;
        let kind = metadata.file_type();
        let kind = if kind.is_file() {
            FileType::RegularFile
        } else if kind.is_dir() {
            FileType::Directory
        } else if kind.is_symlink() {
            FileType::Symlink
        } else {
            FileType::from_raw_mode(metadata.mode() as _)
        };
        Stat {
            kind: Kind(kind),
            mode: metadata.mode(),
            size: metadata.size(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ino: metadata.ino(),
            dev: metadata.dev(),
            nlink: metadata.nlink(),
        }
    }
}

/// Why a walk stopped at a component.
pub(crate) enum Stop {
    /// The component is not a real directory, or could not be opened as
    /// one: the path it was reached by, and the error.
    Refused(PathBuf, io::Error),
    /// A missing directory could not be made, or given its mode.
    Create(anyhow::Error),
}

/// Walks from `root` through `components`, each a real directory, and
/// returns the last one held open. A symbolic link anywhere along the way
/// stops the walk; it is never followed.
///
/// With `create`, a missing directory is made on the way, with that mode
/// exactly, rather than refused.
///
/// On Linux a walk that creates nothing is one `openat2` with
/// `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`: the kernel resolves the whole
/// path and refuses a symbolic link, `..` or a magic link on the way. A
/// kernel without it (before 5.6), or a sandbox that blocks it, falls back
/// to the walk below, which is as safe and slower: one `openat` per
/// component with `O_NOFOLLOW`, two descriptors open at a time.
pub(crate) fn walk(root: &Path, components: &[&str], create: Option<u32>) -> Result<Dir, Stop> {
    let root_dir = Dir::open(root).map_err(|error| Stop::Refused(root.to_path_buf(), error))?;
    if components.is_empty() {
        return Ok(root_dir);
    }
    #[cfg(target_os = "linux")]
    if create.is_none() {
        if let Some(found) = openat2::beneath(&root_dir, components) {
            return Ok(found);
        }
    }
    let mut current = root_dir;
    for component in components {
        if let Some(mode) = create {
            make_directory(&current, component, mode).map_err(Stop::Create)?;
        }
        current = current
            .open_dir(component)
            .map_err(|error| Stop::Refused(current.join(component), error))?;
    }
    Ok(current)
}

/// Makes the directory `name` with exactly `mode`, unless one is already
/// there. What is there is checked by the walk's next step, which refuses
/// anything that is not a real directory.
fn make_directory(dir: &Dir, name: &str, mode: u32) -> anyhow::Result<()> {
    use anyhow::Context;
    match dir.create_dir(name, mode) {
        Ok(()) => dir
            .set_mode(name, mode)
            .with_context(|| format!("unable to set permissions on {}", dir.join(name).display())),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("unable to create {}", dir.join(name).display()))
        }
    }
}

#[cfg(target_os = "linux")]
mod openat2 {
    //! The single-call walk, where the kernel offers it.

    use std::sync::atomic::{AtomicBool, Ordering};

    use rustix::fs::{Mode, ResolveFlags};
    use rustix::io::Errno;

    use super::{Dir, DIRECTORY};

    /// Set once the kernel has said it has no `openat2`, so every later
    /// walk goes straight to the per-component one.
    static UNAVAILABLE: AtomicBool = AtomicBool::new(false);

    /// Whether the kernel has said it has no `openat2`.
    #[cfg(test)]
    pub(crate) fn unavailable() -> bool {
        UNAVAILABLE.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    thread_local! {
        /// Test seams, per thread so that parallel tests cannot see each
        /// other's: whether `openat2` is withheld, as an old kernel would,
        /// and how many walks it has completed.
        pub(crate) static WITHHELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        pub(crate) static COMPLETED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// Opens `components` beneath `root` in one call, or `None` when the
    /// per-component walk should run instead: the kernel lacks the call, or
    /// the call refused, and the walk will say precisely which component
    /// it refused and why.
    pub(super) fn beneath(root: &Dir, components: &[&str]) -> Option<Dir> {
        #[cfg(test)]
        if WITHHELD.get() {
            return None;
        }
        if UNAVAILABLE.load(Ordering::Relaxed) {
            return None;
        }
        let relative = components.join("/");
        match rustix::fs::openat2(
            root,
            relative.as_str(),
            DIRECTORY,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        ) {
            Ok(fd) => {
                #[cfg(test)]
                COMPLETED.set(COMPLETED.get() + 1);
                Some(Dir {
                    fd,
                    path: root.path.join(&relative),
                })
            }
            // No such call (before 5.6), or a sandbox that answers for one
            // it blocks; flags or a structure it does not know.
            Err(error)
                if error == Errno::NOSYS
                    || error == Errno::PERM
                    || error == Errno::INVAL
                    || error == Errno::TOOBIG =>
            {
                UNAVAILABLE.store(true, Ordering::Relaxed);
                None
            }
            // A refusal, or EAGAIN from a rename racing the resolution: the
            // walk below answers either, naming the component.
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

    #[cfg(target_os = "linux")]
    use super::openat2::{unavailable, COMPLETED, WITHHELD};

    /// `root/a/b` and `root/file` are real; `root/link` is a symbolic link
    /// to `outside`, which holds a directory `b` of its own.
    struct Tree {
        _keep: tempfile::TempDir,
        root: PathBuf,
        outside: PathBuf,
    }

    fn tree() -> Tree {
        let keep = tempfile::tempdir().expect("a temporary directory");
        let root = keep.path().join("root");
        let outside = keep.path().join("outside");
        std::fs::create_dir_all(root.join("a/b")).expect("a/b");
        std::fs::create_dir_all(outside.join("b")).expect("outside/b");
        symlink(&outside, root.join("link")).expect("the link");
        std::fs::write(root.join("file"), b"file").expect("the file");
        Tree {
            _keep: keep,
            root,
            outside,
        }
    }

    fn mode(path: &Path) -> u32 {
        std::fs::symlink_metadata(path)
            .expect("inspectable")
            .permissions()
            .mode()
            & 0o777
    }

    /// Runs `check` through every walk this platform has: on Linux once
    /// through `openat2` and once through the per-component walk, with
    /// `openat2` withheld as a kernel before 5.6 withholds it.
    fn through_each_walk(check: impl Fn()) {
        check();
        #[cfg(target_os = "linux")]
        {
            WITHHELD.set(true);
            check();
            WITHHELD.set(false);
        }
    }

    #[test]
    fn a_walk_descends_real_directories() {
        let tree = tree();
        through_each_walk(|| {
            let found = walk(&tree.root, &["a", "b"], None)
                .ok()
                .expect("a/b is real all the way down");
            assert_eq!(found.path(), tree.root.join("a/b"));
            assert_eq!(
                found.stat_self().expect("stat").ino(),
                std::fs::metadata(tree.root.join("a/b")).expect("a/b").ino()
            );
        });
    }

    /// The walk's own refusal, asserted against this code path: a symbolic
    /// link, a file and a missing name each stop it at that component, and
    /// a root that is itself a link stops it at the root.
    #[test]
    fn a_walk_refuses_a_symbolic_link_anywhere_along_the_way() {
        let tree = tree();
        through_each_walk(|| {
            for components in [&["link"][..], &["link", "b"], &["file"], &["missing", "b"]] {
                match walk(&tree.root, components, None) {
                    Err(Stop::Refused(at, _)) => {
                        assert_eq!(at, tree.root.join(components[0]), "{components:?}")
                    }
                    Err(Stop::Create(error)) => panic!("{components:?}: {error:#}"),
                    Ok(found) => panic!("{components:?} walked to {}", found.path().display()),
                }
            }
            match walk(&tree.root.join("link"), &["b"], None) {
                Err(Stop::Refused(at, _)) => assert_eq!(at, tree.root.join("link")),
                _ => panic!("a root that is a link must be refused"),
            }
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn openat2_walks_where_the_kernel_has_it_and_the_walk_stands_in_where_it_does_not() {
        let tree = tree();
        let before = COMPLETED.get();
        walk(&tree.root, &["a", "b"], None).ok().expect("walks");
        // Every kernel since 5.6 has it; one without it said so once, and
        // every walk since has gone the long way.
        assert!(
            COMPLETED.get() == before + 1 || unavailable(),
            "openat2 was available and unused"
        );

        let before = COMPLETED.get();
        WITHHELD.set(true);
        let found = walk(&tree.root, &["a", "b"], None)
            .ok()
            .expect("walks without it");
        WITHHELD.set(false);
        assert_eq!(COMPLETED.get(), before, "a withheld openat2 was used");
        assert_eq!(found.path(), tree.root.join("a/b"));

        // A walk that creates goes a component at a time, as it must.
        let before = COMPLETED.get();
        walk(&tree.root, &["a", "made"], Some(0o700))
            .ok()
            .expect("creates");
        assert_eq!(COMPLETED.get(), before);
    }

    #[test]
    fn a_walk_that_creates_makes_each_directory_with_its_mode_and_never_through_a_link() {
        let tree = tree();
        let found = walk(&tree.root, &["new", "deeper"], Some(0o750))
            .ok()
            .expect("creates");
        assert_eq!(found.path(), tree.root.join("new/deeper"));
        for directory in ["new", "new/deeper"] {
            assert_eq!(mode(&tree.root.join(directory)), 0o750, "{directory}");
        }
        assert!(matches!(
            walk(&tree.root, &["link", "made"], Some(0o750)),
            Err(Stop::Refused(..))
        ));
        assert!(!tree.outside.join("made").exists());
    }

    /// The mechanism the path race is closed by, held still rather than
    /// raced: a directory replaced by a symbolic link after it was walked
    /// to is not where anything done through the held descriptor goes.
    /// With a path looked up again, all three would have landed outside.
    #[test]
    fn a_held_directory_is_not_redirected_by_a_link_swapped_in_for_it() {
        let tree = tree();
        let held = walk(&tree.root, &["a"], None).ok().expect("walks");
        std::fs::rename(tree.root.join("a"), tree.root.join("a.moved")).expect("moved aside");
        symlink(&tree.outside, tree.root.join("a")).expect("a link swapped in");

        held.create_private_file("written").expect("creates a file");
        held.create_dir("made", 0o700).expect("makes a directory");
        held.remove_dir("b").expect("removes a directory");

        assert!(tree.root.join("a.moved/written").is_file());
        assert!(tree.root.join("a.moved/made").is_dir());
        assert!(!tree.root.join("a.moved/b").exists());
        assert!(!tree.outside.join("written").exists());
        assert!(!tree.outside.join("made").exists());
        assert!(
            tree.outside.join("b").is_dir(),
            "the link's target lost a directory"
        );
    }

    #[test]
    fn set_mode_never_changes_what_a_symbolic_link_points_at() {
        let tree = tree();
        let root = Dir::open(&tree.root).expect("the root");
        let outside = mode(&tree.outside);
        let changed = root.set_mode("link", 0o700);
        // Linux cannot change a link's own mode and refuses; macOS changes
        // the link itself. Either way its target keeps its mode.
        #[cfg(target_os = "linux")]
        assert_eq!(
            changed.expect_err("a link is refused").raw_os_error(),
            Some(libc::ELOOP)
        );
        let _ = changed;
        assert_eq!(mode(&tree.outside), outside);

        root.set_mode("file", 0o640).expect("a file");
        assert_eq!(mode(&tree.root.join("file")), 0o640);
        root.set_mode("a", 0o711).expect("a directory");
        assert_eq!(mode(&tree.root.join("a")), 0o711);
    }

    #[test]
    fn remove_tree_removes_a_symbolic_link_and_not_what_it_points_at() {
        let tree = tree();
        std::fs::create_dir_all(tree.root.join("doomed/inner")).expect("doomed");
        symlink(&tree.outside, tree.root.join("doomed/inner/escape")).expect("a link");
        Dir::open(&tree.root)
            .expect("the root")
            .remove_tree("doomed")
            .expect("removes");
        assert!(!tree.root.join("doomed").exists());
        assert!(
            tree.outside.join("b").is_dir(),
            "the link's target lost a directory"
        );
    }
}
