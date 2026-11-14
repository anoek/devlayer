mod fixtures;

use anyhow::{Result, anyhow};
use fixtures::*;
use nix::fcntl::{Flock, FlockArg};
use rstest::*;
use std::fs::File;
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
    let pid = pid.split_whitespace().next()?.parse::<i32>().ok()?;
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
    layer.hold()?;
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
    layer.hold()?;
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
    layer.hold()?;
    assert!(layer_pid(&layer).is_some());
    let lock = PathBuf::from(format!("{}.lock", layer.dir()?.display()));
    layer.run(&["delete", "-y"])?;
    std::thread::sleep(Duration::from_secs(2));
    assert!(!lock.exists());
    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&layer.name));
    Ok(())
}

/// Once its command exits, a layer shuts itself down, keeping its changes,
/// and running a command in it again restarts it
#[rstest]
fn ai_test_idle_layer_shuts_down(mut layer: LayerManager) -> Result<()> {
    let name = format!("devlayer-coverage-idle-{}", rid());
    let file = std::env::current_dir()?.join(&name);
    layer.run(&["touch", file.to_str().unwrap_or_default()])?;
    assert!(wait_until(Duration::from_secs(5), || {
        !pid_file(&layer).unwrap().exists()
    }));
    assert!(!has_mounts(&layer)?);
    assert!(!file.exists());

    layer.run(&["list"])?;
    let stopped = layer.last_stdout.split("Stopped layers:").nth(1);
    assert!(stopped.is_some_and(|stopped| stopped.contains(&layer.name)));

    layer.run(&["status", "/"])?;
    assert!(layer.last_stdout.contains(&name));

    layer.hold()?;
    assert!(layer_pid(&layer).is_some());
    Ok(())
}

/// A process left running in a layer keeps it up until the process exits
#[rstest]
fn ai_test_background_process_keeps_layer_up(
    mut layer: LayerManager,
) -> Result<()> {
    layer.hold()?;
    std::thread::sleep(Duration::from_secs(3));
    assert!(layer_pid(&layer).is_some());
    layer.run(&["pkill", "-f", "sleep 3600"])?;
    assert!(wait_until(Duration::from_secs(5), || {
        !pid_file(&layer).unwrap().exists()
    }));
    Ok(())
}

/// Processes orphaned inside a layer are reaped rather than left as zombies
#[rstest]
fn ai_test_orphans_reaped(mut layer: LayerManager) -> Result<()> {
    layer.hold()?;
    // The subshell exits right away, orphaning a short-lived child
    layer.run(&["sh", "-c", "(sleep 0.2 &)"])?;
    std::thread::sleep(Duration::from_secs(2));
    layer.run(&[
        "sh",
        "-c",
        "grep -l '^State:.*Z' /proc/[0-9]*/status || true",
    ])?;
    assert_eq!(layer.last_stdout.trim(), "");
    Ok(())
}

/// A layer with resource limits removes its cgroup when it shuts down
#[rstest]
fn ai_test_idle_shutdown_removes_cgroup(mut layer: LayerManager) -> Result<()> {
    // Resource limits aren't available everywhere, e.g. inside another layer
    if !layer.pass(&["--cpus=1", "true"]) {
        return Ok(());
    }
    let cgroup = PathBuf::from("/sys/fs/cgroup/devlayer").join(format!(
        "{}-{}",
        nix::unistd::getuid(),
        layer.name
    ));
    assert!(wait_until(Duration::from_secs(5), || !cgroup.exists()));
    Ok(())
}

/// When a layer was restarted by the time its old supervisor gets to clean
/// up, the supervisor leaves the new instance's pid file and mounts alone
#[rstest]
fn ai_test_supervisor_leaves_restarted_layer(
    mut layer: LayerManager,
) -> Result<()> {
    let mut other = LayerManager::new();
    layer.hold()?;
    other.hold()?;
    let pid = layer_pid(&layer).expect("layer should be running");
    assert!(layer_pid(&other).is_some(), "other layer should be running");
    let other_pid_file = std::fs::read_to_string(pid_file(&other)?)?;
    let pid_file = pid_file(&layer)?;
    let lock_file = format!("{}.lock", layer.dir()?.display());

    // Hold the layer lock so the supervisor waits to clean up, as it would
    // while another devlayer restarts the layer
    let lock = Flock::lock(File::open(&lock_file)?, FlockArg::LockExclusive)
        .map_err(|(_, e)| e)?;
    Command::new("sudo")
        .args(["kill", "-KILL", &pid.to_string()])
        .status()?;
    assert!(wait_until(Duration::from_secs(5), || {
        !PathBuf::from(format!("/proc/{}", pid)).exists()
    }));
    // Restarting writes the new PID 1 to the pid file, a running layer's
    // PID 1 stands in for it
    Command::new("sudo")
        .args([
            "sh",
            "-c",
            &format!("printf %s '{}' > {}", other_pid_file, pid_file.display()),
        ])
        .status()?;
    drop(lock);

    std::thread::sleep(Duration::from_secs(2));
    assert!(has_mounts(&layer)?);
    assert_eq!(std::fs::read_to_string(&pid_file)?, other_pid_file);

    // Don't let stopping this layer stop the other one
    Command::new("sudo")
        .args(["rm", &pid_file.to_string_lossy()])
        .status()?;
    Ok(())
}

/// A command that's slow to start in a new layer still runs: the layer
/// doesn't shut down for lack of anything running in it while the command
/// is on its way
#[rstest]
fn ai_test_layer_waits_for_starting_command(
    mut layer: LayerManager,
) -> Result<()> {
    // By then PID 1 has looked for anything running in the layer
    layer.run_with_env(
        &["sh", "-c", "echo started"],
        "TEST_DELAY_FORK",
        "1500",
    )?;
    assert_eq!(layer.last_stdout.trim(), "started");
    Ok(())
}

/// A launch killed before its command starts doesn't keep the layer from
/// shutting down
#[rstest]
fn ai_test_killed_launch_lets_layer_shut_down(
    layer: LayerManager,
) -> Result<()> {
    let pid_file = pid_file(&layer)?;
    // The setuid binary rather than sudo, so we kill the launch itself
    let mut launch = Command::new(format!("{}-setuid", layer.devlayer_bin))
        .args([
            &format!("--join={}", layer.name),
            "--no-config",
            "--no-bind-cwd",
            "true",
        ])
        .env("TEST_DELAY_FORK", "60000")
        .spawn()?;
    assert!(wait_until(Duration::from_secs(5), || pid_file.exists()));
    launch.kill()?;
    launch.wait()?;
    assert!(wait_until(Duration::from_secs(5), || !pid_file.exists()));
    Ok(())
}
