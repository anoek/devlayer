use nix::{sys::signal::kill, unistd::Pid};
use std::path::Path;
use std::path::PathBuf;

use log::trace;

/**
 * Get the PID of a running layer.
 *
 * Returns None if the layer is not running.
 */
pub fn get_running_layer_pid(
    layers_storage_dir: &Path,
    layer_name: &str,
) -> Option<Pid> {
    let layers_storage_dir = &PathBuf::from(layers_storage_dir);
    let pid_file = get_layer_pid_path(layers_storage_dir, layer_name);
    if pid_file.exists() {
        match std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|pid_str| pid_str.parse::<i32>().ok())
            .map(Pid::from_raw)
            .filter(|&pid| kill(pid, None).is_ok())
            .and_then(|pid| {
                trace!(
                    "Existing layer '{}' found with PID: {}",
                    layer_name, pid
                );

                std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw()))
                    .ok()
                    .map(|status| (pid, status))
            })
            .and_then(|(pid, status)| {
                let components: Vec<&str> = status.split_whitespace().collect();
                components
                    .get(2)
                    .filter(|&status| *status != "Z" && *status != "T")
                    .map(|_| pid)
            }) {
            Some(pid) => return Some(pid),
            None => {
                trace!(
                    "Invalid PID file or layer was not in a good state, cleaning up"
                );
                std::fs::remove_file(&pid_file).ok();
            }
        }
    }

    None
}

pub fn get_layer_pid_path(
    layers_storage_dir: &Path,
    layer_name: &str,
) -> PathBuf {
    layers_storage_dir.join(format!("{}.pid", layer_name))
}
