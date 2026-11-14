use nix::unistd::Pid;
use std::path::Path;
use std::path::PathBuf;

use log::trace;

/**
 * Get the PID 1 of a running layer, from its pid file.
 *
 * The pid file is only trusted while it names PID 1 of a pid namespace
 * below ours that started when the pid file says it did. A stale pid file,
 * e.g. after a reboot, may name an unrelated process, which we must not
 * kill everything alongside of or run commands next to.
 *
 * Returns None, removing the pid file, if the layer is not running.
 */
pub fn get_running_layer_pid(
    layers_storage_dir: &Path,
    layer_name: &str,
) -> Option<Pid> {
    let pid_file = get_layer_pid_path(layers_storage_dir, layer_name);
    if !pid_file.exists() {
        return None;
    }

    let pid = std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|contents| {
            let mut fields = contents.split_whitespace();
            let pid = Pid::from_raw(fields.next()?.parse().ok()?);
            let start_time = fields.next()?.parse::<u64>().ok()?;
            let (state, started) = process_state_and_start_time(pid)?;
            (started == start_time
                && state != "Z"
                && state != "T"
                && is_namespace_init(pid))
            .then_some(pid)
        });

    match pid {
        Some(pid) => {
            trace!("Existing layer '{}' found with PID: {}", layer_name, pid);
        }
        None => {
            trace!(
                "Layer '{}' isn't running, removing its pid file",
                layer_name
            );
            std::fs::remove_file(&pid_file).ok();
        }
    }
    pid
}

/// What to write to the pid file of a layer whose PID 1 is `pid`
pub fn layer_pid_file_contents(pid: Pid) -> String {
    let start_time = process_state_and_start_time(pid).map_or(0, |(_, t)| t);
    format!("{} {}", pid, start_time)
}

/// The state (e.g. "R", "Z") and start time, in clock ticks since boot, of
/// process `pid`
fn process_state_and_start_time(pid: Pid) -> Option<(String, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    // The command name is in parentheses and may contain anything
    let fields: Vec<&str> = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    // Fields 3 (state) and 22 (starttime) of proc_pid_stat(5)
    Some((fields.first()?.to_string(), fields.get(19)?.parse().ok()?))
}

/// Whether `pid` is PID 1 of a pid namespace below ours
fn is_namespace_init(pid: Pid) -> bool {
    std::fs::read_to_string(format!("/proc/{}/status", pid)).is_ok_and(
        |status| {
            status.lines().any(|line| {
                let ids: Vec<&str> = line
                    .strip_prefix("NSpid:")
                    .map(|ids| ids.split_whitespace().collect())
                    .unwrap_or_default();
                ids.len() > 1 && ids.last() == Some(&"1")
            })
        },
    )
}

pub fn get_layer_pid_path(
    layers_storage_dir: &Path,
    layer_name: &str,
) -> PathBuf {
    layers_storage_dir.join(format!("{}.pid", layer_name))
}
