//! The supervisor is a small root process on the host that starts a layer's
//! PID 1 and waits for it to exit. When PID 1 exits, for whatever reason, the
//! supervisor cleans up after the layer the way `stop` does: it removes the
//! pid file and the layer's cgroup and unmounts the layer.
//!
//! The supervisor and PID 1 share a handle on the layer lock. PID 1 only
//! shuts the layer down while holding the lock, so it can't shut down while
//! a command is starting in the layer, and it exits still holding it. The
//! supervisor keeps holding it until it has cleaned up, so nothing can join
//! or restart the layer in the meantime.

use crate::layer::Layer;
use crate::layer::cgroup::remove_cgroup;
use crate::util::{SharedLock, get_running_layer_pid_or_remove};
use anyhow::{Context, Result};
use log::trace;
use nix::sys::wait::waitpid;
use nix::unistd::{Gid, Pid, Uid, setresgid, setresuid, setsid};
use std::os::fd::{AsRawFd, RawFd};

/// Detaches the current (freshly forked) process from the caller's session
/// and makes it fully root, so the user can't signal it and it survives the
/// user's terminal going away. PID 1, which we start, inherits being fully
/// root, which joining the layer with pidfd setns() requires.
pub(super) fn become_supervisor() -> Result<()> {
    setsid().context("failed to create a session for the layer supervisor")?;
    setresuid(Uid::from_raw(0), Uid::from_raw(0), Uid::from_raw(0))
        .context("failed to set layer supervisor UIDs to root")?;
    setresgid(Gid::from_raw(0), Gid::from_raw(0), Gid::from_raw(0))
        .context("failed to set layer supervisor GIDs to root")?;
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }
    Ok(())
}

/// Points stdin, stdout and stderr at /dev/null and closes every other file
/// descriptor but our handle on the layer lock, so we don't keep the
/// caller's terminal, pipes or lock on the layer open while we wait, and
/// don't hold the user's working directory busy.
pub(super) fn detach(lock: &SharedLock) {
    if let Ok(null) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
    {
        for fd in 0..=2 {
            unsafe { libc::dup2(null.as_raw_fd(), fd) };
        }
    }
    close_fds_except(&[lock.as_raw_fd()]);
    let _ = std::env::set_current_dir("/");
}

/// Closes every file descriptor from 3 up, except those in `keep`
pub(super) fn close_fds_except(keep: &[RawFd]) {
    let mut keep: Vec<u32> = keep.iter().map(|&fd| fd as u32).collect();
    keep.sort_unstable();
    let mut first = 3;
    for fd in keep {
        if fd > first {
            unsafe { libc::syscall(libc::SYS_close_range, first, fd - 1, 0) };
        }
        first = first.max(fd + 1);
    }
    unsafe { libc::syscall(libc::SYS_close_range, first, u32::MAX, 0) };
}

/// Waits for PID 1 to exit, cleans up after the layer, and exits.
/// `own_cgroup` is whether the layer created a cgroup of its own, and `lock`
/// is the handle on the layer lock we share with PID 1.
pub(super) fn supervise(
    layer: &Layer,
    pid1: Pid,
    own_cgroup: bool,
    lock: SharedLock,
) -> ! {
    trace!("Supervising layer '{}' with PID 1 {}", layer.name, pid1);
    while let Err(nix::errno::Errno::EINTR) = waitpid(pid1, None) {}
    let _ = cleanup(layer, own_cgroup, lock);
    std::process::exit(0);
}

fn cleanup(layer: &Layer, own_cgroup: bool, lock: SharedLock) -> Result<()> {
    /* When PID 1 shut the layer down, it left us holding the lock already.
     * Without a lock file the layer was deleted, which already stopped and
     * unmounted it. */
    let Some(lock) = lock.lock()? else {
        return Ok(());
    };

    /* If the pid file names a running PID 1, the layer was restarted, so
     * leave the new instance alone. Otherwise this removes the pid file. */
    if get_running_layer_pid_or_remove(&lock, &layer.storage_dir, &layer.name)
        .is_some()
    {
        return Ok(());
    }

    let cgroup = layer.cgroup_path();
    if own_cgroup && cgroup.exists() {
        remove_cgroup(&cgroup);
    }

    layer.unmount()
}
