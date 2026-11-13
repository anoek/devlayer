//! The supervisor is a small root process on the host that starts a layer's
//! PID 1 and waits for it to exit. When PID 1 exits, for whatever reason, the
//! supervisor cleans up after the layer the way `stop` does: it removes the
//! pid file and the layer's cgroup and unmounts the layer.

use crate::layer::Layer;
use crate::layer::cgroup::remove_cgroup;
use crate::util::{Lock, get_running_layer_pid};
use anyhow::{Context, Result};
use log::trace;
use nix::sys::wait::waitpid;
use nix::unistd::{Gid, Pid, Uid, setresgid, setresuid, setsid};
use std::os::fd::AsRawFd;
use std::path::Path;

/// Detaches the current (freshly forked) process from the caller's session
/// and makes it fully root, so the user can't signal it and it survives the
/// user's terminal going away.
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
/// descriptor, so we don't keep the caller's terminal, pipes or layer lock
/// open while we wait, and don't hold the user's working directory busy.
pub(super) fn detach() {
    if let Ok(null) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
    {
        for fd in 0..=2 {
            unsafe { libc::dup2(null.as_raw_fd(), fd) };
        }
    }
    unsafe { libc::syscall(libc::SYS_close_range, 3, u32::MAX, 0) };
    let _ = std::env::set_current_dir("/");
}

/// Waits for PID 1 to exit, cleans up after the layer, and exits.
/// `own_cgroup` is whether the layer created a cgroup of its own.
pub(super) fn supervise(layer: &Layer, pid1: Pid, own_cgroup: bool) -> ! {
    trace!("Supervising layer '{}' with PID 1 {}", layer.name, pid1);
    while let Err(nix::errno::Errno::EINTR) = waitpid(pid1, None) {}
    let _ = cleanup(layer, own_cgroup);
    std::process::exit(0);
}

fn cleanup(layer: &Layer, own_cgroup: bool) -> Result<()> {
    let storage_dir = layer.base.parent().unwrap_or(Path::new("/"));

    /* Without a lock file the layer was deleted, which already stopped and
     * unmounted it. Opening it without creating it keeps it deleted. */
    let Some(_lock) = Lock::existing_layer(storage_dir, &layer.name)? else {
        return Ok(());
    };

    /* If the pid file names a running PID 1, the layer was restarted, so
     * leave the new instance alone. Otherwise this removes the pid file. */
    if get_running_layer_pid(storage_dir, &layer.name).is_some() {
        return Ok(());
    }

    let cgroup = layer.cgroup_path();
    if own_cgroup && cgroup.exists() {
        remove_cgroup(&cgroup);
    }

    layer.unmount()
}
