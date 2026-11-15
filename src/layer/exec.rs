#![allow(static_mut_refs)]
use crate::layer::Layer;
use crate::util::{Lock, drop_privileges};
use anyhow::{Context, Result, anyhow};
use libc::syscall;
use log::trace;
use nix::libc::setns;
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::{
    ForkResult, Gid, Pid, Uid, chdir, chroot, execvpe, fork, getgid, getuid,
    setresgid, setresuid,
};
use std::ffi::{CString, c_int};
#[cfg(feature = "coverage")]
unsafe extern "C" {
    fn __llvm_profile_dump();
    fn __llvm_profile_set_filename(filename: *const i8);
}

impl Layer {
    /**
     * Exec within the provided layer. This function never returns on
     * success. `lock` is the layer lock, which keeps PID 1 from shutting the
     * layer down until our command is running in it.
     */
    pub fn exec(&self, command: &[String], lock: Box<Lock>) -> Result<()> {
        let cwd = std::env::current_dir()
            .context(format!("failed to get current directory"))?;

        /* With host networking, PID 1 is in the host's network namespace, so
         * CLONE_NEWNET just joins that */
        let ns_flags = libc::CLONE_NEWNS
            | libc::CLONE_NEWPID
            | libc::CLONE_NEWIPC
            | libc::CLONE_NEWNET
            | libc::CLONE_NEWCGROUP
            | libc::CLONE_NEWUTS;

        #[cfg(feature = "coverage")]
        let ns_flags = if std::env::var_os("TEST_UNABLE_TO_JOIN_LAYER")
            .is_some()
        {
            // in this test case we are failing to enter a layer so our
            // .profraw file will be outside of the layer, so we'll place
            // it directly where we want it for coverage analysis
            unsafe {
                __llvm_profile_set_filename(
                    CString::new(
                        format!("coverage/profraw/join-layer-failure-{}-{}-%m.profraw",
                            self.pid,
                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                                .context("Failed to get current time")?
                                .as_secs())
                            .as_str()
                    )
                        .context("Failed to create CString for profile filename")?
                        .as_ptr(),
                );
            }
            ns_flags | libc::CLONE_FS // invalid flag combination with newns
        } else {
            ns_flags
        };

        /* pidfd-based setns() uses ptrace_may_access() to verify the
         * caller can access the target process. This requires:
         *   1. The caller's real UID/GID matches the target's UIDs/GIDs,
         *      or CAP_SYS_PTRACE is available.
         *   2. The caller is dumpable (dumpable == SUID_DUMP_USER == 1).
         *
         * When running as a setuid binary, real UID != 0, which fails
         * check (1). EUID transitions (e.g., in access checks) can set
         * dumpable to SUID_DUMP_ROOT (2), which fails check (2).
         *
         * Fix: temporarily elevate real UID/GID to root (restoring the
         * original as the saved UID/GID), then reset dumpable to 1 since
         * changing real UID clears it. */
        let saved_ruid = getuid();
        let saved_rgid = getgid();
        setresuid(Uid::from_raw(0), Uid::from_raw(0), saved_ruid)
            .context("failed to set real UID to root for namespace join")?;
        setresgid(Gid::from_raw(0), Gid::from_raw(0), saved_rgid)
            .context("failed to set real GID to root for namespace join")?;
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 1);
        };

        /* Join the layer's cgroup so we're subject to its resource limits.
         * This needs to happen before we enter the layer's mount namespace,
         * where the cgroup filesystem is read-only. */
        self.join_cgroup()?;

        /* Join the layer namespaces */
        let pid_fd = pidfd_open(self.pid, 0)?;
        if (unsafe { setns(pid_fd, ns_flags) }) != 0 {
            return Err(anyhow!(
                "Failed to join layer namespaces: {}",
                nix::errno::Errno::last()
            ));
        }

        // Enter the layer mount namespace
        chdir("/").context(format!("failed to chdir to /"))?; // This will be the layer root
        chroot(".").context(format!("failed to chroot to ."))?; // Now we fully enter the layer namespace

        /* Restore our CWD to be within our layer */
        trace!("Setting CWD to {}", cwd.display());
        std::env::set_current_dir(&cwd)
            .context(format!("failed to set CWD to {}", cwd.display()))?;

        /* Set uid and gid to be what it should be, usually not root */
        trace!("Dropping privileges to uid/gid: {}/{}", self.uid, self.gid);
        drop_privileges(self.uid, self.gid).context(format!(
            "failed to drop privileges to uid/gid: {}/{}",
            self.uid, self.gid
        ))?;

        unsafe {
            std::env::set_var("DEVLAYER", self.name.clone());
            std::env::set_var(
                "DEVLAYER_STORAGE_DIR",
                self.data_storage_dir.clone(),
            );
        }

        /* Stands in for a command that's slow to start, e.g. on a loaded
         * machine */
        #[cfg(feature = "coverage")]
        if let Some(ms) = std::env::var("TEST_DELAY_FORK")
            .ok()
            .and_then(|ms| ms.parse().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }

        /* We need to fork to properly enter the PID namespace */
        let result = unsafe { fork() }.context(format!("failed to fork"))?;
        /* Our command is in the layer's PID namespace now, where PID 1 sees
         * it */
        drop(lock);
        match result {
            ForkResult::Parent { child } => {
                /* Parent process */
                let mut ct = 0;

                loop {
                    // lazy coverage hack to hit the _ condition. This is completely
                    // unnecessary for normal operation, but it's also not harmful.
                    let waitpid_flags = if ct == 0 {
                        ct += 1;
                        Some(nix::sys::wait::WaitPidFlag::WNOHANG)
                    } else {
                        None
                    };

                    /* When our child receives a signal, we bubble that up to ourselves so that
                     * our caller can see the signal as well */
                    match waitpid(child, waitpid_flags) {
                        Ok(WaitStatus::Exited(_, exit_code)) => {
                            std::process::exit(exit_code);
                        }
                        Ok(WaitStatus::Signaled(_, signal, _)) => {
                            #[cfg(feature = "coverage")]
                            unsafe {
                                __llvm_profile_set_filename(
                                    CString::new(
                                        format!("coverage/profraw/signal-{}-{}-%m.profraw",
                                            self.pid,
                                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                                                .context("Failed to get current time")?
                                                .as_secs())
                                            .as_str()
                                    )
                                        .context("Failed to create CString for profile filename")?
                                        .as_ptr(),
                                );
                                __llvm_profile_dump();
                            }

                            let _ = nix::sys::signal::kill(
                                Pid::from_raw(std::process::id() as i32),
                                signal,
                            );
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            }
            ForkResult::Child => {
                /* Convert our command, args, and environment strings into things we need
                 * to pass to execvpe */
                assert!(!command.is_empty(), "Command must not be empty");
                let command_cstr = CString::new(command[0].as_str())?;
                let args_cstr: Vec<CString> = command
                    .iter()
                    .skip(1)
                    .map(|s| CString::new(s.as_str()))
                    .collect::<Result<_, _>>()?;
                let environment = std::env::vars()
                    .map(|(key, value)| {
                        CString::new(format!("{}={}", key, value)).context(
                            "Failed to create CString for environment variable",
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;

                // Create vectors of pointers for args and env
                let args_ptr: Vec<&std::ffi::CStr> =
                    std::iter::once(command_cstr.as_ref())
                        .chain(args_cstr.iter().map(|s| s.as_ref()))
                        .collect();
                let env_ptr: Vec<&std::ffi::CStr> =
                    environment.iter().map(|s| s.as_ref()).collect();

                // Flush gcov coverage data before we exec
                #[cfg(feature = "coverage")]
                unsafe {
                    __llvm_profile_dump();
                }

                // execvpe. This will never return on success
                let Err(e) = execvpe(&command_cstr, &args_ptr, &env_ptr);
                Err(anyhow!(
                    "Failed to execute command {} [cwd={}]: {}",
                    args_ptr
                        .iter()
                        .map(|s| s.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" "),
                    cwd.display(),
                    e
                ))
            }
        }
    }
}

fn pidfd_open(pid: Pid, flags: i32) -> Result<c_int> {
    let raw_pid = pid.as_raw();
    let fd = unsafe { syscall(libc::SYS_pidfd_open, raw_pid, flags) };
    if fd < 0 {
        Err(anyhow!(
            "Failed to open pidfd: {}",
            nix::errno::Errno::last()
        ))
    } else {
        Ok(fd as c_int)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pidfd_open() {
        let pid = Pid::from_raw(0xfffffff);
        assert!(pidfd_open(pid, 0).is_err());
    }
}
