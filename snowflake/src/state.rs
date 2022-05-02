//! Working with state directories.

use {
    os_ext::{O_DIRECTORY, O_PATH, mkdirat, open, openat},
    std::{
        io::{self, ErrorKind::AlreadyExists},
        lazy::SyncOnceCell,
        os::unix::io::{AsFd, BorrowedFd, OwnedFd},
        path::Path,
        sync::atomic::{AtomicU32, Ordering::SeqCst},
    },
};

// Paths to the different components of the state directory.
const SCRATCHES_DIR:      &str = "scratches";
const CACHED_ACTIONS_DIR: &str = "cached_actions";
const CACHED_OUTPUTS_DIR: &str = "cached_outputs";

/// Handle to a state directory.
///
/// A state directory, often at the path `.snowflake`,
/// contains on-disk state pertaining to a project.
/// Most state persists across build system invocations.
pub struct State
{
    /// Handle to the state directory.
    state_dir: OwnedFd,

    // Handles to the different components of the state directory.
    // See their eponymous methods to learn about their purposes.
    scratches_dir:      SyncOnceCell<OwnedFd>,
    cached_actions_dir: SyncOnceCell<OwnedFd>,
    cached_outputs_dir: SyncOnceCell<OwnedFd>,

    /// Name of the next scratch directory to create.
    next_scratch_dir: AtomicU32,
}

impl State
{
    /// Open a state directory.
    ///
    /// The state directory must already exist.
    /// Components of the state directory are not opened immediately;
    /// they are opened when they are first used.
    pub fn open<P>(path: P) -> io::Result<Self>
        where P: AsRef<Path>
    {
        let state_dir = open(path, O_DIRECTORY | O_PATH, 0)?;

        let this = Self{
            state_dir,
            scratches_dir:      SyncOnceCell::new(),
            cached_actions_dir: SyncOnceCell::new(),
            cached_outputs_dir: SyncOnceCell::new(),
            next_scratch_dir:   AtomicU32::new(0),
        };

        Ok(this)
    }

    /// Handle to the scratches directory.
    ///
    /// The scratches directory contains scratch directories.
    /// Scratch directories are temporary directories used during builds.
    /// Scratch directories do not survive restarts of the build system.
    fn scratches_dir(&self) -> io::Result<BorrowedFd>
    {
        self.ensure_open_dir_once(&self.scratches_dir, SCRATCHES_DIR)
    }

    /// Create and open a new scratch directory.
    ///
    /// The scratch directory starts out empty.
    pub fn new_scratch_dir(&self) -> io::Result<OwnedFd>
    {
        let scratches_dir = self.scratches_dir()?;
        let path = self.next_scratch_dir.fetch_add(1, SeqCst).to_string();
        mkdirat(Some(scratches_dir), &path, 0o755)?;
        openat(Some(scratches_dir), path, O_DIRECTORY | O_PATH, 0)
    }

    /// Handle to the cached actions directory.
    ///
    /// The cached actions directory maps each action to its outputs.
    fn cached_actions_dir(&self) -> io::Result<BorrowedFd>
    {
        #![allow(unused)]  // TODO: Use this somewhere.
        self.ensure_open_dir_once(&self.cached_actions_dir, CACHED_ACTIONS_DIR)
    }

    /// Handle to the cached outputs directory.
    ///
    /// The cached outputs directory stores each output
    /// using a content-addressable naming scheme.
    fn cached_outputs_dir(&self) -> io::Result<BorrowedFd>
    {
        #![allow(unused)]  // TODO: Use this somewhere.
        self.ensure_open_dir_once(&self.cached_outputs_dir, CACHED_OUTPUTS_DIR)
    }

    /// Ensure that a directory exists and open it.
    fn ensure_open_dir_once<'a>(
        &self,
        cell: &'a SyncOnceCell<OwnedFd>,
        path: &str,
    ) -> io::Result<BorrowedFd<'a>>
    {
        let owned_fd = cell.get_or_try_init(|| {
            let dirfd = Some(self.state_dir.as_fd());
            mkdirat(dirfd, path, 0o755)
                .or_else(ok_if_already_exists)?;
            openat(dirfd, path, O_DIRECTORY | O_PATH, 0)
        })?;
        Ok(owned_fd.as_fd())
    }
}

fn ok_if_already_exists(err: io::Error) -> io::Result<()>
{
    if err.kind() == AlreadyExists {
        Ok(())
    } else {
        Err(err)
    }
}
