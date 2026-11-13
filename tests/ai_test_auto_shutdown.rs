mod fixtures;

use anyhow::{Result, anyhow};
use fixtures::*;
use rstest::*;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// The layer's pid file, `<storage>/<name>.pid`
fn pid_file(layer: &LayerManager) -> Result<PathBuf> {
    Ok(PathBuf::from(format!("{}.pid", layer.dir()?.display())))
}

/// PID 1 of the layer, if its pid file names a live process
fn layer_pid(layer: &LayerManager) -> Option<i32> {
    let pid = std::fs::read_to_string(pid_file(layer).ok()?).ok()?;
    let pid = pid.trim().parse::<i32>().ok()?;
    PathBuf::from(format!("/proc/{}", pid))
        .exists()
        .then_some(pid)
}

/// Whether anything is mounted under the layer's directory
fn has_mounts(layer: &LayerManager) -> Result<bool> {
    let dir = layer.dir()?;
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")?;
    Ok(mountinfo.lines().any(|line| {
        line.split_whitespace().nth(4).is_some_and(|mount_point| {
            PathBuf::from(mount_point).starts_with(&dir)
        })
    }))
}

/// Polls `f` every 50 ms until it returns true or `timeout` passes
fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if f() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The parent pid of `pid`
fn ppid_of(pid: i32) -> Result<i32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid))?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .ok_or_else(|| anyhow!("no PPid line for {}", pid))?
        .trim()
        .parse::<i32>()
        .map_err(Into::into)
}

/// When PID 1 dies, the supervisor unmounts the layer and removes its pid file
#[rstest]
fn ai_test_supervisor_cleans_up_after_pid1_exit(
    mut layer: LayerManager,
) -> Result<()> {
    layer.run(&["true"])?;
    let pid = layer_pid(&layer).expect("layer should be running");
    assert!(has_mounts(&layer)?);
    Command::new("sudo")
        .args(["kill", "-KILL", &pid.to_string()])
        .status()?;
    assert!(wait_until(Duration::from_secs(5), || {
        !pid_file(&layer).unwrap().exists()
    }));
    assert!(wait_until(Duration::from_secs(5), || {
        !has_mounts(&layer).unwrap()
    }));
    Ok(())
}

/// PID 1's parent is the devlayer supervisor, and the user can't signal it
#[rstest]
fn ai_test_supervisor_not_killable_by_user(
    mut layer: LayerManager,
) -> Result<()> {
    layer.run(&["true"])?;
    let pid = layer_pid(&layer).expect("layer should be running");
    let ppid = ppid_of(pid)?;
    let comm = std::fs::read_to_string(format!("/proc/{}/comm", ppid))?;
    assert_eq!(comm.trim(), "devlayer");
    let status = Command::new("kill")
        .args(["-TERM", &ppid.to_string()])
        .status()?;
    assert!(!status.success());
    assert!(PathBuf::from(format!("/proc/{}", ppid)).exists());
    Ok(())
}

/// After an explicit stop, the supervisor's cleanup is harmless: the layer
/// restarts and deletes normally, and delete doesn't leave a lock file behind
#[rstest]
fn ai_test_supervisor_after_stop_and_delete(
    mut layer: LayerManager,
) -> Result<()> {
    layer.run(&["true"])?;
    layer.run(&["stop"])?;
    layer.run(&["true"])?;
    assert!(layer_pid(&layer).is_some());
    let lock = PathBuf::from(format!("{}.lock", layer.dir()?.display()));
    layer.run(&["delete", "-y"])?;
    std::thread::sleep(Duration::from_secs(2));
    assert!(!lock.exists());
    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&layer.name));
    Ok(())
}
