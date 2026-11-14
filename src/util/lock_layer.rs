use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use log::trace;
use nix::fcntl::Flock;

use anyhow::{Context, Result, anyhow};

pub struct Lock {
    path: PathBuf,
    #[allow(dead_code)]
    lock: Flock<File>,
}

pub fn get_layer_lock_path(
    layers_storage_dir: &Path,
    layer_name: &str,
) -> PathBuf {
    layers_storage_dir.join(format!("{}.lock", layer_name))
}

impl Lock {
    /** Acquire the lock on a layer, creating its lock file if needed. */
    pub fn layer(
        layers_storage_dir: &Path,
        layer_name: &str,
    ) -> Result<Box<Lock>> {
        let lock_file = get_layer_lock_path(layers_storage_dir, layer_name);
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_file)
            .map_err(|e| {
                anyhow!(
                    "Failed to open lock file for layer {}: {}",
                    layer_name,
                    e
                )
            })?;
        Self::acquire(lock_file, file)
    }

    /// Whether our lock file is still the one at its path
    fn is_current(&self) -> bool {
        let ours = self.lock.metadata();
        let current = std::fs::metadata(&self.path);
        ours.is_ok_and(|ours| {
            current.is_ok_and(|current| {
                ours.dev() == current.dev() && ours.ino() == current.ino()
            })
        })
    }

    fn acquire(lock_file: PathBuf, file: File) -> Result<Box<Lock>> {
        trace!("Acquiring lock {}", lock_file.display());
        let lock =
            nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive)
                .map_err(|(_, e)| anyhow!("Failed to acquire lock: {}", e))?;

        trace!("Acquired lock {}", lock_file.display());
        Ok(Box::new(Lock {
            path: lock_file,
            lock,
        }))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        trace!("Unlocking lock {}", self.path.display());
    }
}

/// A handle on an existing layer lock file that isn't locked yet. Forked
/// processes share the handle, and with it the lock: once one of them takes
/// it, it's held until one of them unlocks it, or all of them have closed
/// the handle.
pub struct SharedLock {
    path: PathBuf,
    file: File,
}

impl SharedLock {
    pub fn open(
        layers_storage_dir: &Path,
        layer_name: &str,
    ) -> Result<SharedLock> {
        let path = get_layer_lock_path(layers_storage_dir, layer_name);
        let file = File::open(&path).context("Failed to open layer lock")?;
        Ok(SharedLock { path, file })
    }

    /// Takes the lock unless someone else holds it, returning whether we did
    pub fn try_lock(&self) -> bool {
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB)
                == 0
        }
    }

    pub fn unlock(&self) {
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }

    /// Waits for the lock, unless we hold it already. Returns None when the
    /// lock file has been deleted or replaced, i.e. the layer was deleted.
    pub fn lock(self) -> Result<Option<Box<Lock>>> {
        let lock = Lock::acquire(self.path, self.file)?;
        Ok(Some(lock).filter(|lock| lock.is_current()))
    }
}

impl AsRawFd for SharedLock {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}
