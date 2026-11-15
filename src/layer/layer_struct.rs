use crate::util::{get_layer_lock_path, get_layer_pid_path};
use nix::unistd::Gid;
use nix::unistd::Pid;
use nix::unistd::Uid;
use std::path::PathBuf;

pub struct Layer {
    pub name: String,
    /// The storage directory the layer is in
    pub storage_dir: PathBuf,
    pub base: PathBuf,
    pub work_base: PathBuf,
    pub upper_base: PathBuf,
    pub overlay_base: PathBuf,
    pub data_storage_dir: PathBuf, // storage dir for layer data and nested layers
    pub root_overlay: PathBuf,
    pub pid: Pid,
    pub uid: Uid,
    pub gid: Gid,
}

impl Layer {
    /// The pid file naming the layer's PID 1, next to the layer's directory
    pub fn pid_path(&self) -> PathBuf {
        get_layer_pid_path(&self.storage_dir, &self.name)
    }

    /// The layer's lock file, next to the layer's directory
    pub fn lock_path(&self) -> PathBuf {
        get_layer_lock_path(&self.storage_dir, &self.name)
    }
}
