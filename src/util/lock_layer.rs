use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use log::trace;
use nix::fcntl::Flock;

use anyhow::{Result, anyhow};

pub struct Lock {
    path: PathBuf,
    #[allow(dead_code)]
    lock: Flock<File>,
}

impl Lock {
    /** Acquire a lock on the layer storage directory. */
    pub fn layer(
        layers_storage_dir: &Path,
        layer_name: &str,
    ) -> Result<Box<Lock>> {
        let lock_file = layers_storage_dir.join(format!("{}.lock", layer_name));
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

    /** Like `layer`, but returns None instead of creating the lock file
     * when it can't be opened, e.g. because the layer was deleted. */
    pub fn existing_layer(
        layers_storage_dir: &Path,
        layer_name: &str,
    ) -> Result<Option<Box<Lock>>> {
        let lock_file = layers_storage_dir.join(format!("{}.lock", layer_name));
        match OpenOptions::new().read(true).write(true).open(&lock_file) {
            Ok(file) => Self::acquire(lock_file, file).map(Some),
            Err(_) => Ok(None),
        }
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
